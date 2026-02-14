use clap::Parser;

use crate::error::{AppError, AppResult};

use super::TesterArgs;

pub(crate) fn parse_test_args<I, T>(args: I) -> AppResult<TesterArgs>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    TesterArgs::try_parse_from(args).map_err(AppError::from)
}
