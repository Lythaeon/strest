use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

use clap::{ArgMatches, CommandFactory, FromArgMatches};
use rand::distributions::Distribution;
use rand::thread_rng;

use crate::app::{self, run_cleanup, run_local, run_replay};
use crate::args::{CleanupArgs, Command, OutputFormat, TesterArgs};
use crate::config::types::ScenarioConfig;
use crate::error::{AppError, AppResult, ValidationError};

/// Default config filenames checked when no CLI args are provided.
const DEFAULT_CONFIG_FILES: [&str; 2] = ["strest.toml", "strest.json"];
/// Only one shard is allowed when DB logging is enabled.
const SINGLE_LOG_SHARD: usize = 1;

struct DumpUrlsPlan {
    pattern: String,
    count: usize,
    max_repeat: u32,
}

struct LocalArgs {
    args: TesterArgs,
}

impl LocalArgs {
    fn new(mut args: TesterArgs) -> AppResult<Self> {
        if args.url.is_none() && args.scenario.is_none() {
            tracing::error!("Missing URL (set --url or provide in config).");
            return Err(AppError::validation(ValidationError::MissingUrl));
        }
        args.distributed_stream_summaries = false;
        Ok(Self { args })
    }
}

enum RunPlan {
    Cleanup(CleanupArgs),
    Replay(TesterArgs),
    DumpUrls(DumpUrlsPlan),
    Service(TesterArgs),
    Controller {
        args: TesterArgs,
        scenarios: Option<BTreeMap<String, ScenarioConfig>>,
    },
    Agent(TesterArgs),
    Local(LocalArgs),
}

pub(crate) fn run() -> AppResult<()> {
    let (args, matches) = match parse_args()? {
        Some(parsed) => parsed,
        None => return Ok(()),
    };

    crate::logger::init_logging(args.verbose, args.no_color);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(run_async(args, &matches))
}

fn parse_args() -> AppResult<Option<(TesterArgs, ArgMatches)>> {
    let mut cmd = TesterArgs::command();
    let raw_args: Vec<OsString> = std::env::args_os().collect();

    if should_show_help(&raw_args) {
        cmd.print_help()?;
        println!();
        return Ok(None);
    }

    let matches = cmd.get_matches_from(raw_args);
    let args = TesterArgs::from_arg_matches(&matches)?;

    Ok(Some((args, matches)))
}

fn should_show_help(raw_args: &[OsString]) -> bool {
    let treat_as_empty =
        matches!(raw_args, [] | [_]) || matches!(raw_args, [_, second] if second == "--");
    if !treat_as_empty {
        return false;
    }

    !has_default_config()
}

fn has_default_config() -> bool {
    DEFAULT_CONFIG_FILES
        .iter()
        .any(|path| Path::new(path).exists())
}

async fn run_async(args: TesterArgs, matches: &ArgMatches) -> AppResult<()> {
    let plan = build_plan(args, matches)?;
    execute_plan(plan).await
}

fn build_plan(mut args: TesterArgs, matches: &ArgMatches) -> AppResult<RunPlan> {
    if let Some(command) = args.command.take() {
        match command {
            Command::Cleanup(cleanup_args) => {
                return Ok(RunPlan::Cleanup(cleanup_args));
            }
        }
    }

    if args.replay {
        return Ok(RunPlan::Replay(args));
    }

    let scenario_registry = apply_config(&mut args, matches)?;

    apply_output_aliases(&mut args)?;
    validate_db_logging(&args)?;

    if args.dump_urls.is_some() {
        let plan = build_dump_urls_plan(&args)?;
        return Ok(RunPlan::DumpUrls(plan));
    }

    if args.controller_listen.is_some() && args.agent_join.is_some() {
        return Err(AppError::validation(
            ValidationError::ControllerAgentConflict,
        ));
    }

    if args.install_service || args.uninstall_service {
        return Ok(RunPlan::Service(args));
    }

    if args.script.is_some() && args.scenario.is_some() {
        return Err(AppError::validation(
            ValidationError::ScriptScenarioConflict,
        ));
    }

    if let Some(script_path) = args.script.as_deref() {
        let scenario = crate::script::load_scenario_from_wasm(script_path, &args)?;
        args.scenario = Some(scenario);
    }

    if args.controller_listen.is_some() {
        return Ok(RunPlan::Controller {
            args,
            scenarios: scenario_registry,
        });
    }

    if args.no_ua && !args.authorized {
        tracing::error!(
            "Refusing to disable the default User-Agent without explicit authorization."
        );
        return Err(AppError::validation(
            ValidationError::NoUserAgentWithoutAuthorization,
        ));
    }

    if args.agent_join.is_some() {
        return Ok(RunPlan::Agent(args));
    }

    let local_args = LocalArgs::new(args)?;
    Ok(RunPlan::Local(local_args))
}

