use super::load_scenario_from_wasm;
use crate::args::TesterArgs;
use crate::error::{AppError, AppResult};
use clap::Parser;
use std::fmt::Write as _;
use std::io::Write;
use tempfile::NamedTempFile;

fn escape_wat_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                if write!(out, "\\{:02x}", ch as u32).is_err() {
                    out.push_str("\\00");
                }
            }
            _ => out.push(ch),
        }
    }
    out
}

fn build_wasm_with_json(json: &str, len_override: Option<i32>) -> AppResult<Vec<u8>> {
    let escaped = escape_wat_string(json);
    let len = len_override.unwrap_or_else(|| i32::try_from(json.len()).unwrap_or(i32::MAX));

    let wat = format!(
        "(module\n  (memory (export \"memory\") 1 2)\n  \
             (data (i32.const 0) \"{escaped}\")\n  \
             (func (export \"scenario_ptr\") (result i32) (i32.const 0))\n  \
             (func (export \"scenario_len\") (result i32) (i32.const {len}))\n)"
    );

    wat::parse_str(&wat).map_err(|err| AppError::script(format!("Failed to parse WAT: {}", err)))
}

fn default_args() -> AppResult<TesterArgs> {
    TesterArgs::try_parse_from(["strest", "--url", "http://localhost"])
        .map_err(|err| AppError::script(format!("Failed to parse args: {}", err)))
}

#[test]
fn load_scenario_from_wasm_reads_json() -> AppResult<()> {
    let json = r#"{"schema_version":1,"base_url":"http://example.com","steps":[{"path":"/"}]}"#;
    let wasm_bytes = build_wasm_with_json(json, None)?;

    let mut file = NamedTempFile::new()
        .map_err(|err| AppError::script(format!("tempfile creation failed: {}", err)))?;
    file.write_all(&wasm_bytes)
        .map_err(|err| AppError::script(format!("tempfile write failed: {}", err)))?;

    let path = file
        .path()
        .to_str()
        .ok_or_else(|| AppError::script("Temp file path is not UTF-8."))?;

    let args = default_args()?;
    let scenario = load_scenario_from_wasm(path, &args)?;
    if scenario.base_url.as_deref() != Some("http://example.com") {
        return Err(AppError::script("Scenario base_url did not match."));
    }
    if scenario.steps.len() != 1 {
        return Err(AppError::script("Scenario steps length did not match."));
    }
    if scenario.steps.first().and_then(|step| step.path.as_deref()) != Some("/") {
        return Err(AppError::script("Scenario path did not match."));
    }

    Ok(())
}

#[test]
fn load_scenario_from_wasm_rejects_negative_length() -> AppResult<()> {
    let json = r#"{"schema_version":1,"steps":[{"path":"/"}]}"#;
    let wasm_bytes = build_wasm_with_json(json, Some(-1))?;

    let mut file = NamedTempFile::new()
        .map_err(|err| AppError::script(format!("tempfile creation failed: {}", err)))?;
    file.write_all(&wasm_bytes)
        .map_err(|err| AppError::script(format!("tempfile write failed: {}", err)))?;

    let path = file
        .path()
        .to_str()
        .ok_or_else(|| AppError::script("Temp file path is not UTF-8."))?;

    let args = default_args()?;
    let result = load_scenario_from_wasm(path, &args);
    match result {
        Ok(_) => Err(AppError::script("Expected error for negative length.")),
        Err(err) => {
            if err.to_string().contains("must be non-negative") {
                Ok(())
            } else {
                Err(AppError::script(format!("Unexpected error: {}", err)))
            }
        }
    }
}
