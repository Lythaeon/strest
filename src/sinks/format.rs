use std::fmt::Write as _;

use crate::error::{AppError, AppResult, SinkError};

pub(super) fn write_line(output: &mut String, line: &str) -> AppResult<()> {
    writeln!(output, "{}", line).map_err(|err| AppError::sink(SinkError::WriteLine { source: err }))
}

pub(super) fn format_x100(value: u64) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}
