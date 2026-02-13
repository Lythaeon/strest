# WASM Plugin Examples

This folder contains minimal plugins for the `--plugin` runtime.

## 1) echo-hook-logger

Prints hook payload highlights to stderr for:

- run start
- metrics summary
- artifacts
- run end

Build:

```bash
cargo build --release --target wasm32-wasip1 --manifest-path examples/plugins/echo-hook-logger/Cargo.toml
```

Run:

```bash
cargo run --features wasm -- \
  --plugin ./examples/plugins/echo-hook-logger/target/wasm32-wasip1/release/echo-hook-logger.wasm \
  -u http://localhost:3000 -t 10 --summary --no-tui --no-charts
```

## 2) slo-guard

Fails the plugin hook if success rate drops below a threshold.
Default threshold: `99%`.

Optional env override:

```bash
export STREST_MIN_SUCCESS_PCT=95
```

Build:

```bash
cargo build --release --target wasm32-wasip1 --manifest-path examples/plugins/slo-guard/Cargo.toml
```

Run:

```bash
cargo run --features wasm -- \
  --plugin ./examples/plugins/slo-guard/target/wasm32-wasip1/release/slo-guard.wasm \
  -u http://localhost:3000 -t 10 --summary --no-tui --no-charts
```