async fn execute_plan(plan: RunPlan) -> AppResult<()> {
    match plan {
        RunPlan::Cleanup(cleanup_args) => run_cleanup(&cleanup_args).await,
        RunPlan::Replay(args) => run_replay(&args).await,
        RunPlan::DumpUrls(plan) => dump_urls(plan),
        RunPlan::Service(args) => {
            crate::service::handle_service_action(&args)?;
            Ok(())
        }
        RunPlan::Controller { args, scenarios } => {
            crate::distributed::run_controller(&args, scenarios).await
        }
        RunPlan::Agent(args) => crate::distributed::run_agent(args).await,
        RunPlan::Local(local) => {
            let outcome = run_local(local.args, None, None).await?;
            if !outcome.runtime_errors.is_empty() {
                app::print_runtime_errors(&outcome.runtime_errors);
                return Err(AppError::validation(ValidationError::RuntimeErrors));
            }
            Ok(())
        }
    }
}

fn apply_config(
    args: &mut TesterArgs,
    matches: &ArgMatches,
) -> AppResult<Option<BTreeMap<String, ScenarioConfig>>> {
    let mut scenario_registry = None;
    let mut loaded_config = crate::config::load_config(args.config.as_deref())?;
    if let Some(config) = loaded_config.as_mut() {
        scenario_registry = config.scenarios.take();
        crate::config::apply_config(args, matches, config)?;
    }
    Ok(scenario_registry)
}

fn apply_output_aliases(args: &mut TesterArgs) -> AppResult<()> {
    let output = match args.output.clone() {
        Some(output) => output,
        None => {
            if args.output_format.is_some() {
                return Err(AppError::validation(
                    ValidationError::OutputFormatRequiresOutput,
                ));
            }
            return Ok(());
        }
    };

    if args.export_csv.is_some() || args.export_json.is_some() || args.export_jsonl.is_some() {
        return Err(AppError::validation(ValidationError::OutputWithExportFlags));
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

fn validate_db_logging(args: &TesterArgs) -> AppResult<()> {
    if args.db_url.is_some() && args.log_shards.get() > SINGLE_LOG_SHARD {
        return Err(AppError::validation(
            ValidationError::DbUrlRequiresSingleShard,
        ));
    }
    Ok(())
}

fn build_dump_urls_plan(args: &TesterArgs) -> AppResult<DumpUrlsPlan> {
    if args.scenario.is_some() {
        return Err(AppError::validation(ValidationError::DumpUrlsWithScenario));
    }
    if !args.rand_regex_url {
        return Err(AppError::validation(
            ValidationError::DumpUrlsRequiresRandRegex,
        ));
    }
    let count = args
        .dump_urls
        .map(|value| value.get())
        .ok_or_else(|| AppError::validation(ValidationError::DumpUrlsRequiresCount))?;
    let pattern = args
        .url
        .as_deref()
        .ok_or_else(|| AppError::validation(ValidationError::MissingUrl))?
        .to_owned();
    let max_repeat = u32::try_from(args.max_repeat.get()).unwrap_or(u32::MAX);

    Ok(DumpUrlsPlan {
        pattern,
        count,
        max_repeat,
    })
}

fn dump_urls(plan: DumpUrlsPlan) -> AppResult<()> {
    let regex = rand_regex::Regex::compile(&plan.pattern, plan.max_repeat).map_err(|err| {
        AppError::validation(ValidationError::InvalidRandRegex {
            pattern: plan.pattern,
            source: err,
        })
    })?;
    let mut rng = thread_rng();
    for _ in 0..plan.count {
        let url: String = regex.sample(&mut rng);
        println!("{}", url);
    }
    Ok(())
}
