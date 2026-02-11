use std::collections::BTreeMap;

use crate::args::{ControllerMode, TesterArgs};
use crate::config::types::ScenarioConfig;
use crate::error::AppResult;

use super::{auto, manual};

/// Runs the distributed controller in auto or manual mode.
///
/// # Errors
///
/// Returns an error if the controller cannot bind, validate configuration,
/// or complete the distributed run.
pub async fn run_controller(
    args: &TesterArgs,
    scenarios: Option<BTreeMap<String, ScenarioConfig>>,
) -> AppResult<()> {
    match args.controller_mode {
        ControllerMode::Auto => auto::run_controller_auto(args).await,
        ControllerMode::Manual => {
            manual::run_controller_manual(args, scenarios.unwrap_or_default()).await
        }
    }
}
