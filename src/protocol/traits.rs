use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::args::TesterArgs;
use crate::domain::run::{LoadMode, ProtocolKind};
use crate::error::{AppError, AppResult, ValidationError};
use crate::metrics::{LogSink, Metrics};
use crate::shutdown::ShutdownSender;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolAdapterError {
    pub message: String,
}

pub trait ProtocolAdapter: Send + Sync {
    fn protocol(&self) -> ProtocolKind;
    fn display_name(&self) -> &'static str;
    fn executes_traffic(&self) -> bool;
    fn supports_stateful_connections(&self) -> bool;
    fn supported_load_modes(&self) -> &'static [LoadMode];
}

pub trait TransportAdapter: ProtocolAdapter {
    /// Creates the request sender task for this protocol adapter.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedProtocol` when the adapter does not provide
    /// traffic execution behavior.
    fn setup_request_sender(
        &self,
        args: &TesterArgs,
        shutdown_tx: &ShutdownSender,
        metrics_tx: &mpsc::Sender<Metrics>,
        log_sink: Option<&Arc<LogSink>>,
    ) -> AppResult<JoinHandle<()>> {
        let _ = (args, shutdown_tx, metrics_tx, log_sink);
        Err(AppError::validation(ValidationError::UnsupportedProtocol {
            protocol: self.protocol().as_str().to_owned(),
            supported: String::new(),
        }))
    }
}
