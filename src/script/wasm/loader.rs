use wasmparser::{ExternalKind, ValType};

use crate::args::{Scenario, TesterArgs};
use crate::config::apply::parse_scenario;
use crate::config::types::ScenarioConfig;
use crate::error::{AppError, AppResult, ScriptError, WasmError};

use super::constants::{MAX_SCENARIO_BYTES, WASM_PAGE_SIZE};
use super::module::WasmModuleInfo;
use super::parse::parse_module;
use super::validate::{
    validate_wasm_bytes, validate_wasm_memory, validate_wasm_scenario, validate_wasm_table,
};

pub(crate) fn load_scenario_from_wasm(script_path: &str, args: &TesterArgs) -> AppResult<Scenario> {
    let wasm_bytes = std::fs::read(script_path).map_err(|err| {
        AppError::script(ScriptError::ReadWasmScript {
            path: script_path.to_owned(),
            source: err,
        })
    })?;
    validate_wasm_bytes(script_path, &wasm_bytes)?;

    let info = parse_module(&wasm_bytes)?;

    let memory_index = export_index(&info, "memory", ExternalKind::Memory)?;
    let memory_index = usize::try_from(memory_index)
        .map_err(|_err| AppError::script(WasmError::InvalidMemoryIndex))?;
    let memory = info
        .memories
        .get(memory_index)
        .ok_or_else(|| AppError::script(WasmError::MemoryExportMissing))?;
    validate_wasm_memory(memory)?;

    if let Some((_, table_index)) = info.exports.get("table") {
        let table_index = usize::try_from(*table_index)
            .map_err(|_err| AppError::script(WasmError::InvalidTableIndex))?;
        let table = info
            .tables
            .get(table_index)
            .ok_or_else(|| AppError::script(WasmError::TableExportMissing))?;
        validate_wasm_table(table)?;
    }

    let memory_bytes = memory
        .initial
        .checked_mul(WASM_PAGE_SIZE)
        .ok_or_else(|| AppError::script(WasmError::MemorySizeOverflow))?;
    let memory_len = usize::try_from(memory_bytes)
        .map_err(|_err| AppError::script(WasmError::MemorySizeOverflow))?;
    let mut memory_image = vec![0u8; memory_len];

    for (segment_memory, offset, data) in &info.data_segments {
        if usize::try_from(*segment_memory).ok() != Some(memory_index) {
            return Err(AppError::script(WasmError::DataSegmentWrongMemory));
        }
        let offset = usize::try_from(*offset)
            .map_err(|_err| AppError::script(WasmError::DataSegmentOffsetOverflow))?;
        let end = offset
            .checked_add(data.len())
            .ok_or_else(|| AppError::script(WasmError::DataSegmentOverflow))?;
        let target = memory_image
            .get_mut(offset..end)
            .ok_or_else(|| AppError::script(WasmError::DataSegmentOutOfBounds))?;
        target.copy_from_slice(data);
    }

    let ptr = scenario_const(&info, "scenario_ptr")?;
    let len = scenario_const(&info, "scenario_len")?;

    if ptr < 0 || len < 0 {
        return Err(AppError::script(WasmError::ScenarioPointerOrLengthNegative));
    }

    let len = usize::try_from(len)
        .map_err(|err| AppError::script(WasmError::InvalidScenarioLength { source: err }))?;
    if len > MAX_SCENARIO_BYTES {
        return Err(AppError::script(WasmError::ScenarioPayloadTooLarge));
    }

    let ptr = usize::try_from(ptr)
        .map_err(|err| AppError::script(WasmError::InvalidScenarioPointer { source: err }))?;
    let end = ptr
        .checked_add(len)
        .ok_or_else(|| AppError::script(WasmError::ScenarioPointerOverflow))?;
    let buffer = memory_image
        .get(ptr..end)
        .ok_or_else(|| AppError::script(WasmError::ScenarioPointerOutOfBounds))?;
    let json = std::str::from_utf8(buffer)
        .map_err(|err| AppError::script(WasmError::ScenarioJsonInvalidUtf8 { source: err }))?;
    let scenario_config: ScenarioConfig = serde_json::from_str(json)
        .map_err(|err| AppError::script(WasmError::ScenarioJsonInvalid { source: err }))?;
    validate_wasm_scenario(&scenario_config)?;

    parse_scenario(&scenario_config, args).map_err(|err| {
        if let AppError::Config(source) = err {
            AppError::script(ScriptError::ScenarioConfig { source })
        } else {
            err
        }
    })
}

fn export_index(info: &WasmModuleInfo, name: &str, kind: ExternalKind) -> AppResult<u32> {
    match info.exports.get(name) {
        Some((export_kind, index)) if *export_kind == kind => Ok(*index),
        Some(_) => Err(AppError::script(WasmError::ExportWrongKind {
            name: name.to_owned(),
        })),
        None => Err(AppError::script(WasmError::ExportMissing {
            name: name.to_owned(),
        })),
    }
}

fn scenario_const(info: &WasmModuleInfo, name: &str) -> AppResult<i32> {
    let func_index = export_index(info, name, ExternalKind::Func)?;
    let func_index = usize::try_from(func_index).map_err(|_err| {
        AppError::script(WasmError::InvalidFunctionIndex {
            name: name.to_owned(),
        })
    })?;
    let type_index = info.func_type_indices.get(func_index).ok_or_else(|| {
        AppError::script(WasmError::MissingFunctionSignature {
            name: name.to_owned(),
        })
    })?;
    let type_index = usize::try_from(*type_index).map_err(|_err| {
        AppError::script(WasmError::InvalidFunctionType {
            name: name.to_owned(),
        })
    })?;
    let func_ty = info
        .func_types
        .get(type_index)
        .and_then(|ty| ty.as_ref())
        .ok_or_else(|| {
            AppError::script(WasmError::ExportNotFunction {
                name: name.to_owned(),
            })
        })?;

    if !func_ty.params().is_empty() {
        return Err(AppError::script(WasmError::ExportTakesParameters {
            name: name.to_owned(),
        }));
    }
    let results = func_ty.results();
    if results.len() != 1 || results.first() != Some(&ValType::I32) {
        return Err(AppError::script(WasmError::ExportReturnTypeInvalid {
            name: name.to_owned(),
        }));
    }

    let value = info
        .func_constants
        .get(func_index)
        .ok_or_else(|| {
            AppError::script(WasmError::MissingFunctionBody {
                name: name.to_owned(),
            })
        })?
        .ok_or_else(|| {
            AppError::script(WasmError::ExportMustReturnConstI32 {
                name: name.to_owned(),
            })
        })?;
    Ok(value)
}
