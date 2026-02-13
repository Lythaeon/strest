use std::io::Write;

use tempfile::NamedTempFile;

use crate::error::AppResult;

use super::validate::validate_plugin_module;

fn write_wasm_module(wat: &str) -> AppResult<(NamedTempFile, String, Vec<u8>)> {
    let wasm_bytes = wat::parse_str(wat)
        .map_err(|err| crate::error::AppError::script(format!("failed to parse WAT: {}", err)))?;

    let mut file = NamedTempFile::new().map_err(|err| {
        crate::error::AppError::script(format!("failed to create tempfile: {}", err))
    })?;
    file.write_all(&wasm_bytes).map_err(|err| {
        crate::error::AppError::script(format!("failed to write tempfile: {}", err))
    })?;
    let path = file
        .path()
        .to_str()
        .ok_or_else(|| crate::error::AppError::script("temp path is not valid UTF-8"))?
        .to_owned();
    Ok((file, path, wasm_bytes))
}

#[test]
fn validate_plugin_allows_wasi_imports() -> AppResult<()> {
    let wat = r#"
        (module
          (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
          (func (export "_start")
            i32.const 0
            call $proc_exit))
    "#;
    let (_file, path, wasm_bytes) = write_wasm_module(wat)?;
    validate_plugin_module(&path, &wasm_bytes)?;
    Ok(())
}

#[test]
fn validate_plugin_rejects_non_wasi_imports() -> AppResult<()> {
    let wat = r#"
        (module
          (import "env" "log" (func $log (param i32))))
    "#;
    let (_file, path, wasm_bytes) = write_wasm_module(wat)?;
    let result = validate_plugin_module(&path, &wasm_bytes);
    if result.is_ok() {
        return Err(crate::error::AppError::script(
            "expected non-wasi import validation to fail",
        ));
    }
    Ok(())
}
