//! Script engines for generating scenarios.

mod loader;
#[cfg(feature = "wasm")]
mod wasm;

pub(crate) use loader::load_scenario_from_wasm;
