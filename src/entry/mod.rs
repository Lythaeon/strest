mod plan;

use std::ffi::OsString;
use std::path::Path;

use clap::{ArgMatches, CommandFactory, FromArgMatches};

use crate::args::TesterArgs;
use crate::error::AppResult;
use plan::{build_plan, execute_plan};

/// Default config filenames checked when no CLI args are provided.
const DEFAULT_CONFIG_FILES: [&str; 2] = ["strest.toml", "strest.json"];

pub(crate) fn run() -> AppResult<()> {
    let (args, matches) = match parse_args()? {
        Some(parsed) => parsed,
        None => return Ok(()),
    };

    crate::system::logger::init_logging(args.verbose, args.no_color);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(run_async(args, &matches))
}

fn parse_args() -> AppResult<Option<(TesterArgs, ArgMatches)>> {
    let mut cmd = TesterArgs::command();
    let raw_args: Vec<OsString> = std::env::args_os().collect();

    if should_show_help(&raw_args) {
        cmd.print_help()?;
        println!();
        return Ok(None);
    }

    let matches = cmd.get_matches_from(raw_args);
    let args = TesterArgs::from_arg_matches(&matches)?;

    Ok(Some((args, matches)))
}

fn should_show_help(raw_args: &[OsString]) -> bool {
    let treat_as_empty =
        matches!(raw_args, [] | [_]) || matches!(raw_args, [_, second] if second == "--");
    if !treat_as_empty {
        return false;
    }

    !has_default_config()
}

fn has_default_config() -> bool {
    DEFAULT_CONFIG_FILES
        .iter()
        .any(|path| Path::new(path).exists())
}

async fn run_async(args: TesterArgs, matches: &ArgMatches) -> AppResult<()> {
    let plan = build_plan(args, matches)?;
    execute_plan(plan).await
}
