mod distributed;
mod load;
pub(crate) mod scenario;
mod section_basic;
mod section_runtime;
mod section_tail;
mod util;

use std::collections::BTreeMap;

use clap::ArgMatches;

use crate::args::TesterArgs;
use crate::config::types::ScenarioConfig;
use crate::error::{AppError, AppResult, ConfigError};

use super::types::ConfigFile;
use scenario::ScenarioDefaults;

/// Builds config-driven overrides over preset arguments.
///
/// # Errors
///
/// Returns an error when config values are invalid or conflict with CLI options.
pub fn apply_config(
    preset_args: TesterArgs,
    matches: &ArgMatches,
    mut config: ConfigFile,
) -> AppResult<(TesterArgs, Option<BTreeMap<String, ScenarioConfig>>)> {
    validate_config_conflicts(&config)?;

    // Merge order is centralized here and intentionally explicit:
    // command-line values in `preset_args` win, config fills missing values,
    // and untouched fields keep preset defaults.
    let mut effective_args = preset_args;
    section_basic::apply_basic_config(&mut effective_args, matches, &config)?;
    section_runtime::apply_runtime_config(&mut effective_args, matches, &config)?;
    let scenario_defaults = ScenarioDefaults::new(
        effective_args.url.clone(),
        effective_args.method,
        effective_args.data.clone(),
        effective_args.headers.clone(),
    );
    section_tail::apply_tail_config(&mut effective_args, matches, &config, &scenario_defaults)?;

    let scenario_registry = config.scenarios.take();

    Ok((effective_args, scenario_registry))
}

fn validate_config_conflicts(config: &ConfigFile) -> AppResult<()> {
    if config.data.is_some() && (config.data_file.is_some() || config.data_lines.is_some()) {
        return Err(AppError::config(ConfigError::Conflict {
            left: "data",
            right: "data_file/data_lines",
        }));
    }
    if config.data_file.is_some() && config.data_lines.is_some() {
        return Err(AppError::config(ConfigError::Conflict {
            left: "data_file",
            right: "data_lines",
        }));
    }
    if config.form.is_some()
        && (config.data.is_some() || config.data_file.is_some() || config.data_lines.is_some())
    {
        return Err(AppError::config(ConfigError::Conflict {
            left: "form",
            right: "data/data_file/data_lines",
        }));
    }
    if config.load.is_some() && (config.rate.is_some() || config.rpm.is_some()) {
        return Err(AppError::config(ConfigError::Conflict {
            left: "load",
            right: "rate/rpm",
        }));
    }
    Ok(())
}
