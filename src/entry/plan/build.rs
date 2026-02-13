use std::collections::BTreeMap;

use clap::ArgMatches;

use crate::args::{Command, LoadMode, OutputFormat, TesterArgs};
use crate::config::types::ScenarioConfig;
#[cfg(not(feature = "wasm"))]
use crate::error::ScriptError;
use crate::error::{AppError, AppResult, ValidationError};
use crate::protocol::protocol_registry;

use super::types::{DumpUrlsPlan, LocalArgs, RunPlan};

/// Only one shard is allowed when DB logging is enabled.
const SINGLE_LOG_SHARD: usize = 1;

pub(crate) fn build_plan(mut args: TesterArgs, matches: &ArgMatches) -> AppResult<RunPlan> {
    if let Some(command) = args.command.take() {
        match command {
            Command::Quick(preset) => {
                args.url = Some(preset.url);
                args.target_duration = preset.target_duration;
                args.max_tasks = preset.max_tasks;
                args.rate_limit = preset.rate_limit;
                args.load_mode = LoadMode::Arrival;
                args.no_charts = true;
            }
            Command::Soak(preset) => {
                args.url = Some(preset.url);
                args.target_duration = preset.target_duration;
                args.max_tasks = preset.max_tasks;
                args.rate_limit = preset.rate_limit;
                args.load_mode = LoadMode::Soak;
                args.no_ui = true;
                args.summary = true;
            }
            Command::Spike(preset) => {
                args.url = Some(preset.url);
                args.target_duration = preset.target_duration;
                args.max_tasks = preset.max_tasks;
                args.spawn_rate_per_tick = preset.spawn_rate_per_tick;
                args.tick_interval = preset.tick_interval;
                args.load_mode = LoadMode::Burst;
            }
            Command::Distributed(preset) => {
                args.url = Some(preset.url);
                args.target_duration = preset.target_duration;
                args.controller_listen = Some(preset.controller_listen);
                args.min_agents = preset.agents;
                args.auth_token = preset.auth_token;
                args.load_mode = LoadMode::Ramp;
                args.distributed_stream_summaries = true;
                args.no_ui = true;
                args.summary = true;
            }
            Command::Cleanup(cleanup_args) => {
                return Ok(RunPlan::Cleanup(cleanup_args));
            }
            Command::Compare(compare_args) => {
                return Ok(RunPlan::Compare(compare_args));
            }
        }
    }

    if args.replay {
        return Ok(RunPlan::Replay(args));
    }

    let scenario_registry = apply_config(&mut args, matches)?;

    apply_output_aliases(&mut args)?;
    validate_db_logging(&args)?;
    validate_protocol_support(&args)?;

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

    #[cfg(not(feature = "wasm"))]
    if !args.plugin.is_empty() {
        return Err(AppError::script(ScriptError::WasmFeatureDisabled));
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

fn validate_protocol_support(args: &TesterArgs) -> AppResult<()> {
    let registry = protocol_registry();
    let Some(adapter) = registry.adapter(args.protocol) else {
        let supported = registry.executable_protocols_csv();
        return Err(AppError::validation(ValidationError::UnsupportedProtocol {
            protocol: args.protocol.as_str().to_owned(),
            supported,
        }));
    };
    tracing::debug!(
        "Protocol adapter selected: {} (stateful={})",
        adapter.display_name(),
        adapter.supports_stateful_connections()
    );
    if !registry.supports_execution(args.protocol) {
        let supported = registry.executable_protocols_csv();
        return Err(AppError::validation(ValidationError::UnsupportedProtocol {
            protocol: args.protocol.as_str().to_owned(),
            supported,
        }));
    }
    if !registry.supports_load_mode(args.protocol, args.load_mode) {
        return Err(AppError::validation(
            ValidationError::UnsupportedLoadModeForProtocol {
                protocol: args.protocol.as_str().to_owned(),
                load_mode: args.load_mode.as_str().to_owned(),
            },
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
