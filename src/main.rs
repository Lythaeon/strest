mod app;
mod args;
mod charts;
mod config;
mod distributed;
mod error;
mod http;
mod logger;
mod metrics;
#[cfg(feature = "wasm")]
mod probestack;
mod script;
mod service;
mod shutdown;
mod sinks;
mod ui;

#[cfg(feature = "alloc-profiler")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

use app::{run_cleanup, run_local, run_replay};
use args::{Command, OutputFormat, TesterArgs};
use clap::{CommandFactory, FromArgMatches};
use error::{AppError, AppResult};
use rand::distributions::Distribution;
use rand::thread_rng;
use std::path::Path;

fn main() -> AppResult<()> {
    let mut cmd = TesterArgs::command();
    let raw_args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let treat_as_empty = raw_args.len() <= 1
        || (raw_args.len() == 2 && raw_args.get(1).map(|arg| arg == "--").unwrap_or(false));
    if treat_as_empty {
        let has_default_config =
            Path::new("strest.toml").exists() || Path::new("strest.json").exists();
        if !has_default_config {
            cmd.print_help()?;
            println!();
            return Ok(());
        }
    }

    let matches = cmd.get_matches_from(raw_args);
    let mut args =
        TesterArgs::from_arg_matches(&matches).map_err(|err| AppError::Message(err.to_string()))?;

    logger::init_logging(args.verbose, args.no_color);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        if let Some(command) = args.command.clone() {
            match command {
                Command::Cleanup(cleanup_args) => {
                    run_cleanup(&cleanup_args).await.map_err(AppError::from)?;
                    return Ok(());
                }
            }
        }

        if args.replay {
            run_replay(&args).await?;
            return Ok(());
        }

        let mut scenario_registry = None;
        let mut loaded_config = config::load_config(args.config.as_deref()).map_err(AppError::from)?;
        if let Some(config) = loaded_config.as_mut() {
            scenario_registry = config.scenarios.take();
            config::apply_config(&mut args, &matches, config).map_err(AppError::from)?;
        }

        apply_output_aliases(&mut args).map_err(AppError::from)?;
        validate_db_logging(&args).map_err(AppError::from)?;

        if args.dump_urls.is_some() {
            dump_urls(&args).map_err(AppError::from)?;
            return Ok(());
        }

        if args.controller_listen.is_some() && args.agent_join.is_some() {
            return Err("Cannot run as controller and agent at the same time.".into());
        }

        if args.install_service || args.uninstall_service {
            service::handle_service_action(&args).map_err(AppError::from)?;
            return Ok(());
        }

        if args.script.is_some() && args.scenario.is_some() {
            return Err("Cannot combine --script with scenario config.".into());
        }

        if let Some(script_path) = args.script.as_deref() {
            let scenario = script::load_scenario_from_wasm(script_path, &args)
                .map_err(AppError::from)?;
            args.scenario = Some(scenario);
        }

        if args.controller_listen.is_some() {
            distributed::run_controller(&args, scenario_registry)
                .await
                .map_err(AppError::from)?;
            return Ok(());
        }

        if args.no_ua && !args.authorized {
            tracing::error!(
                "Refusing to disable the default User-Agent without explicit authorization."
            );
            return Err(
                "Disabling the default User-Agent requires --authorized (or config authorized = true)."
                    .into(),
            );
        }

        if args.agent_join.is_some() {
            distributed::run_agent(args)
                .await
                .map_err(AppError::from)?;
            return Ok(());
        }

        if args.url.is_none() && args.scenario.is_none() {
            tracing::error!("Missing URL (set --url or provide in config).");
            return Err("Missing URL (set --url or provide in config).".into());
        }

        args.distributed_stream_summaries = false;
        let outcome = run_local(args, None, None).await?;

        if !outcome.runtime_errors.is_empty() {
            app::print_runtime_errors(&outcome.runtime_errors);
            return Err("Runtime errors occurred.".into());
        }

        Ok(())
    })
}

fn apply_output_aliases(args: &mut TesterArgs) -> Result<(), String> {
    let output = match args.output.clone() {
        Some(output) => output,
        None => {
            if args.output_format.is_some() {
                return Err("`--output-format` requires `--output`.".to_owned());
            }
            return Ok(());
        }
    };

    if args.export_csv.is_some() || args.export_json.is_some() || args.export_jsonl.is_some() {
        return Err("`--output` cannot be combined with export flags.".to_owned());
    }

    let format = args
        .output_format
        .unwrap_or_else(|| infer_output_format(&output).unwrap_or(OutputFormat::Text));

    match format {
        OutputFormat::Json => {
            args.export_json = Some(output);
        }
        OutputFormat::Jsonl => {
            args.export_jsonl = Some(output);
        }
        OutputFormat::Csv => {
            args.export_csv = Some(output);
        }
        OutputFormat::Text | OutputFormat::Quiet => {
            args.output_format = Some(format);
        }
    }

    Ok(())
}

fn infer_output_format(output: &str) -> Option<OutputFormat> {
    let lower = output.to_ascii_lowercase();
    if lower.ends_with(".jsonl") {
        return Some(OutputFormat::Jsonl);
    }
    if lower.ends_with(".json") {
        return Some(OutputFormat::Json);
    }
    if lower.ends_with(".csv") {
        return Some(OutputFormat::Csv);
    }
    None
}

fn validate_db_logging(args: &TesterArgs) -> Result<(), String> {
    if args.db_url.is_some() && args.log_shards.get() > 1 {
        return Err("`--db-url` requires `--log-shards 1`.".to_owned());
    }
    Ok(())
}

fn dump_urls(args: &TesterArgs) -> Result<(), String> {
    if args.scenario.is_some() {
        return Err("--dump-urls cannot be used with scenarios.".to_owned());
    }
    if !args.rand_regex_url {
        return Err("--dump-urls requires --rand-regex-url.".to_owned());
    }
    let count = args
        .dump_urls
        .map(|value| value.get())
        .ok_or_else(|| "--dump-urls requires a count.".to_owned())?;
    let pattern = args
        .url
        .as_deref()
        .ok_or_else(|| "Missing URL (set --url or provide in config).".to_owned())?;
    let max_repeat = u32::try_from(args.max_repeat.get()).unwrap_or(u32::MAX);
    let regex = rand_regex::Regex::compile(pattern, max_repeat)
        .map_err(|err| format!("Invalid rand-regex pattern '{}': {}", pattern, err))?;
    let mut rng = thread_rng();
    for _ in 0..count {
        let url: String = regex.sample(&mut rng);
        println!("{}", url);
    }
    Ok(())
}
