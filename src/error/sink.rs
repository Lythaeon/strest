use thiserror::Error;

#[derive(Debug, Error)]
pub enum SinkError {
    #[error("Failed to write line: {source}")]
    WriteLine {
        #[source]
        source: std::fmt::Error,
    },
    #[error("Failed to write Prometheus sink: {source}")]
    WritePrometheus {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to serialize OTel sink: {source}")]
    SerializeOtel {
        #[source]
        source: serde_json::Error,
    },
    #[error("Failed to write OTel sink: {source}")]
    WriteOtel {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to write Influx sink: {source}")]
    WriteInflux {
        #[source]
        source: std::io::Error,
    },
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
