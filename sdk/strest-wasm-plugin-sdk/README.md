# strest-wasm-plugin-sdk

Minimal SDK for building `strest` WASM plugins using the WASI command ABI.

The host (`strest`) invokes your plugin through `wasmer` and passes:

- hook name in `STREST_PLUGIN_HOOK`
- ABI version in `STREST_PLUGIN_API_VERSION`
- JSON payload on `stdin`

Return `0` for success and non-zero for failure.

## Minimal plugin example

```rust
use strest_wasm_plugin_sdk::{Plugin, RunStartPayload, run_plugin};

struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn on_run_start(&mut self, payload: &RunStartPayload) -> Result<(), String> {
        eprintln!("run_start protocol={}", payload.protocol);
        Ok(())
    }
}

fn main() {
    let mut plugin = ExamplePlugin;
    std::process::exit(run_plugin(&mut plugin));
}
```

Build for WASI and run with strest:

```bash
cargo build --release --target wasm32-wasip1
strest --plugin ./target/wasm32-wasip1/release/your_plugin.wasm ...
```
