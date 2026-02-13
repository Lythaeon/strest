pub(super) const PLUGIN_API_VERSION: i32 = 1;

pub(super) const MAX_PLUGIN_WASM_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MAX_PLUGIN_PAYLOAD_BYTES: usize = 256 * 1024;
pub(super) const HOOK_RUN_START: &str = "strest_on_run_start";
pub(super) const HOOK_METRICS_SUMMARY: &str = "strest_on_metrics_summary";
pub(super) const HOOK_ARTIFACT: &str = "strest_on_artifact";
pub(super) const HOOK_RUN_END: &str = "strest_on_run_end";

pub(super) const WASMER_BIN: &str = "wasmer";
