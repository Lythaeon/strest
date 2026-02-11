use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

#[derive(Debug, Deserialize, Default)]
pub(super) struct ControlStartRequest {
    pub(super) scenario_name: Option<String>,
    pub(super) scenario: Option<crate::config::types::ScenarioConfig>,
    pub(super) start_after_ms: Option<u64>,
    pub(super) agent_wait_timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(super) struct ControlResponse {
    pub(super) status: String,
    pub(super) run_id: Option<String>,
}

#[derive(Debug)]
pub(super) struct ControlError {
    pub(super) status: u16,
    pub(super) message: String,
}

impl ControlError {
    pub(super) fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

pub(super) enum ControlCommand {
    Start {
        request: ControlStartRequest,
        respond_to: oneshot::Sender<Result<ControlResponse, ControlError>>,
    },
    Stop {
        respond_to: oneshot::Sender<Result<ControlResponse, ControlError>>,
    },
}
