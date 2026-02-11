use super::ValidationError;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config '{path}': {source}")]
    ReadConfig {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse TOML config '{path}': {source}")]
    ParseToml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("Failed to parse JSON config '{path}': {source}")]
    ParseJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("Unsupported config extension '{ext}'. Use .toml or .json.")]
    UnsupportedExtension { ext: String },
    #[error("Config file must have .toml or .json extension.")]
    MissingExtension,
    #[error("Config cannot set both '{left}' and '{right}'.")]
    Conflict {
        left: &'static str,
        right: &'static str,
    },
    #[error("Invalid header: {source}")]
    InvalidHeader {
        #[source]
        source: ValidationError,
    },
    #[error("Invalid connect-to entry: {source}")]
    InvalidConnectTo {
        #[source]
        source: ValidationError,
    },
    #[error("Invalid metrics range: {source}")]
    InvalidMetricsRange {
        #[source]
        source: ValidationError,
    },
    #[error("Invalid charts_latency_bucket_ms: {source}")]
    InvalidChartsLatencyBucket {
        #[source]
        source: ValidationError,
    },
    #[error("Config '{field}' must be >= 1: {source}")]
    FieldMustBePositive {
        field: String,
        #[source]
        source: ValidationError,
    },
    #[error("Load profile requires a rate/rpm or at least one stage.")]
    LoadProfileMissingRateOrStages,
    #[error("Config rate/rpm must be >= 1.")]
    RateRpmMustBePositive,
    #[error("Stage {index} must define one of target, rate, or rpm.")]
    StageMissingTargetRateRpm { index: usize },
    #[error("Stage {index} cannot combine target, rate, and rpm.")]
    StageConflictingTargetRateRpm { index: usize },
    #[error("Config '{context}' cannot define both rate and rpm.")]
    RateRpmConflict { context: String },
    #[error("Unsupported scenario schema_version {version}.")]
    UnsupportedScenarioSchema { version: u32 },
    #[error("Scenario must include at least one step.")]
    ScenarioMissingSteps,
    #[error("Scenario step {index} must define url/path or set scenario.base_url.")]
    ScenarioStepMissingUrlOrPath { index: usize },
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
    #[error("Invalid distributed.role '{value}'. Use 'controller' or 'agent'.")]
    InvalidDistributedRole { value: String },
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
