mod app;
mod args;
mod charts;
mod config;
mod distributed;
mod entry;
mod error;
mod http;
mod metrics;
mod protocol;
mod script;
mod service;
mod shutdown;
mod sinks;
mod system;
mod ui;
#[cfg(feature = "wasm")]
mod wasm_plugins;
#[cfg(feature = "wasm")]
mod wasm_runtime;

#[cfg(feature = "alloc-profiler")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

use error::AppResult;

fn main() -> AppResult<()> {
    entry::run()
}
