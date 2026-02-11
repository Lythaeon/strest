use std::path::PathBuf;

use super::AppError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("URL list was empty.")]
    UrlListEmpty,
    #[error("Failed to clone request for initial test.")]
    CloneRequestFailed,
    #[error("Test request failed: {source}")]
    TestRequestFailed {
        #[source]
        source: reqwest::Error,
    },
    #[error("Scenario has no steps.")]
    ScenarioHasNoSteps,
    #[error("Scenario preflight failed: {source}")]
    ScenarioPreflightFailed {
        #[source]
        source: Box<AppError>,
    },
    #[error("Invalid URL '{url}': {source}")]
    InvalidUrl {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("Body lines file was empty.")]
    BodyLinesEmpty,
    #[error("Failed to build request: {source}")]
    BuildRequestFailed {
        #[source]
        source: reqwest::Error,
    },
    #[error("Failed to read form file '{path}': {source}")]
    ReadFormFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Invalid connect-to host: {source}")]
    InvalidConnectToHost {
        #[source]
        source: url::ParseError,
    },
    #[error("Invalid connect-to port.")]
    InvalidConnectToPort,
    #[error("Failed to build sigv4 params: {source}")]
    SigV4Params {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to build sigv4 request: {source}")]
    SigV4Request {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to sign request: {source}")]
    SigV4Sign {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to build sign request: {source}")]
    SigV4BuildSign {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Invalid scenario url '{url}': {source}")]
    InvalidScenarioUrl {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("Scenario step missing url/path.")]
    ScenarioStepMissingUrlOrPath,
    #[error("Scenario base_url is required for relative paths.")]
    ScenarioBaseUrlRequired,
    #[error("Invalid scenario base_url '{url}': {source}")]
    InvalidScenarioBaseUrl {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("Failed to join URL '{url}': {source}")]
    JoinUrlFailed {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("No addresses resolved for {host}.")]
    NoAddressesResolved { host: String },
    #[error("Failed to read cacert '{path}': {source}")]
    ReadCacert {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Invalid cacert '{path}': {source}")]
    InvalidCacert {
        path: PathBuf,
        #[source]
        source: reqwest::Error,
    },
    #[error("Failed to read cert '{path}': {source}")]
    ReadCert {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to read key '{path}': {source}")]
    ReadKey {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Invalid cert/key: {source}")]
    InvalidIdentity {
        #[source]
        source: reqwest::Error,
    },
    #[error("Failed to build HTTP client: {source}")]
    BuildClientFailed {
        #[source]
        source: reqwest::Error,
    },
    #[error("Invalid URL source for static workload.")]
    InvalidUrlSourceForStaticWorkload,
    #[error("Invalid body source for static workload.")]
    InvalidBodySourceForStaticWorkload,
    #[error("Failed to read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to read URL file '{path}': {source}")]
    ReadUrlFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("URL file '{path}' was empty.")]
    UrlFileEmpty { path: PathBuf },
    #[error("Failed to resolve {host}:{port} ({source})")]
    ResolveHost {
        host: String,
        port: u16,
        #[source]
        source: std::io::Error,
    },
}
