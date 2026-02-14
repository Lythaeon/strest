use thiserror::Error;

#[cfg(feature = "wasm")]
use super::ConfigError;

#[derive(Debug, Error)]
pub enum ScriptError {
    #[cfg(feature = "wasm")]
    #[error(transparent)]
    Wasm(#[from] WasmError),
    #[cfg(feature = "wasm")]
    #[error("Failed to read wasm script '{path}': {source}")]
    ReadWasmScript {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[cfg(feature = "wasm")]
    #[error("Scenario config error: {source}")]
    ScenarioConfig {
        #[from]
        source: ConfigError,
    },
    #[cfg(not(feature = "wasm"))]
    #[error("WASM scripting requires the 'wasm' feature.")]
    WasmFeatureDisabled,
    #[cfg(feature = "wasm")]
    #[error("WASM plugin error: {message}")]
    WasmPlugin { message: String },
    #[cfg(test)]
    #[error("Test expectation failed: {message}")]
    TestExpectation { message: &'static str },
    #[cfg(test)]
    #[error("Test expectation failed: {message}: {value}")]
    TestExpectationValue {
        message: &'static str,
        value: String,
    },
}

#[cfg(feature = "wasm")]
#[derive(Debug, Error, Clone, Copy)]
pub enum WasmSection {
    #[error("type")]
    Type,
    #[error("function")]
    Function,
    #[error("export")]
    Export,
    #[error("memory")]
    Memory,
    #[error("table")]
    Table,
    #[error("data")]
    Data,
}

#[cfg(feature = "wasm")]
#[derive(Debug, Error)]
pub enum WasmError {
    #[error("Failed to read wasm metadata '{path}': {source}")]
    ReadMetadata {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Invalid wasm file size: {source}")]
    InvalidFileSize {
        #[source]
        source: std::num::TryFromIntError,
    },
    #[error("WASM module is too large.")]
    ModuleTooLarge,
    #[error("Failed to parse wasm: {source}")]
    Parse {
        #[source]
        source: wasmparser::BinaryReaderError,
    },
    #[error("WASM module must not import any host definitions.")]
    ImportsNotAllowed,
    #[error("Invalid {section} section: {source}")]
    InvalidSection {
        section: WasmSection,
        #[source]
        source: wasmparser::BinaryReaderError,
    },
    #[error("WASM data segment offset must be non-negative.")]
    DataSegmentOffsetNegative,
    #[error("WASM data segments must be active.")]
    DataSegmentsMustBeActive,
    #[error("WASM function section did not match code section.")]
    FunctionSectionMismatch,
    #[error("Invalid const expr: {source}")]
    InvalidConstExpr {
        #[source]
        source: wasmparser::BinaryReaderError,
    },
    #[error("WASM const expressions must be constant integers.")]
    ConstExprNotConstant,
    #[error("Unexpected operators after const expression.")]
    UnexpectedConstExprOperators,
    #[error("WASM const expressions must return a value.")]
    ConstExprMissingValue,
    #[error("Invalid function body: {source}")]
    InvalidFunctionBody {
        #[source]
        source: wasmparser::BinaryReaderError,
    },
    #[error("WASM scenario functions must return a constant i32 value.")]
    ScenarioFuncMustReturnConstI32,
    #[error("Unexpected operators after constant function.")]
    UnexpectedOperatorsAfterConstFunction,
    #[error("WASM scenario functions must return a value.")]
    ScenarioFuncMustReturnValue,
    #[error("WASM memory must be non-shared.")]
    MemoryShared,
    #[error("WASM memory must be 32-bit.")]
    Memory64Bit,
    #[error("WASM memory must declare a maximum size.")]
    MemoryMissingMax,
    #[error("WASM memory exceeds the configured limit.")]
    MemoryExceedsLimit,
    #[error("WASM table exceeds the configured limit.")]
    TableExceedsLimit,
    #[error("Scenario schema_version is required for WASM scripts.")]
    ScenarioSchemaMissing,
    #[error("Unsupported scenario schema_version {version}.")]
    ScenarioSchemaUnsupported { version: u32 },
    #[error("Scenario must include at least one step.")]
    ScenarioMissingSteps,
    #[error("Scenario vars cannot contain empty keys.")]
    ScenarioVarsEmptyKey,
    #[error("Scenario step {step} has invalid assert_status {status}.")]
    ScenarioStepInvalidAssertStatus { step: usize, status: u16 },
    #[error("Scenario step {step} vars cannot contain empty keys.")]
    ScenarioStepVarsEmptyKey { step: usize },
    #[error("WASM export '{name}' has the wrong kind.")]
    ExportWrongKind { name: String },
    #[error("WASM module missing '{name}' export.")]
    ExportMissing { name: String },
    #[error("Invalid function index for {name}.")]
    InvalidFunctionIndex { name: String },
    #[error("Missing function signature for {name}.")]
    MissingFunctionSignature { name: String },
    #[error("Invalid function type for {name}.")]
    InvalidFunctionType { name: String },
    #[error("{name} export must be a function type.")]
    ExportNotFunction { name: String },
    #[error("{name} must not take parameters.")]
    ExportTakesParameters { name: String },
    #[error("{name} must return a single i32.")]
    ExportReturnTypeInvalid { name: String },
    #[error("Missing function body for {name}.")]
    MissingFunctionBody { name: String },
    #[error("{name} must return a constant i32.")]
    ExportMustReturnConstI32 { name: String },
    #[error("Invalid memory index.")]
    InvalidMemoryIndex,
    #[error("WASM module missing memory export.")]
    MemoryExportMissing,
    #[error("Invalid table index.")]
    InvalidTableIndex,
    #[error("WASM module missing table export.")]
    TableExportMissing,
    #[error("WASM memory size overflow.")]
    MemorySizeOverflow,
    #[error("WASM data segments must target the exported memory.")]
    DataSegmentWrongMemory,
    #[error("WASM data segment offset overflow.")]
    DataSegmentOffsetOverflow,
    #[error("WASM data segment overflow.")]
    DataSegmentOverflow,
    #[error("WASM data segment exceeds memory size.")]
    DataSegmentOutOfBounds,
    #[error("WASM scenario pointer/length must be non-negative.")]
    ScenarioPointerOrLengthNegative,
    #[error("Invalid scenario length: {source}")]
    InvalidScenarioLength {
        #[source]
        source: std::num::TryFromIntError,
    },
    #[error("WASM scenario payload too large.")]
    ScenarioPayloadTooLarge,
    #[error("Invalid scenario pointer: {source}")]
    InvalidScenarioPointer {
        #[source]
        source: std::num::TryFromIntError,
    },
    #[error("WASM scenario pointer overflow.")]
    ScenarioPointerOverflow,
    #[error("WASM scenario pointer out of bounds.")]
    ScenarioPointerOutOfBounds,
    #[error("Scenario JSON is not valid UTF-8: {source}")]
    ScenarioJsonInvalidUtf8 {
        #[source]
        source: std::str::Utf8Error,
    },
    #[error("Invalid scenario JSON: {source}")]
    ScenarioJsonInvalid {
        #[source]
        source: serde_json::Error,
    },
}
