use std::fmt::Write as _;

pub(super) fn write_line(output: &mut String, line: &str) -> Result<(), String> {
    writeln!(output, "{}", line).map_err(|err| format!("Failed to write line: {}", err))
}

pub(super) fn format_x100(value: u64) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}
