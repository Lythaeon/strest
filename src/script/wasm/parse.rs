use wasmparser::{CompositeType, ConstExpr, DataKind, FunctionBody, Operator, Parser, Payload};

use crate::error::{AppError, AppResult, WasmError, WasmSection};

use super::module::WasmModuleInfo;

pub(super) fn parse_module(wasm_bytes: &[u8]) -> AppResult<WasmModuleInfo> {
    let mut info = WasmModuleInfo {
        func_types: Vec::new(),
        func_type_indices: Vec::new(),
        func_constants: Vec::new(),
        exports: std::collections::HashMap::new(),
        memories: Vec::new(),
        tables: Vec::new(),
        data_segments: Vec::new(),
    };

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.map_err(|err| AppError::script(WasmError::Parse { source: err }))?;
        // We only care about sections that impact scenario extraction.
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "Other payload sections are intentionally ignored."
        )]
        match payload {
            Payload::ImportSection(reader) => {
                if reader.count() > 0 {
                    return Err(AppError::script(WasmError::ImportsNotAllowed));
                }
            }
            Payload::TypeSection(reader) => {
                for group in reader {
                    let group = group.map_err(|err| {
                        AppError::script(WasmError::InvalidSection {
                            section: WasmSection::Type,
                            source: err,
                        })
                    })?;
                    for subtype in group.into_types() {
                        let func_ty = match subtype.composite_type {
                            CompositeType::Func(func_ty) => Some(func_ty),
                            CompositeType::Array(_) | CompositeType::Struct(_) => None,
                        };
                        info.func_types.push(func_ty);
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for func in reader {
                    let type_index = func.map_err(|err| {
                        AppError::script(WasmError::InvalidSection {
                            section: WasmSection::Function,
                            source: err,
                        })
                    })?;
                    info.func_type_indices.push(type_index);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(|err| {
                        AppError::script(WasmError::InvalidSection {
                            section: WasmSection::Export,
                            source: err,
                        })
                    })?;
                    info.exports
                        .insert(export.name.to_owned(), (export.kind, export.index));
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.map_err(|err| {
                        AppError::script(WasmError::InvalidSection {
                            section: WasmSection::Memory,
                            source: err,
                        })
                    })?;
                    info.memories.push(memory);
                }
            }
            Payload::TableSection(reader) => {
                for table in reader {
                    let table = table.map_err(|err| {
                        AppError::script(WasmError::InvalidSection {
                            section: WasmSection::Table,
                            source: err,
                        })
                    })?;
                    info.tables.push(table.ty);
                }
            }
            Payload::DataSection(reader) => {
                for data in reader {
                    let data = data.map_err(|err| {
                        AppError::script(WasmError::InvalidSection {
                            section: WasmSection::Data,
                            source: err,
                        })
                    })?;
                    match data.kind {
                        DataKind::Active {
                            memory_index,
                            offset_expr,
                        } => {
                            let offset = eval_const_expr_i64(&offset_expr)?;
                            let offset = u64::try_from(offset).map_err(|_err| {
                                AppError::script(WasmError::DataSegmentOffsetNegative)
                            })?;
                            info.data_segments
                                .push((memory_index, offset, data.data.to_vec()));
                        }
                        DataKind::Passive => {
                            return Err(AppError::script(WasmError::DataSegmentsMustBeActive));
                        }
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                let constant = parse_const_i32(&body).ok();
                info.func_constants.push(constant);
            }
            Payload::End(_) => break,
            _ => {}
        }
    }

    if info.func_constants.len() != info.func_type_indices.len() {
        return Err(AppError::script(WasmError::FunctionSectionMismatch));
    }

    Ok(info)
}

fn eval_const_expr_i64(expr: &ConstExpr<'_>) -> AppResult<i64> {
    let mut reader = expr.get_operators_reader();
    let mut value: Option<i64> = None;

    while !reader.eof() {
        let op = reader
            .read()
            .map_err(|err| AppError::script(WasmError::InvalidConstExpr { source: err }))?;
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "Non-const operators are rejected explicitly."
        )]
        match op {
            Operator::I32Const { value: v } => {
                value = Some(i64::from(v));
            }
            Operator::I64Const { value: v } => {
                value = Some(v);
            }
            Operator::End => break,
            _ => {
                return Err(AppError::script(WasmError::ConstExprNotConstant));
            }
        }
    }

    if !reader.eof() {
        return Err(AppError::script(WasmError::UnexpectedConstExprOperators));
    }

    value.ok_or_else(|| AppError::script(WasmError::ConstExprMissingValue))
}

fn parse_const_i32(body: &FunctionBody<'_>) -> AppResult<i32> {
    let mut reader = body
        .get_operators_reader()
        .map_err(|err| AppError::script(WasmError::InvalidFunctionBody { source: err }))?;
    let mut value: Option<i32> = None;

    while !reader.eof() {
        let op = reader
            .read()
            .map_err(|err| AppError::script(WasmError::InvalidFunctionBody { source: err }))?;
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "Non-const operators are rejected explicitly."
        )]
        match op {
            Operator::I32Const { value: v } => {
                value = Some(v);
            }
            Operator::End => break,
            Operator::Nop => {}
            _ => {
                return Err(AppError::script(WasmError::ScenarioFuncMustReturnConstI32));
            }
        }
    }

    if !reader.eof() {
        return Err(AppError::script(
            WasmError::UnexpectedOperatorsAfterConstFunction,
        ));
    }

    value.ok_or_else(|| AppError::script(WasmError::ScenarioFuncMustReturnValue))
}
