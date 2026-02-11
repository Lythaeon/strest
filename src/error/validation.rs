use thiserror::Error;
#[derive(Debug, Error, Clone, Copy)]
pub enum ConnectToPortKind {
    #[error("source")]
    Source,
    #[error("target")]
    Target,
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid header format: '{value}'. Expected 'Key: Value'")]
    InvalidHeaderFormat { value: String },
    #[error("Invalid boolean '{value}'. Expected true/false, yes/no, on/off, or 1/0.")]
    InvalidBoolean { value: String },
    #[error(
        "Invalid connect-to '{value}'. Expected 'source_host:source_port:target_host:target_port'."
    )]
    InvalidConnectToFormat { value: String },
    #[error("Invalid connect-to '{value}'.")]
    InvalidConnectTo { value: String },
    #[error("Invalid {kind} port in '{value}': {source}")]
    InvalidConnectToPort {
        value: String,
        kind: ConnectToPortKind,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("Invalid connect-to '{value}'. Host must not be empty.")]
    ConnectToHostEmpty { value: String },
    #[error("Duration must not be empty.")]
    DurationEmpty,
    #[error("Invalid duration '{value}'.")]
    InvalidDurationFormat { value: String },
    #[error("Invalid duration '{value}': {source}")]
    InvalidDurationNumber {
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("Duration overflow.")]
    DurationOverflow,
    #[error("Invalid duration unit '{unit}'.")]
    InvalidDurationUnit { unit: String },
    #[error("Duration must be > 0.")]
    DurationZero,
    #[error("Invalid older-than duration.")]
    InvalidOlderThanDuration,
    #[error("Invalid HTTP version '{value}'. Use 0.9, 1.0, 1.1, 2, or 3.")]
    InvalidHttpVersion { value: String },
    #[error("Invalid controller mode '{value}'. Use auto or manual.")]
    InvalidControllerMode { value: String },
    #[error("Invalid TLS version '{value}'. Use 1.0, 1.1, 1.2, or 1.3.")]
    InvalidTlsVersion { value: String },
    #[error("Value must be >= {min}.")]
    ValueTooSmall { min: u64 },
    #[error("Invalid value: {source}")]
    InvalidNumber {
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("Cannot run as controller and agent at the same time.")]
    ControllerAgentConflict,
    #[error("Cannot combine --script with scenario config.")]
    ScriptScenarioConflict,
    #[error(
        "Disabling the default User-Agent requires --authorized (or config authorized = true)."
    )]
    NoUserAgentWithoutAuthorization,
    #[error("Missing URL (set --url or provide in config).")]
    MissingUrl,
    #[error("Runtime errors occurred.")]
    RuntimeErrors,
    #[error("`--output-format` requires `--output`.")]
    OutputFormatRequiresOutput,
    #[error("`--output` cannot be combined with export flags.")]
    OutputWithExportFlags,
    #[error("`--db-url` requires `--log-shards 1`.")]
    DbUrlRequiresSingleShard,
    #[error("--dump-urls cannot be used with scenarios.")]
    DumpUrlsWithScenario,
    #[error("--dump-urls requires --rand-regex-url.")]
    DumpUrlsRequiresRandRegex,
    #[error("--dump-urls requires a count.")]
    DumpUrlsRequiresCount,
    #[error("Invalid rand-regex pattern '{pattern}': {source}")]
    InvalidRandRegex {
        pattern: String,
        #[source]
        source: rand_regex::Error,
    },
    #[error("tls-min must be <= tls-max.")]
    TlsMinGreaterThanMax,
    #[error("ALPN includes h3, but http3 is not enabled.")]
    AlpnH3WithoutHttp3,
    #[error("Cannot enable http2 and http3 at the same time.")]
    Http2Http3Conflict,
    #[error("Cannot enable http3 while ALPN is restricted to http/1.1.")]
    Http3WithHttp1OnlyAlpn,
    #[error("Cannot enable http3 while ALPN is restricted to h2.")]
    Http3WithH2OnlyAlpn,
    #[error(
        "HTTP/3 support is not enabled in this build. Rebuild with --features http3 and set \
RUSTFLAGS=\"--cfg reqwest_unstable\"."
    )]
    Http3NotEnabled,
    #[error("Cannot enable http2 while ALPN is set to http/1.1 only.")]
    Http2WithHttp1OnlyAlpn,
    #[error("Unsupported ALPN protocol '{protocol}'. Use h2, http/1.1, or h3.")]
    UnsupportedAlpnProtocol { protocol: String },
    #[error("Cannot enable both ipv4 and ipv6 only modes.")]
    Ipv4Ipv6Conflict,
    #[error("proxy-http2 conflicts with proxy-http-version.")]
    ProxyHttp2Conflict,
    #[error("URL generation flags are not supported with scenarios.")]
    ScenarioUrlGenerationConflict,
    #[error("Cannot combine --urls-from-file with --rand-regex-url.")]
    UrlsFromFileAndRandRegexConflict,
    #[error("Multipart form uploads are not supported with AWS SigV4 signing.")]
    SigV4FormUnsupported,
    #[error("--cert requires --key.")]
    CertRequiresKey,
    #[error("--key requires --cert.")]
    KeyRequiresCert,
    #[error("Invalid proxy header name '{header}': {source}")]
    InvalidProxyHeaderName {
        header: String,
        #[source]
        source: http::header::InvalidHeaderName,
    },
    #[error("Invalid proxy header value for '{header}': {source}")]
    InvalidProxyHeaderValue {
        header: String,
        #[source]
        source: http::header::InvalidHeaderValue,
    },
    #[error("Invalid proxy URL '{url}': {source}")]
    InvalidProxyUrl {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("proxy http version 3 is not supported.")]
    ProxyHttpVersionUnsupported,
    #[error("Invalid URL '{url}': {source}")]
    InvalidUrl {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("URL is missing host.")]
    UrlMissingHost,
    #[error("Invalid base_url '{url}': {source}")]
    InvalidBaseUrl {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("Scenario base_url is missing host.")]
    ScenarioBaseUrlMissingHost,
    #[error("Invalid scenario url '{url}': {source}")]
    InvalidScenarioUrl {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("Scenario url is missing host.")]
    ScenarioUrlMissingHost,
    #[error("Invalid form entry '{entry}'. Expected 'name=value' or 'name=@path'.")]
    InvalidFormEntryFormat { entry: String },
    #[error("Invalid form entry '{entry}'. Field name must not be empty.")]
    FormEntryNameEmpty { entry: String },
    #[error("Invalid form entry '{entry}'. File path must not be empty.")]
    FormEntryPathEmpty { entry: String },
    #[error("--aws-sigv4 requires --basic-auth.")]
    AwsSigv4RequiresBasicAuth,
    #[error("--aws-session requires --aws-sigv4.")]
    AwsSessionRequiresSigv4,
    #[error("Expected format username:password.")]
    AuthPairInvalidFormat,
    #[error("Auth username must not be empty.")]
    AuthUsernameEmpty,
    #[error("Invalid aws-sigv4 format. Expected aws:amz:region:service.")]
    AwsSigv4InvalidFormat,
    #[error("aws-sigv4 region/service must not be empty.")]
    AwsSigv4EmptyRegionOrService,
    #[error("Expected format start-end (e.g., 10-30)")]
    MetricsRangeFormat,
    #[error("Invalid start value: {source}")]
    MetricsRangeInvalidStart {
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("Invalid end value: {source}")]
    MetricsRangeInvalidEnd {
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("Start must be <= end")]
    MetricsRangeStartAfterEnd,
    #[error("Failed to build runtime: {source}")]
    RuntimeBuildFailed {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to send shutdown")]
    ShutdownSendFailed,
    #[error("Shutdown task join error: {source}")]
    ShutdownJoinFailed {
        #[source]
        source: tokio::task::JoinError,
    },
    #[error("Replay start must be <= replay end.")]
    ReplayStartAfterEnd,
    #[error("Replay snapshot interval must be >= 1ms.")]
    ReplaySnapshotIntervalTooSmall,
    #[error("Replay snapshot start must be <= replay snapshot end.")]
    ReplaySnapshotStartAfterEnd,
    #[error("Provide only one of --export-csv, --export-json, or --export-jsonl for replay.")]
    ReplayExportSourceConflict,
    #[error("Unsupported snapshot format '{value}'. Expected json, jsonl, or csv.")]
    InvalidSnapshotFormat { value: String },
    #[error("Test expectation failed: {message}")]
    TestExpectation { message: &'static str },
    #[error("Test expectation failed: {message}: {value}")]
    TestExpectationValue {
        message: &'static str,
        value: String,
    },
}
