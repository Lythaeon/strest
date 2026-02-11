mod app;
mod args;
mod charts;
mod config;
mod distributed;
mod entry;
mod error;
mod http;
mod logger;
mod metrics;
#[cfg(feature = "wasm")]
mod probestack;
mod script;
mod service;
mod shutdown;
mod shutdown_handlers;
mod sinks;
mod ui;

#[cfg(feature = "alloc-profiler")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

use error::AppResult;

fn main() -> AppResult<()> {
    entry::run()
}
