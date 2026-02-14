use std::collections::BTreeMap;

use crate::args::TesterArgs;
use crate::config::types::ScenarioConfig;
use crate::domain::run::RunConfig;

#[derive(Debug)]
pub(crate) struct LocalRunCommand {
    run_config: RunConfig,
    args: TesterArgs,
}

impl LocalRunCommand {
    #[must_use]
    pub(crate) const fn new(run_config: RunConfig, args: TesterArgs) -> Self {
        Self { run_config, args }
    }

    #[must_use]
    pub(crate) const fn run_config(&self) -> &RunConfig {
        &self.run_config
    }

    #[must_use]
    pub(crate) const fn no_color(&self) -> bool {
        self.args.no_color
    }

    #[must_use]
    pub(crate) fn into_args(self) -> TesterArgs {
        self.args
    }
}

#[derive(Debug)]
pub(crate) struct ReplayRunCommand {
    run_config: RunConfig,
    args: TesterArgs,
}

impl ReplayRunCommand {
    #[must_use]
    pub(crate) const fn new(run_config: RunConfig, args: TesterArgs) -> Self {
        Self { run_config, args }
    }

    #[must_use]
    pub(crate) const fn run_config(&self) -> &RunConfig {
        &self.run_config
    }

    #[must_use]
    pub(crate) const fn no_color(&self) -> bool {
        self.args.no_color
    }

    #[must_use]
    pub(crate) const fn as_args(&self) -> &TesterArgs {
        &self.args
    }
}

#[derive(Debug)]
pub(crate) struct ServiceCommand {
    args: TesterArgs,
}

impl ServiceCommand {
    #[must_use]
    pub(crate) const fn new(args: TesterArgs) -> Self {
        Self { args }
    }

    #[must_use]
    pub(crate) const fn as_args(&self) -> &TesterArgs {
        &self.args
    }
}

#[derive(Debug)]
pub(crate) enum DistributedRunMode {
    Controller {
        scenarios: Option<BTreeMap<String, ScenarioConfig>>,
    },
    Agent,
}

#[derive(Debug)]
pub(crate) struct DistributedRunCommand {
    run_config: RunConfig,
    args: TesterArgs,
    mode: DistributedRunMode,
}

impl DistributedRunCommand {
    #[must_use]
    pub(crate) const fn new_controller(
        run_config: RunConfig,
        args: TesterArgs,
        scenarios: Option<BTreeMap<String, ScenarioConfig>>,
    ) -> Self {
        Self {
            run_config,
            args,
            mode: DistributedRunMode::Controller { scenarios },
        }
    }

    #[must_use]
    pub(crate) const fn new_agent(run_config: RunConfig, args: TesterArgs) -> Self {
        Self {
            run_config,
            args,
            mode: DistributedRunMode::Agent,
        }
    }

    #[must_use]
    pub(crate) const fn run_config(&self) -> &RunConfig {
        &self.run_config
    }

    #[must_use]
    pub(crate) const fn no_color(&self) -> bool {
        self.args.no_color
    }

    #[must_use]
    pub(crate) const fn mode_name(&self) -> &'static str {
        match self.mode {
            DistributedRunMode::Controller { .. } => "controller",
            DistributedRunMode::Agent => "agent",
        }
    }

    #[must_use]
    pub(crate) fn into_parts(self) -> (TesterArgs, DistributedRunMode) {
        (self.args, self.mode)
    }
}
