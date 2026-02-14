use std::collections::BTreeMap;

use crate::config::types::ScenarioConfig;
use crate::domain::run::RunConfig;

#[derive(Debug)]
pub(crate) struct LocalRunCommand {
    run_config: RunConfig,
    no_color: bool,
}

impl LocalRunCommand {
    #[must_use]
    pub(crate) const fn new(run_config: RunConfig, no_color: bool) -> Self {
        Self {
            run_config,
            no_color,
        }
    }

    #[must_use]
    pub(crate) const fn run_config(&self) -> &RunConfig {
        &self.run_config
    }

    #[must_use]
    pub(crate) const fn no_color(&self) -> bool {
        self.no_color
    }
}

#[derive(Debug)]
pub(crate) struct ReplayRunCommand {
    run_config: RunConfig,
    no_color: bool,
}

impl ReplayRunCommand {
    #[must_use]
    pub(crate) const fn new(run_config: RunConfig, no_color: bool) -> Self {
        Self {
            run_config,
            no_color,
        }
    }

    #[must_use]
    pub(crate) const fn run_config(&self) -> &RunConfig {
        &self.run_config
    }

    #[must_use]
    pub(crate) const fn no_color(&self) -> bool {
        self.no_color
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
    no_color: bool,
    mode: DistributedRunMode,
}

impl DistributedRunCommand {
    #[must_use]
    pub(crate) const fn new_controller(
        run_config: RunConfig,
        no_color: bool,
        scenarios: Option<BTreeMap<String, ScenarioConfig>>,
    ) -> Self {
        Self {
            run_config,
            no_color,
            mode: DistributedRunMode::Controller { scenarios },
        }
    }

    #[must_use]
    pub(crate) const fn new_agent(run_config: RunConfig, no_color: bool) -> Self {
        Self {
            run_config,
            no_color,
            mode: DistributedRunMode::Agent,
        }
    }

    #[must_use]
    pub(crate) const fn run_config(&self) -> &RunConfig {
        &self.run_config
    }

    #[must_use]
    pub(crate) const fn no_color(&self) -> bool {
        self.no_color
    }

    #[must_use]
    pub(crate) const fn mode_name(&self) -> &'static str {
        match self.mode {
            DistributedRunMode::Controller { .. } => "controller",
            DistributedRunMode::Agent => "agent",
        }
    }

    #[must_use]
    pub(crate) fn into_mode(self) -> DistributedRunMode {
        self.mode
    }
}
