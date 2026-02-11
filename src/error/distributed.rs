use super::ValidationError;
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy)]
pub enum WireValueField {
    #[error("target_duration")]
    TargetDuration,
    #[error("log_shards")]
    LogShards,
    #[error("max_tasks")]
    MaxTasks,
    #[error("spawn_rate_per_tick")]
    SpawnRatePerTick,
    #[error("tick_interval")]
    TickInterval,
    #[error("rate_limit")]
    RateLimit,
    #[error("metrics_max")]
    MetricsMax,
    #[error("stream_interval_ms")]
    StreamIntervalMs,
}

#[derive(Debug, Error)]
pub enum DistributedError {
    #[error("Missing required option: {option}")]
    MissingOption { option: &'static str },
    #[error("Missing --controller-listen.")]
    MissingControllerListen,
    #[error("Missing --control-listen for manual controller.")]
    MissingControlListen,
    #[error("I/O error during {context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("Connection error to {addr}: {source}")]
    Connection {
        addr: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Bind error on {addr}: {source}")]
    Bind {
        addr: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Timed out waiting for {expected} agents (got {actual}).")]
    AgentWaitTimeout { expected: usize, actual: usize },
    #[error("Connection closed.")]
    ConnectionClosed,
    #[error("Wire message exceeded max size ({max_bytes} bytes).")]
    WireMessageTooLarge { max_bytes: usize },
    #[error("Wire message was not valid UTF-8: {source}")]
    WireMessageInvalidUtf8 {
        #[source]
        source: std::str::Utf8Error,
    },
    #[error("Wire {field} must be >= 1: {source}")]
    WireValueTooSmall {
        field: WireValueField,
        #[source]
        source: ValidationError,
    },
    #[error("Run id mismatch (expected {expected}, got {actual}).")]
    RunIdMismatch { expected: String, actual: String },
    #[error("Received stop before start.")]
    StopBeforeStart,
    #[error("Received config while waiting for start.")]
    ConfigWhileWaitingForStart,
    #[error("Unexpected controller message while running.")]
    UnexpectedControllerMessageWhileRunning,
    #[error("Unexpected message from controller.")]
    UnexpectedMessageFromController,
    #[error("Controller connection closed.")]
    ControllerConnectionClosed,
    #[error("Start received before config.")]
    StartBeforeConfig,
    #[error("Control channel closed.")]
    ControlChannelClosed,
    #[error("Agent event channel closed.")]
    AgentEventChannelClosed,
    #[error("Distributed run completed with errors.")]
    RunCompletedWithErrors,
    #[error("Timed out waiting for agent hello.")]
    AgentHelloTimeout,
    #[error("Expected hello from agent.")]
    ExpectedHelloFromAgent,
    #[error("Invalid auth token.")]
    InvalidAuthToken,
    #[error("Serialization error during {context}: {source}")]
    Serialize {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("Deserialization error during {context}: {source}")]
    Deserialize {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("Remote error: {message}")]
    Remote { message: String },
    #[cfg(test)]
    #[error("Test expectation failed: {message}")]
    TestExpectation { message: &'static str },
    #[cfg(test)]
    #[error("Test expectation failed: {message}: {value}")]
    TestExpectationValue {
        message: &'static str,
        value: String,
    },
}
