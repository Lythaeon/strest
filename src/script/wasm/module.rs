use std::collections::HashMap;

use wasmparser::{ExternalKind, FuncType, MemoryType, TableType};

pub(super) struct WasmModuleInfo {
    pub(super) func_types: Vec<Option<FuncType>>,
    pub(super) func_type_indices: Vec<u32>,
    pub(super) func_constants: Vec<Option<i32>>,
    pub(super) exports: HashMap<String, (ExternalKind, u32)>,
    pub(super) memories: Vec<MemoryType>,
    pub(super) tables: Vec<TableType>,
    pub(super) data_segments: Vec<(u32, u64, Vec<u8>)>,
}
