mod section_runtime_network;
mod section_runtime_output;

use clap::ArgMatches;

use crate::args::TesterArgs;
use crate::error::AppResult;

use super::super::types::ConfigFile;

pub(super) fn apply_runtime_config(
    args: &mut TesterArgs,
    matches: &ArgMatches,
    config: &ConfigFile,
) -> AppResult<()> {
    section_runtime_output::apply_runtime_output_config(args, matches, config)?;
    section_runtime_network::apply_runtime_network_config(args, matches, config)?;
    Ok(())
}
