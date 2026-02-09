#![cfg(feature = "wasm")]

mod support_single;

use std::fmt::Write as _;
use std::io::Write;

use support_single::{run_strest, spawn_http_server_or_skip};
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

fn build_wasm_with_json(json: &str) -> Result<Vec<u8>, String> {
    let escaped = escape_wat_string(json);
    let len = i32::try_from(json.len()).map_err(|err| err.to_string())?;

    let wat = format!(
        "(module\n  (memory (export \"memory\") 1 2)\n  \
         (data (i32.const 0) \"{escaped}\")\n  \
         (func (export \"scenario_ptr\") (result i32) (i32.const 0))\n  \
         (func (export \"scenario_len\") (result i32) (i32.const {len}))\n)"
    );

    wat::parse_str(&wat).map_err(|err| err.to_string())
}

#[test]
fn e2e_wasm_script_runs() -> Result<(), String> {
    let Some((url, _server)) = spawn_http_server_or_skip()? else {
        return Ok(());
    };
    let json = format!(r#"{{"schema_version":1,"base_url":"{url}","steps":[{{"path":"/"}}]}}"#);
    let wasm_bytes = build_wasm_with_json(&json)?;

    let mut file = NamedTempFile::new().map_err(|err| err.to_string())?;
    file.write_all(&wasm_bytes).map_err(|err| err.to_string())?;

    let path = file
        .path()
        .to_str()
        .ok_or_else(|| "Temp file path is not UTF-8.".to_owned())?;

    let args = vec![
        "--script".to_owned(),
        path.to_owned(),
        "-t".to_owned(),
        "1".to_owned(),
        "--no-tui".to_owned(),
        "--summary".to_owned(),
        "--no-charts".to_owned(),
        "--max-tasks".to_owned(),
        "1".to_owned(),
    ];

    let output = run_strest(args)?;
    if !output.status.success() {
        return Err(format!(
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}
