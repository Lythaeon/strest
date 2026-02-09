mod app;
mod arcshift;
mod args;
mod charts;
mod config;
mod distributed;
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

use app::run_local;
use args::TesterArgs;
use clap::{CommandFactory, FromArgMatches};
use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
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
    let mut args = TesterArgs::from_arg_matches(&matches)
        .map_err(|err| std::io::Error::other(err.to_string()))?;

    logger::init_logging(args.verbose);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let mut scenario_registry = None;
        let mut loaded_config =
            config::load_config(args.config.as_deref()).map_err(std::io::Error::other)?;
        if let Some(config) = loaded_config.as_mut() {
            scenario_registry = config.scenarios.take();
            config::apply_config(&mut args, &matches, config).map_err(std::io::Error::other)?;
        }

        if args.controller_listen.is_some() && args.agent_join.is_some() {
            return Err(std::io::Error::other(
                "Cannot run as controller and agent at the same time.",
            )
            .into());
        }

        if args.install_service || args.uninstall_service {
            service::handle_service_action(&args).map_err(std::io::Error::other)?;
            return Ok(());
        }

        if args.script.is_some() && args.scenario.is_some() {
            return Err(
                std::io::Error::other("Cannot combine --script with scenario config.").into(),
            );
        }

        if let Some(script_path) = args.script.as_deref() {
            let scenario = script::load_scenario_from_wasm(script_path, &args)
                .map_err(std::io::Error::other)?;
            args.scenario = Some(scenario);
        }

        if args.controller_listen.is_some() {
            distributed::run_controller(&args, scenario_registry)
                .await
                .map_err(std::io::Error::other)?;
            return Ok(());
        }

        if args.no_ua && !args.authorized {
            tracing::error!(
                "Refusing to disable the default User-Agent without explicit authorization."
            );
            return Err(std::io::Error::other(
                "Disabling the default User-Agent requires --authorized (or config authorized = true).",
            )
            .into());
        }

        if args.agent_join.is_some() {
            distributed::run_agent(args)
                .await
                .map_err(std::io::Error::other)?;
            return Ok(());
        }

        if args.url.is_none() && args.scenario.is_none() {
            tracing::error!("Missing URL (set --url or provide in config).");
            return Err(
                std::io::Error::other("Missing URL (set --url or provide in config).").into(),
            );
        }

        args.distributed_stream_summaries = false;
        let outcome = run_local(args, None, None).await?;

        if !outcome.runtime_errors.is_empty() {
            app::print_runtime_errors(&outcome.runtime_errors);
            return Err(std::io::Error::other("Runtime errors occurred.").into());
        }

        Ok(())
    })
}
