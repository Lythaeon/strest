//! CLI argument types and parsing helpers.
mod cli;
mod defaults;
pub(crate) mod parsers;
#[cfg(test)]
mod test_support;
mod types;

#[cfg(test)]
mod tests;

pub use cli::{CleanupArgs, Command, CompareArgs, TesterArgs};
pub use types::{
    ConnectToMapping, ControllerMode, HttpMethod, HttpVersion, LoadMode, LoadProfile, LoadStage,
    OutputFormat, PositiveU64, PositiveUsize, Protocol, Scenario, ScenarioStep, TimeUnit,
    TlsVersion,
};

pub(crate) use defaults::DEFAULT_USER_AGENT;
#[cfg(test)]
pub(crate) use defaults::{default_charts_path, default_tmp_path};
pub(crate) use parsers::{parse_connect_to, parse_header};
#[cfg(test)]
pub(crate) use test_support::parse_test_args;
