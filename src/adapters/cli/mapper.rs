use std::collections::BTreeMap;

use crate::application::commands::{
    AgentRunCommand, ControllerRunCommand, LocalRunCommand, ReplayRunCommand, ServiceCommand,
};
use crate::args::{
    LoadMode as CliLoadMode, Protocol as CliProtocol, Scenario as CliScenario, TesterArgs,
};
use crate::config::types::ScenarioConfig;
use crate::domain::run::{LoadMode, ProtocolKind, RunConfig, Scenario};
use crate::error::{AppError, AppResult, ValidationError};

pub(crate) fn to_local_run_command(mut args: TesterArgs) -> AppResult<LocalRunCommand> {
    if args.url.is_none() && args.scenario.is_none() {
        tracing::error!("Missing URL (set --url or provide in config).");
        return Err(AppError::validation(ValidationError::MissingUrl));
    }
    args.distributed_stream_summaries = false;
    let run_config = to_run_config(&args);
    Ok(LocalRunCommand::new(run_config, args))
}

pub(crate) fn to_replay_run_command(args: TesterArgs) -> ReplayRunCommand {
    let run_config = to_run_config(&args);
    ReplayRunCommand::new(run_config, args)
}

pub(crate) const fn to_service_command(args: TesterArgs) -> ServiceCommand {
    ServiceCommand::new(args)
}

pub(crate) fn to_controller_run_command(
    args: TesterArgs,
    scenarios: Option<BTreeMap<String, ScenarioConfig>>,
) -> ControllerRunCommand {
    let run_config = to_run_config(&args);
    ControllerRunCommand::new(run_config, args, scenarios)
}

pub(crate) fn to_agent_run_command(args: TesterArgs) -> AgentRunCommand {
    let run_config = to_run_config(&args);
    AgentRunCommand::new(run_config, args)
}

fn to_run_config(args: &TesterArgs) -> RunConfig {
    RunConfig {
        protocol: map_protocol(args.protocol),
        load_mode: map_load_mode(args.load_mode),
        target_url: args.url.clone(),
        scenario: args.scenario.as_ref().map(map_scenario),
    }
}

const fn map_protocol(protocol: CliProtocol) -> ProtocolKind {
    protocol.to_domain()
}

const fn map_load_mode(load_mode: CliLoadMode) -> LoadMode {
    load_mode.to_domain()
}

fn map_scenario(scenario: &CliScenario) -> Scenario {
    Scenario {
        base_url: scenario.base_url.clone(),
        vars_count: scenario.vars.len(),
        step_count: scenario.steps.len(),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::args::TesterArgs;
    use crate::domain::run::{LoadMode, ProtocolKind};
    use crate::error::{AppError, ValidationError};

    use super::to_local_run_command;

    #[test]
    fn local_mapper_requires_url_or_scenario() {
        let args_result = TesterArgs::try_parse_from(["strest"]);
        assert!(
            args_result.is_ok(),
            "Expected CLI parsing to succeed without URL for mapper validation"
        );
        let args = if let Ok(args) = args_result {
            args
        } else {
            return;
        };

        let mapped = to_local_run_command(args);
        assert!(
            mapped.is_err(),
            "Expected local command mapper to reject missing URL and scenario"
        );
        let err = if let Err(err) = mapped {
            err
        } else {
            return;
        };
        assert!(matches!(
            err,
            AppError::Validation(ValidationError::MissingUrl)
        ));
    }

    #[test]
    fn local_mapper_builds_domain_run_config() {
        let args_result = TesterArgs::try_parse_from([
            "strest",
            "--url",
            "grpc://127.0.0.1:50051/test.Service/Method",
            "--protocol",
            "grpc-unary",
            "--load-mode",
            "arrival",
        ]);
        assert!(
            args_result.is_ok(),
            "Expected CLI parsing to succeed for local mapper test"
        );
        let args = if let Ok(args) = args_result {
            args
        } else {
            return;
        };

        let mapped = to_local_run_command(args);
        assert!(
            mapped.is_ok(),
            "Expected local command mapping to succeed for valid arguments"
        );
        let command = if let Ok(command) = mapped {
            command
        } else {
            return;
        };

        assert_eq!(command.run_config().protocol, ProtocolKind::GrpcUnary);
        assert_eq!(command.run_config().load_mode, LoadMode::Arrival);
        assert_eq!(
            command.run_config().target_url.as_deref(),
            Some("grpc://127.0.0.1:50051/test.Service/Method")
        );
        assert_eq!(command.run_config().scenario_step_count(), 0);
    }
}
