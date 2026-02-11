use wasmparser::{MemoryType, TableType};

use crate::config::types::{SCENARIO_SCHEMA_VERSION, ScenarioConfig};
use crate::error::{AppError, AppResult, WasmError};

use super::constants::{MAX_MEMORY_PAGES, MAX_TABLE_ELEMENTS, MAX_WASM_BYTES};

pub(super) fn validate_wasm_bytes(script_path: &str, wasm_bytes: &[u8]) -> AppResult<()> {
    let metadata = std::fs::metadata(script_path).map_err(|err| {
        AppError::script(WasmError::ReadMetadata {
            path: script_path.to_owned(),
            source: err,
        })
    })?;
    let len = usize::try_from(metadata.len())
        .map_err(|err| AppError::script(WasmError::InvalidFileSize { source: err }))?;
    if len > MAX_WASM_BYTES {
        return Err(AppError::script(WasmError::ModuleTooLarge));
    }
    if wasm_bytes.len() > MAX_WASM_BYTES {
        return Err(AppError::script(WasmError::ModuleTooLarge));
    }
    Ok(())
}

pub(super) fn validate_wasm_memory(memory: &MemoryType) -> AppResult<()> {
    if memory.shared {
        return Err(AppError::script(WasmError::MemoryShared));
    }

    if memory.memory64 {
        return Err(AppError::script(WasmError::Memory64Bit));
    }

    let max_pages = memory
        .maximum
        .ok_or_else(|| AppError::script(WasmError::MemoryMissingMax))?;

    if memory.initial > MAX_MEMORY_PAGES || max_pages > MAX_MEMORY_PAGES {
        return Err(AppError::script(WasmError::MemoryExceedsLimit));
    }

    Ok(())
}

pub(super) fn validate_wasm_table(table: &TableType) -> AppResult<()> {
    let max_elements = table.maximum.unwrap_or(table.initial);
    if max_elements > MAX_TABLE_ELEMENTS {
        return Err(AppError::script(WasmError::TableExceedsLimit));
    }
    Ok(())
}

pub(super) fn validate_wasm_scenario(config: &ScenarioConfig) -> AppResult<()> {
    let version = config
        .schema_version
        .ok_or_else(|| AppError::script(WasmError::ScenarioSchemaMissing))?;
    if version != SCENARIO_SCHEMA_VERSION {
        return Err(AppError::script(WasmError::ScenarioSchemaUnsupported {
            version,
        }));
    }

    if config.steps.is_empty() {
        return Err(AppError::script(WasmError::ScenarioMissingSteps));
    }

    if let Some(vars) = config.vars.as_ref()
        && vars.keys().any(|key| key.trim().is_empty())
    {
        return Err(AppError::script(WasmError::ScenarioVarsEmptyKey));
    }

    for (idx, step) in config.steps.iter().enumerate() {
        if let Some(status) = step.assert_status
            && !(100..=599).contains(&status)
        {
            return Err(AppError::script(
                WasmError::ScenarioStepInvalidAssertStatus {
                    step: idx.saturating_add(1),
                    status,
                },
            ));
        }
        if let Some(vars) = step.vars.as_ref()
            && vars.keys().any(|key| key.trim().is_empty())
        {
            return Err(AppError::script(WasmError::ScenarioStepVarsEmptyKey {
                step: idx.saturating_add(1),
            }));
        }
    }

    Ok(())
}
