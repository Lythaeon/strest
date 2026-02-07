use crate::args::{Scenario, TesterArgs};
use crate::config::apply::parse_scenario;
use crate::config::types::{SCENARIO_SCHEMA_VERSION, ScenarioConfig};
use std::collections::HashMap;
use wasmparser::{
    CompositeType, ConstExpr, DataKind, ExternalKind, FuncType, FunctionBody, MemoryType, Operator,
    Parser, Payload, TableType, ValType,
};

const MAX_WASM_BYTES: usize = 4 * 1024 * 1024;
const MAX_SCENARIO_BYTES: usize = 1024 * 1024;
const MAX_MEMORY_PAGES: u64 = 128;
const MAX_TABLE_ELEMENTS: u32 = 1024;
const WASM_PAGE_SIZE: u64 = 64 * 1024;

struct WasmModuleInfo {
    func_types: Vec<Option<FuncType>>,
    func_type_indices: Vec<u32>,
    func_constants: Vec<Option<i32>>,
    exports: HashMap<String, (ExternalKind, u32)>,
    memories: Vec<MemoryType>,
    tables: Vec<TableType>,
    data_segments: Vec<(u32, u64, Vec<u8>)>,
}

fn validate_wasm_bytes(script_path: &str, wasm_bytes: &[u8]) -> Result<(), String> {
    let metadata = std::fs::metadata(script_path)
        .map_err(|err| format!("Failed to read wasm metadata '{}': {}", script_path, err))?;
    let len = usize::try_from(metadata.len())
        .map_err(|err| format!("Invalid wasm file size: {}", err))?;
    if len > MAX_WASM_BYTES {
        return Err("WASM module is too large.".to_owned());
    }
    if wasm_bytes.len() > MAX_WASM_BYTES {
        return Err("WASM module is too large.".to_owned());
    }
    Ok(())
}

