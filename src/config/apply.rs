mod distributed;
mod load;
pub(crate) mod scenario;
mod section_basic;
mod section_runtime;
mod section_tail;
mod util;

use clap::ArgMatches;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, ConfigError};

use super::types::ConfigFile;

/// Applies configuration values to CLI arguments.
///
/// # Errors
///
/// Returns an error when config values are invalid or conflict with CLI options.
pub fn apply_config(
    args: &mut TesterArgs,
    matches: &ArgMatches,
    config: &ConfigFile,
) -> AppResult<()> {
    validate_config_conflicts(config)?;
    section_basic::apply_basic_config(args, matches, config)?;
    section_runtime::apply_runtime_config(args, matches, config)?;
    section_tail::apply_tail_config(args, matches, config)?;
    Ok(())
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
