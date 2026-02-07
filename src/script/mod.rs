//! Script engines for generating scenarios.

#[cfg(feature = "wasm")]
mod wasm;

#[cfg(feature = "wasm")]
pub(crate) fn load_scenario_from_wasm(
    script_path: &str,
    args: &crate::args::TesterArgs,
) -> Result<crate::args::Scenario, String> {
    wasm::load_scenario_from_wasm(script_path, args)
}

#[cfg(not(feature = "wasm"))]
pub(crate) fn load_scenario_from_wasm(
    _: &str,
    _: &crate::args::TesterArgs,
) -> Result<crate::args::Scenario, String> {
    Err("WASM scripting requires the 'wasm' feature.".to_owned())
}