fn parse_module(wasm_bytes: &[u8]) -> Result<WasmModuleInfo, String> {
    let mut info = WasmModuleInfo {
        func_types: Vec::new(),
        func_type_indices: Vec::new(),
        func_constants: Vec::new(),
        exports: HashMap::new(),
        memories: Vec::new(),
        tables: Vec::new(),
        data_segments: Vec::new(),
    };

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.map_err(|err| format!("Failed to parse wasm: {}", err))?;
        match payload {
            Payload::ImportSection(reader) => {
                if reader.count() > 0 {
                    return Err("WASM module must not import any host definitions.".to_owned());
                }
            }
            Payload::TypeSection(reader) => {
                for group in reader {
                    let group = group.map_err(|err| format!("Invalid type section: {}", err))?;
                    for subtype in group.into_types() {
                        let func_ty = match subtype.composite_type {
                            CompositeType::Func(func_ty) => Some(func_ty),
                            _ => None,
                        };
                        info.func_types.push(func_ty);
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for func in reader {
                    let type_index =
                        func.map_err(|err| format!("Invalid function section: {}", err))?;
                    info.func_type_indices.push(type_index);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export =
                        export.map_err(|err| format!("Invalid export section: {}", err))?;
                    info.exports
                        .insert(export.name.to_string(), (export.kind, export.index));
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory =
                        memory.map_err(|err| format!("Invalid memory section: {}", err))?;
                    info.memories.push(memory);
                }
            }
            Payload::TableSection(reader) => {
                for table in reader {
                    let table = table.map_err(|err| format!("Invalid table section: {}", err))?;
                    info.tables.push(table.ty);
                }
            }
            Payload::DataSection(reader) => {
                for data in reader {
                    let data = data.map_err(|err| format!("Invalid data section: {}", err))?;
                    match data.kind {
                        DataKind::Active {
                            memory_index,
                            offset_expr,
                        } => {
                            let offset = eval_const_expr_i64(&offset_expr)?;
                            let offset = u64::try_from(offset).map_err(|_| {
                                "WASM data segment offset must be non-negative.".to_owned()
                            })?;
                            info.data_segments
                                .push((memory_index, offset, data.data.to_vec()));
                        }
                        DataKind::Passive => {
                            return Err("WASM data segments must be active.".to_owned());
                        }
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                let constant = match parse_const_i32(body) {
                    Ok(value) => Some(value),
                    Err(_) => None,
                };
                info.func_constants.push(constant);
            }
            Payload::End(_) => break,
            _ => {}
        }
    }

    if info.func_constants.len() != info.func_type_indices.len() {
        return Err("WASM function section did not match code section.".to_owned());
    }

    Ok(info)
}

fn eval_const_expr_i64(expr: &ConstExpr<'_>) -> Result<i64, String> {
    let mut reader = expr.get_operators_reader();
    let mut value: Option<i64> = None;

    while !reader.eof() {
        let op = reader
            .read()
            .map_err(|err| format!("Invalid const expr: {}", err))?;
        match op {
            Operator::I32Const { value: v } => {
                value = Some(i64::from(v));
            }
            Operator::I64Const { value: v } => {
                value = Some(v);
            }
            Operator::End => break,
            _ => {
                return Err("WASM const expressions must be constant integers.".to_owned());
            }
        }
    }

    if !reader.eof() {
        return Err("Unexpected operators after const expression.".to_owned());
    }

    value.ok_or_else(|| "WASM const expressions must return a value.".to_owned())
}

fn parse_const_i32(body: FunctionBody<'_>) -> Result<i32, String> {
    let mut reader = body
        .get_operators_reader()
        .map_err(|err| format!("Invalid function body: {}", err))?;
    let mut value: Option<i32> = None;

    while !reader.eof() {
        let op = reader
            .read()
            .map_err(|err| format!("Invalid function body: {}", err))?;
        match op {
            Operator::I32Const { value: v } => {
                value = Some(v);
            }
            Operator::End => break,
            Operator::Nop => {}
            _ => {
                return Err("WASM scenario functions must return a constant i32 value.".to_owned());
            }
        }
    }

    if !reader.eof() {
        return Err("Unexpected operators after constant function.".to_owned());
    }

    value.ok_or_else(|| "WASM scenario functions must return a value.".to_owned())
}

fn validate_wasm_memory(memory: &MemoryType) -> Result<(), String> {
    if memory.shared {
        return Err("WASM memory must be non-shared.".to_owned());
    }

    if memory.memory64 {
        return Err("WASM memory must be 32-bit.".to_owned());
    }

    let max_pages = memory
        .maximum
        .ok_or_else(|| "WASM memory must declare a maximum size.".to_owned())?;

    if memory.initial > MAX_MEMORY_PAGES || max_pages > MAX_MEMORY_PAGES {
        return Err("WASM memory exceeds the configured limit.".to_owned());
    }

    Ok(())
}

fn validate_wasm_table(table: &TableType) -> Result<(), String> {
    let max_elements = table.maximum.unwrap_or(table.initial);
    if max_elements > MAX_TABLE_ELEMENTS {
        return Err("WASM table exceeds the configured limit.".to_owned());
    }
    Ok(())
}

fn validate_wasm_scenario(config: &ScenarioConfig) -> Result<(), String> {
    let version = config
        .schema_version
        .ok_or_else(|| "Scenario schema_version is required for WASM scripts.".to_owned())?;
    if version != SCENARIO_SCHEMA_VERSION {
        return Err(format!("Unsupported scenario schema_version {}.", version));
    }

    if config.steps.is_empty() {
        return Err("Scenario must include at least one step.".to_owned());
    }

    if let Some(vars) = config.vars.as_ref()
        && vars.keys().any(|key| key.trim().is_empty())
    {
        return Err("Scenario vars cannot contain empty keys.".to_owned());
    }

    for (idx, step) in config.steps.iter().enumerate() {
        if let Some(status) = step.assert_status
            && !(100..=599).contains(&status)
        {
            return Err(format!(
                "Scenario step {} has invalid assert_status {}.",
                idx.saturating_add(1),
                status
            ));
        }
        if let Some(vars) = step.vars.as_ref()
            && vars.keys().any(|key| key.trim().is_empty())
        {
            return Err(format!(
                "Scenario step {} vars cannot contain empty keys.",
                idx.saturating_add(1)
            ));
        }
    }

    Ok(())
}

fn export_index(info: &WasmModuleInfo, name: &str, kind: ExternalKind) -> Result<u32, String> {
    match info.exports.get(name) {
        Some((export_kind, index)) if *export_kind == kind => Ok(*index),
        Some(_) => Err(format!("WASM export '{}' has the wrong kind.", name)),
        None => Err(format!("WASM module missing '{}' export.", name)),
    }
}

fn scenario_const(info: &WasmModuleInfo, name: &str) -> Result<i32, String> {
    let func_index = export_index(info, name, ExternalKind::Func)?;
    let func_index =
        usize::try_from(func_index).map_err(|_| format!("Invalid function index for {}.", name))?;
    let type_index = info
        .func_type_indices
        .get(func_index)
        .ok_or_else(|| format!("Missing function signature for {}.", name))?;
    let type_index =
        usize::try_from(*type_index).map_err(|_| format!("Invalid function type for {}.", name))?;
    let func_ty = info
        .func_types
        .get(type_index)
        .and_then(|ty| ty.as_ref())
        .ok_or_else(|| format!("{} export must be a function type.", name))?;

    if !func_ty.params().is_empty() {
        return Err(format!("{} must not take parameters.", name));
    }
    let results = func_ty.results();
    if results.len() != 1 || results[0] != ValType::I32 {
        return Err(format!("{} must return a single i32.", name));
    }

    let value = info
        .func_constants
        .get(func_index)
        .ok_or_else(|| format!("Missing function body for {}.", name))?
        .ok_or_else(|| format!("{} must return a constant i32.", name))?;
    Ok(value)
}

pub(crate) fn load_scenario_from_wasm(
    script_path: &str,
    args: &TesterArgs,
) -> Result<Scenario, String> {
    let wasm_bytes = std::fs::read(script_path)
        .map_err(|err| format!("Failed to read wasm script '{}': {}", script_path, err))?;
    validate_wasm_bytes(script_path, &wasm_bytes)?;

    let info = parse_module(&wasm_bytes)?;

    let memory_index = export_index(&info, "memory", ExternalKind::Memory)?;
    let memory_index =
        usize::try_from(memory_index).map_err(|_| "Invalid memory index.".to_owned())?;
    let memory = info
        .memories
        .get(memory_index)
        .ok_or_else(|| "WASM module missing memory export.".to_owned())?;
    validate_wasm_memory(memory)?;

    if let Some((_, table_index)) = info.exports.get("table") {
        let table_index =
            usize::try_from(*table_index).map_err(|_| "Invalid table index.".to_owned())?;
        let table = info
            .tables
            .get(table_index)
            .ok_or_else(|| "WASM module missing table export.".to_owned())?;
        validate_wasm_table(table)?;
    }

    let memory_bytes = memory
        .initial
        .checked_mul(WASM_PAGE_SIZE)
        .ok_or_else(|| "WASM memory size overflow.".to_owned())?;
    let memory_len =
        usize::try_from(memory_bytes).map_err(|_| "WASM memory size overflow.".to_owned())?;
    let mut memory_image = vec![0u8; memory_len];

    for (segment_memory, offset, data) in &info.data_segments {
        if usize::try_from(*segment_memory).ok() != Some(memory_index) {
            return Err("WASM data segments must target the exported memory.".to_owned());
        }
        let offset = usize::try_from(*offset)
            .map_err(|_| "WASM data segment offset overflow.".to_owned())?;
        let end = offset
            .checked_add(data.len())
            .ok_or_else(|| "WASM data segment overflow.".to_owned())?;
        if end > memory_image.len() {
            return Err("WASM data segment exceeds memory size.".to_owned());
        }
        memory_image[offset..end].copy_from_slice(data);
    }

    let ptr = scenario_const(&info, "scenario_ptr")?;
    let len = scenario_const(&info, "scenario_len")?;

    if ptr < 0 || len < 0 {
        return Err("WASM scenario pointer/length must be non-negative.".to_owned());
    }

    let len = usize::try_from(len).map_err(|err| format!("Invalid scenario length: {}", err))?;
    if len > MAX_SCENARIO_BYTES {
        return Err("WASM scenario payload too large.".to_owned());
    }

    let ptr = usize::try_from(ptr).map_err(|err| format!("Invalid scenario pointer: {}", err))?;
    let end = ptr
        .checked_add(len)
        .ok_or_else(|| "WASM scenario pointer overflow.".to_owned())?;
    if end > memory_image.len() {
        return Err("WASM scenario pointer out of bounds.".to_owned());
    }

    let buffer = &memory_image[ptr..end];
    let json = std::str::from_utf8(buffer)
        .map_err(|err| format!("Scenario JSON is not valid UTF-8: {}", err))?;
    let scenario_config: ScenarioConfig =
        serde_json::from_str(json).map_err(|err| format!("Invalid scenario JSON: {}", err))?;
    validate_wasm_scenario(&scenario_config)?;

    parse_scenario(&scenario_config, args)
}

#[cfg(all(test, feature = "wasm"))]
mod tests {
    use super::load_scenario_from_wasm;
    use crate::args::TesterArgs;
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

    fn build_wasm_with_json(json: &str, len_override: Option<i32>) -> Result<Vec<u8>, String> {
        let escaped = escape_wat_string(json);
        let len = len_override.unwrap_or_else(|| i32::try_from(json.len()).unwrap_or(i32::MAX));

        let wat = format!(
            "(module\n  (memory (export \"memory\") 1 2)\n  \
             (data (i32.const 0) \"{escaped}\")\n  \
             (func (export \"scenario_ptr\") (result i32) (i32.const 0))\n  \
             (func (export \"scenario_len\") (result i32) (i32.const {len}))\n)"
        );

        wat::parse_str(&wat).map_err(|err| err.to_string())
    }

    fn default_args() -> Result<TesterArgs, String> {
        TesterArgs::try_parse_from(["strest", "--url", "http://localhost"])
            .map_err(|err| err.to_string())
    }

    #[test]
    fn load_scenario_from_wasm_reads_json() -> Result<(), String> {
        let json = r#"{"schema_version":1,"base_url":"http://example.com","steps":[{"path":"/"}]}"#;
        let wasm_bytes = build_wasm_with_json(json, None)?;

        let mut file = NamedTempFile::new().map_err(|err| err.to_string())?;
        file.write_all(&wasm_bytes).map_err(|err| err.to_string())?;

        let path = file
            .path()
            .to_str()
            .ok_or_else(|| "Temp file path is not UTF-8.".to_owned())?;

        let args = default_args()?;
        let scenario = load_scenario_from_wasm(path, &args)?;
        if scenario.base_url.as_deref() != Some("http://example.com") {
            return Err("Scenario base_url did not match.".to_owned());
        }
        if scenario.steps.len() != 1 {
            return Err("Scenario steps length did not match.".to_owned());
        }
        if scenario.steps.first().and_then(|step| step.path.as_deref()) != Some("/") {
            return Err("Scenario path did not match.".to_owned());
        }

        Ok(())
    }

    #[test]
    fn load_scenario_from_wasm_rejects_negative_length() -> Result<(), String> {
        let json = r#"{"schema_version":1,"steps":[{"path":"/"}]}"#;
        let wasm_bytes = build_wasm_with_json(json, Some(-1))?;

        let mut file = NamedTempFile::new().map_err(|err| err.to_string())?;
        file.write_all(&wasm_bytes).map_err(|err| err.to_string())?;

        let path = file
            .path()
            .to_str()
            .ok_or_else(|| "Temp file path is not UTF-8.".to_owned())?;

        let args = default_args()?;
        let result = load_scenario_from_wasm(path, &args);
        match result {
            Ok(_) => Err("Expected error for negative length.".to_owned()),
            Err(err) => {
                if err.contains("must be non-negative") {
                    Ok(())
                } else {
                    Err(format!("Unexpected error: {}", err))
                }
            }
        }
    }
}
