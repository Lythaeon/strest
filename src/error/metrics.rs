use thiserror::Error;

#[derive(Debug, Error)]
pub enum MetricsError {
    #[error("I/O error during {context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("Histogram error during {context}: {source}")]
    Histogram {
        context: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("{context}: {source}")]
    External {
        context: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("No metrics records found for replay.")]
    ReplayRecordsEmpty,
    #[error("Tmp path is not a file or directory.")]
    ReplayTmpPathInvalid,
    #[error("No metrics logs found in tmp directory.")]
    ReplayTmpNoLogs,
    #[cfg(feature = "alloc-profiler")]
    #[error("jemalloc profiling not compiled (config.prof=false)")]
    ProfilerNotCompiled,
    #[cfg(feature = "alloc-profiler")]
    #[error("jemalloc profiling disabled (opt.prof=false). Set MALLOC_CONF=prof:true")]
    ProfilerDisabled,
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
