use super::*;
use crate::args::parsers::parse_bool_env;
use crate::error::{AppError, AppResult};
use clap::Parser;
use std::time::Duration;
use tempfile::tempdir;

mod defaults;
mod headers;
mod options_core;
mod options_extra;
mod subcommands;
