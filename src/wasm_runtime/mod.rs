mod constants;
mod loader;
mod module;
mod parse;
mod validate;

#[cfg(all(test, feature = "wasm"))]
mod tests;

pub(crate) use loader::load_scenario_from_wasm;
