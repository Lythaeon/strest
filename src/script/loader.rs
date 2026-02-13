use crate::args::{Scenario, TesterArgs};
use crate::error::AppResult;

#[cfg(not(feature = "wasm"))]
use crate::error::{AppError, ScriptError};

#[cfg(feature = "wasm")]
pub(crate) fn load_scenario_from_wasm(script_path: &str, args: &TesterArgs) -> AppResult<Scenario> {
    crate::wasm_runtime::load_scenario_from_wasm(script_path, args)
}

#[cfg(not(feature = "wasm"))]
pub(crate) fn load_scenario_from_wasm(_: &str, _: &TesterArgs) -> AppResult<Scenario> {
    Err(AppError::script(ScriptError::WasmFeatureDisabled))
}
