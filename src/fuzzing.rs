use std::collections::BTreeMap;
use std::time::Duration;

use clap::{ArgMatches, CommandFactory, FromArgMatches};
use rand::distributions::Distribution;
use rand::thread_rng;

use crate::args::{
    PositiveU64, PositiveUsize, Scenario, ScenarioStep, TesterArgs, TlsVersion, parse_header,
    parsers::parse_duration_arg,
};
use crate::config::apply::parse_scenario;
use crate::config::types::{ConfigFile, LoadConfig, ScenarioConfig};
use crate::config::{apply_config, parse_duration_value};
use crate::http::workload::render_template;
use crate::http::workload::{StepRequestContext, build_step_request, build_template_vars};
use crate::metrics::MetricsRange;
use reqwest::Client;

thread_local! {
    static BASE_MATCHES: ArgMatches = TesterArgs::command().get_matches_from(["strest"]);
}

/// Parses a header string in `Key: Value` format.
///
/// # Errors
///
/// Returns an error when the header is malformed.
pub fn parse_header_input(input: &str) -> Result<(String, String), String> {
    parse_header(input)
}

/// Parses a duration argument (e.g. `10s`, `500ms`).
///
/// # Errors
///
/// Returns an error when the duration is invalid.
pub fn parse_duration_arg_input(input: &str) -> Result<Duration, String> {
    parse_duration_arg(input)
}

/// Parses a duration value from config.
///
/// # Errors
///
/// Returns an error when the duration is invalid.
pub fn parse_duration_value_input(input: &str) -> Result<Duration, String> {
    parse_duration_value(input)
}

/// Parses a TLS version (e.g. `1.2`, `1.3`).
///
/// # Errors
///
/// Returns an error when the version is invalid.
pub fn parse_tls_version_input(input: &str) -> Result<TlsVersion, String> {
    input.parse::<TlsVersion>()
}

/// Parses a metrics range in `start-end` format.
///
/// # Errors
///
/// Returns an error when the range is invalid.
pub fn parse_metrics_range_input(input: &str) -> Result<MetricsRange, String> {
    input.parse::<MetricsRange>()
}

#[must_use]
pub fn render_template_input(input: &str, vars: &BTreeMap<String, String>) -> String {
    render_template(input, vars)
}

/// Parses TOML config and applies it to defaults.
///
/// # Errors
///
/// Returns an error when parsing or validation fails.
pub fn apply_config_from_toml(input: &str) -> Result<(), String> {
    let config: ConfigFile = toml::from_str(input).map_err(|err| err.to_string())?;
    apply_config_to_defaults(&config)
}

/// Parses JSON config and applies it to defaults.
///
/// # Errors
///
/// Returns an error when parsing or validation fails.
pub fn apply_config_from_json(input: &[u8]) -> Result<(), String> {
    let config: ConfigFile = serde_json::from_slice(input).map_err(|err| err.to_string())?;
    apply_config_to_defaults(&config)
}

/// Parses a positive u64 string value.
///
/// # Errors
///
/// Returns an error when the value is invalid or zero.
pub fn parse_positive_u64_input(input: &str) -> Result<u64, String> {
    let value: PositiveU64 = input.parse()?;
    Ok(value.get())
}

/// Parses a positive usize string value.
///
/// # Errors
///
/// Returns an error when the value is invalid or zero.
pub fn parse_positive_usize_input(input: &str) -> Result<usize, String> {
    let value: PositiveUsize = input.parse()?;
    Ok(value.get())
}

/// Compiles a rand_regex pattern with a max_repeat hint.
///
/// # Errors
///
/// Returns an error when the regex pattern is invalid.
pub fn compile_rand_regex_input(pattern: &str, max_repeat: u32) -> Result<(), String> {
    let regex = rand_regex::Regex::compile(pattern, max_repeat)
        .map_err(|err| format!("Invalid rand-regex pattern: {}", err))?;
    let _sample: String = rand::thread_rng().sample(&regex);
    Ok(())
}

/// Parses a multipart form entry (name=value or name=@path).
///
/// # Errors
///
/// Returns an error when the entry is malformed.
pub fn parse_form_entry_input(input: &str) -> Result<(), String> {
    let (name, value) = input
        .split_once('=')
        .ok_or_else(|| "Expected form entry format name=value.".to_owned())?;
    let name = name.trim();
    if name.is_empty() {
        return Err("Form field name must not be empty.".to_owned());
    }
    let value = value.trim();
    if let Some(path) = value.strip_prefix('@') {
        if path.is_empty() {
            return Err("Form file path must not be empty.".to_owned());
        }
        return Ok(());
    }
    Ok(())
}

/// Parses a load profile from a config block.
///
/// # Errors
///
/// Returns an error when parsing or validation fails.
pub fn apply_load_config_input(load: LoadConfig) -> Result<(), String> {
    let config = ConfigFile {
        load: Some(load),
        ..ConfigFile::default()
    };
    apply_config_to_defaults(&config)
}

/// Parses a scenario config using default arguments.
///
/// # Errors
///
/// Returns an error when scenario parsing fails.
pub fn parse_scenario_config_input(config: &ScenarioConfig) -> Result<(), String> {
    BASE_MATCHES.with(|matches| {
        let args = TesterArgs::from_arg_matches(matches)
            .map_err(|err| format!("parse args failed: {}", err))?;
        parse_scenario(config, &args).map(|_| ())
    })
}

/// Build a scenario request to exercise URL resolution and template rendering.
///
/// # Errors
///
/// Returns an error when request construction fails.
pub fn build_scenario_request_input(
    scenario: &Scenario,
    step: &ScenarioStep,
    seq: u64,
    step_index: usize,
) -> Result<(), String> {
    let client = Client::new();
    let vars = build_template_vars(scenario, step, seq, step_index);
    build_step_request(
        &client,
        scenario,
        step,
        &vars,
        &StepRequestContext {
            connect_to: &[],
            host_header: None,
            auth: None,
        },
    )?;
    Ok(())
}

/// Loads a config file from disk to exercise extension handling.
///
/// # Errors
///
/// Returns an error when the config file cannot be read or parsed.
pub fn load_config_file_input(path: &std::path::Path) -> Result<(), String> {
    crate::config::load_config_file(path).map(|_| ())
}

fn apply_config_to_defaults(config: &ConfigFile) -> Result<(), String> {
    BASE_MATCHES.with(|matches| {
        let mut args = TesterArgs::from_arg_matches(matches)
            .map_err(|err| format!("parse args failed: {}", err))?;
        apply_config(&mut args, matches, config)
    })
}
