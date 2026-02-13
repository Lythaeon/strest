use wasmparser::{Parser, Payload};

use crate::error::{AppError, AppResult, ScriptError};

use super::constants::MAX_PLUGIN_WASM_BYTES;

pub(super) fn validate_plugin_module(plugin_path: &str, wasm_bytes: &[u8]) -> AppResult<()> {
    let metadata = std::fs::metadata(plugin_path).map_err(|err| {
        AppError::script(ScriptError::WasmPlugin {
            message: format!("failed to read plugin metadata '{}': {}", plugin_path, err),
        })
    })?;

    let file_len = usize::try_from(metadata.len()).map_err(|err| {
        AppError::script(ScriptError::WasmPlugin {
            message: format!("invalid plugin file size for '{}': {}", plugin_path, err),
        })
    })?;
    if file_len > MAX_PLUGIN_WASM_BYTES || wasm_bytes.len() > MAX_PLUGIN_WASM_BYTES {
        return Err(AppError::script(ScriptError::WasmPlugin {
            message: format!(
                "plugin '{}' exceeds max size limit ({} bytes)",
                plugin_path, MAX_PLUGIN_WASM_BYTES
            ),
        }));
    }

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.map_err(|err| {
            AppError::script(ScriptError::WasmPlugin {
                message: format!("failed to parse plugin '{}': {}", plugin_path, err),
            })
        })?;

        if let Payload::ImportSection(reader) = payload {
            for import in reader {
                let import = import.map_err(|err| {
                    AppError::script(ScriptError::WasmPlugin {
                        message: format!(
                            "invalid import section in plugin '{}': {}",
                            plugin_path, err
                        ),
                    })
                })?;
                if import.module != "wasi_snapshot_preview1" {
                    return Err(AppError::script(ScriptError::WasmPlugin {
                        message: format!(
                            "plugin '{}' imports unsupported module '{}'",
                            plugin_path, import.module
                        ),
                    }));
                }
            }
        }
    }

    Ok(())
}
