use clap::ArgMatches;
use clap::parser::ValueSource;

use crate::args::{ConnectToMapping, PositiveU64, PositiveUsize, parse_connect_to, parse_header};
use crate::error::{AppError, AppResult, ConfigError};

pub(super) fn is_cli(matches: &ArgMatches, name: &str) -> bool {
    matches.value_source(name) == Some(ValueSource::CommandLine)
}

pub(super) fn ensure_positive_u64(value: u64, field: &str) -> AppResult<PositiveU64> {
    PositiveU64::try_from(value).map_err(|err| {
        AppError::config(ConfigError::FieldMustBePositive {
            field: field.to_owned(),
            source: err,
        })
    })
}

pub(super) fn ensure_positive_usize(value: usize, field: &str) -> AppResult<PositiveUsize> {
    PositiveUsize::try_from(value).map_err(|err| {
        AppError::config(ConfigError::FieldMustBePositive {
            field: field.to_owned(),
            source: err,
        })
    })
}

pub(super) fn parse_headers(headers: &[String]) -> AppResult<Vec<(String, String)>> {
    let mut parsed = Vec::with_capacity(headers.len());
    for header in headers {
        parsed.push(
            parse_header(header)
                .map_err(|err| AppError::config(ConfigError::InvalidHeader { source: err }))?,
        );
    }
    Ok(parsed)
}

pub(super) fn parse_connect_to_entries(entries: &[String]) -> AppResult<Vec<ConnectToMapping>> {
    let mut parsed = Vec::with_capacity(entries.len());
    for entry in entries {
        parsed.push(
            parse_connect_to(entry)
                .map_err(|err| AppError::config(ConfigError::InvalidConnectTo { source: err }))?,
        );
    }
    Ok(parsed)
}
