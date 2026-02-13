# Advanced

This page covers replay, profiling, WASM scripting, distributed mode, sinks, and operations topics.

## Presets vs Advanced Flags

`strest quick`, `strest soak`, `strest spike`, and `strest distributed` provide opinionated defaults for common workflows.
The original flag-driven workflow remains fully supported for power users.

Use presets when you want a fast starting point, and switch to top-level flags/config when you need exact tuning.
The CLI remains intentionally sharp-edged for advanced users; presets are a convenience layer, not a replacement.

## Protocol Adapter Strategy

The CLI surface includes a protocol adapter selector (`--protocol`) so protocol-specific engines can
be added without redesigning the command model.

Protocol adapters are managed through a central registry in `src/protocol.rs`
and `src/protocol/`.
Each adapter declares:

- protocol key (`Protocol`)
- display name
- whether it is executable in this build
- stateful-connection capability
- supported `LoadMode` values

Important current limitation: this is **not** a runtime-loaded external plugin system yet.
Today, adapters are compiled into the binary and selected through the fixed `Protocol` enum.
So adding a brand-new protocol still requires core code changes (enum + runtime sender wiring),
not just dropping in an external module.

To add metadata for an existing protocol key, implement `ProtocolAdapter` and register it in
`ProtocolRegistry::with_builtins()`. Entry validation and CLI behavior will pick it up automatically.

Example adapter implementations (reference patterns):

- `src/protocol/examples/game_udp.rs`
- `src/protocol/examples/chat_websocket.rs`
- `src/protocol/examples/telemetry_mqtt.rs`

Current execution support:

- `http`, `grpc-unary`, `grpc-streaming`, `websocket`, `tcp`, `udp`
- `quic`, `mqtt`, `enet`, `kcp`, `raknet`

Notes on baseline transport behavior:

- `quic`, `enet`, `kcp`, and `raknet` currently use one-shot datagram probing semantics (UDP-style send + optional recv).
- `mqtt` uses a minimal MQTT 3.1.1 `CONNECT` + optional QoS0 `PUBLISH` flow (topic derived from URL path).
- gRPC adapters accept both `grpc://`/`grpcs://` and `http://`/`https://` URL schemes.

## Load-Mode Intent

`--load-mode` captures run intent and aligns presets/config to “named workflows”:

- `arrival`: default arrival-rate behavior.
- `step`/`ramp`: staged load profiles from config.
- `burst`: spike-style traffic.
- `soak`: long-duration stability runs.
- `jitter`: reserved for future randomized scheduling.

Load mode is also validated per protocol adapter. Notably, `grpc-unary` currently supports
`arrival` and `ramp`, while other executable adapters accept all current load modes.

## Replay (Post-Mortem)

Replay lets you re-run summaries from tmp logs or exported CSV/JSON/JSONL without hitting the target again.
In TTY mode, replay uses the same TUI as live runs (including the latency chart), and it shows snapshot markers plus hotkeys.

From tmp logs:

```bash
strest --replay --tmp-path ~/.strest/tmp
```

From exports:

```bash
strest --replay --export-csv ./metrics.csv
strest --replay --export-json ./metrics.json
strest --replay --export-jsonl ./metrics.jsonl
```

JSONL is recommended for large runs and streaming scenarios because it can be parsed incrementally.

Windowing and controls:

```bash
strest --replay --tmp-path ~/.strest/tmp --replay-start 10s --replay-end 2m --replay-step 5s
```

Replay snapshots:

```bash
strest --replay --export-jsonl ./metrics.jsonl --replay-snapshot-interval 30s --replay-snapshot-format jsonl
strest --replay --tmp-path ~/.strest/tmp --replay-snapshot-start 10s --replay-snapshot-end 2m --replay-snapshot-format json
```

Controls: `space` play/pause, `←/→` seek, `r` restart, `q` quit, `s` mark snapshot start, `e` mark snapshot end, `w` write snapshot.
Snapshots default to `~/.strest/snapshots` (or `%USERPROFILE%\\.strest\\snapshots` on Windows) unless `--replay-snapshot-out` is set.

## Debug + Profiling Features

Strest exposes a few debug/profiling features behind build-time flags:

- `alloc-profiler`: enables jemalloc stats + heap dumps (used by `--alloc-profiler-*` flags).
- `legacy-charts`: keeps the pre-streaming chart pipeline available (primarily for tests).

Build example (profiling enabled):

```bash
JEMALLOC_SYS_WITH_MALLOC_CONF=prof:true,lg_prof_sample:19,prof_prefix:strest \
CARGO_PROFILE_RELEASE_DEBUG=2 CARGO_PROFILE_RELEASE_STRIP=none \
cargo build --release --features alloc-profiler
```

Run with profiling active:

```bash
MALLOC_CONF=prof_active:true \
./target/release/strest -u http://localhost:8887 -t 120 --no-tui --no-charts --summary \
  --rss-log-ms 5000 \
  --alloc-profiler-ms 5000 \
  --alloc-profiler-dump-ms 5000 --alloc-profiler-dump-path ./alloc-prof
```

## WASM Scripts (Experimental)

There are two WASM extension paths in `strest`:

- `--script`: scenario-generation scripts (request model input)
- `--plugin`: lifecycle plugins (run/metrics/artifact hooks)

Use scripts when you want a module to define the scenario payload (and, over time, support dynamic
request generation such as payload/header shaping and response-reactive flows) while still running
on strest's scenario engine.

Build with the optional feature:

```bash
cargo build --release --features wasm
```

Run with a WASM script:

```bash
strest --script ./script.wasm --no-tui --summary --no-charts
```

Example WASM script (prebuilt in this repo):

```bash
# Optional: regenerate from WAT
wasm-tools parse examples/wasm/interesting.wat -o examples/wasm/interesting.wasm

# Run the example scenario
cargo run --features wasm -- --script examples/wasm/interesting.wasm -t 20 --no-tui --summary --no-charts
```

Note: the example scenario targets `http://localhost:8887` and expects `/health`, `/login`,
`/search`, `/items/{id}`, and `/checkout` endpoints to exist. Update the WAT if your server differs.

**WASM contract**

Your module must export:

- `memory`
- `scenario_ptr() -> i32` (pointer to a UTF-8 JSON buffer)
- `scenario_len() -> i32` (length of that buffer in bytes)

The JSON payload must match the `scenario` config schema (same as `strest.toml` / `strest.json`). It **must** include `schema_version: 1`. Size is capped at 1MB.

Sandboxing policy (enforced):

- No imports are allowed.
- The exported `memory` must declare a maximum and be <= 128 pages.
- The module size is capped at 4MB.
- The scenario payload is capped at 1MB.
- `scenario_ptr` and `scenario_len` must return constant `i32` values.
- `memory64` and shared memory are not allowed.

Minimal Rust example (`wasm32-unknown-unknown`):

```rust
#[no_mangle]
pub extern "C" fn scenario_ptr() -> i32 {
    SCENARIO.as_ptr() as i32
}

#[no_mangle]
pub extern "C" fn scenario_len() -> i32 {
    SCENARIO.len() as i32
}

static SCENARIO: &str = r#"{
  "schema_version": 1,
  "base_url": "http://localhost:3000",
  "steps": [
    { "method": "get", "path": "/health", "assert_status": 200 }
  ]
}"#;
```

Notes:

- If you pass `--url`, it becomes the default base URL when the scenario omits `base_url`.
- `--script` cannot be combined with an explicit `scenario` config section.

## WASM Plugins (Experimental)

You can load external WASM plugins with:

```bash
strest --plugin ./plugins/example.wasm -u http://localhost:3000 -t 20 --summary
```

`--plugin` is repeatable. Plugins are invoked through `wasmer` as sandboxed WASI commands and are
intended for lifecycle hook integrations and additional runtime features.

Current hooks:

- `strest_on_run_start`
- `strest_on_metrics_summary`
- `strest_on_artifact`
- `strest_on_run_end`

ABI contract (host -> plugin):

- env `STREST_PLUGIN_API_VERSION=1`
- env `STREST_PLUGIN_HOOK=<hook-name>`
- payload JSON on `stdin`
- exit code `0` => success, non-zero => failure

Sandbox policy (current):

- plugin module size capped at 4MB
- only `wasi_snapshot_preview1` imports are allowed
- per-hook payload capped at 256KB

SDK:

- `sdk/strest-wasm-plugin-sdk`
- helper API for hook dispatch and ABI validation in plugin `main()`

Example plugins:

- `examples/plugins/echo-hook-logger`
- `examples/plugins/slo-guard`
- `examples/plugins/README.md` (build + run commands)

## Output Sinks

Configure output sinks in the config file to emit summary metrics periodically during the run
and once after the run completes. The default update interval is 1000ms.

Example `strest.toml`:

```toml
[sinks]
update_interval_ms = 1000

[sinks.prometheus]
path = "./out/strest.prom"

[sinks.otel]
path = "./out/strest.otel.json"

[sinks.influx]
path = "./out/strest.influx"
```

## Distributed Mode

Run a controller and one or more agents. If you configure sinks on agents, they write per-agent
files when stream summaries are off. If you configure sinks on the controller, it writes an
aggregated sink report at the end of the run (or periodically when streaming).
If `distributed.stream_summaries = true`, agents stream
periodic summaries to the controller; the controller updates sinks during the run and agents
skip local sink writes. Stream cadence is controlled by `distributed.stream_interval_ms`
(default 1000ms) and sink update cadence by `sinks.update_interval_ms`. When UI rendering is
enabled, the controller aggregates streamed metrics into the UI.
Use `distributed.agent_wait_timeout_ms` (or `--agent-wait-timeout-ms`) to bound how long the
controller waits for `min_agents` before starting.
The controller does not generate load; it only orchestrates and aggregates. Agents are the
ones that send requests.
Agents send periodic heartbeats; the controller marks agents unhealthy if no heartbeat is
seen within `--agent-heartbeat-timeout-ms` (default 3000ms).
Aggregated charts are available in distributed mode when `--stream-summaries` is enabled and
`--no-charts` is not set (charts are written by the controller). Per-agent exports are still
disabled during distributed runs.
CLI equivalents: `--stream-summaries` and `--stream-interval-ms 1000`.

When using `--script` in distributed mode, only the controller needs the WASM-enabled build.
The controller loads the script, generates the scenario, and coordinates the agents with the
resulting scenario; agents do not execute WASM.

### Kubernetes (Basic Manifests)

The `kubernetes/` folder ships minimal manifests for a controller and scalable agents.

Apply:

```bash
kubectl apply -f kubernetes/
```

Scale agents:

```bash
kubectl scale deployment strest-agent --replicas=100
```

Start a run (manual controller mode; port-forward the control plane):

```bash
kubectl port-forward service/strest-controller 9010:9010
curl -X POST http://127.0.0.1:9010/start -d '{"start_after_ms":2000}'
```

Agents discover the controller via the service DNS name
`strest-controller.<namespace>.svc.cluster.local` (the manifests use
`strest-controller:9009`).

Scaling notes:

- There is no hard agent limit; practical limits come from OS file descriptors, CPU, and memory.
- Streaming summaries add controller load (histogram decode + merge). Increase `--stream-interval-ms`
  to reduce overhead as agent counts grow.
- Wire messages are capped at 4MB; very large histograms can exceed this limit.

Controller:

```bash
strest --controller-listen 0.0.0.0:9009 --min-agents 2 --auth-token secret
```

Agent:

```bash
strest --agent-join 10.0.0.5:9009 --auth-token secret --agent-weight 2
```

Agent standby (keeps the agent connected and auto-reconnects between runs):

```bash
strest --agent-join 10.0.0.5:9009 --auth-token secret --agent-standby --agent-reconnect-ms 1000
```

Example `strest.toml` controller config:

```toml
[distributed]
role = "controller"
listen = "0.0.0.0:9009"
auth_token = "secret"
min_agents = 2
agent_wait_timeout_ms = 30000
agent_heartbeat_timeout_ms = 3000
stream_summaries = true
stream_interval_ms = 1000
```

Example `strest.toml` agent config:

```toml
[distributed]
role = "agent"
join = "10.0.0.5:9009"
auth_token = "secret"
agent_id = "agent-1"
weight = 2
agent_heartbeat_interval_ms = 1000
```

Manual controller mode (HTTP control plane):

```bash
strest --controller-listen 0.0.0.0:9009 --controller-mode manual --control-listen 127.0.0.1:9010 --auth-token secret --control-auth-token control-secret
```

Manual controller config example:

```toml
[distributed]
role = "controller"
controller_mode = "manual"
listen = "0.0.0.0:9009"
control_listen = "127.0.0.1:9010"
control_auth_token = "control-secret"
```

Start and stop via HTTP:

```bash
curl -X POST http://127.0.0.1:9010/start -H "Authorization: Bearer control-secret"
curl -X POST http://127.0.0.1:9010/start -H "Authorization: Bearer control-secret" -d '{"scenario_name":"login"}'
curl -X POST http://127.0.0.1:9010/stop -H "Authorization: Bearer control-secret"
```

The `/start` payload can include `scenario_name` (from the registry) and/or an inline
`scenario` (same schema as the config file). If you pass a `scenario` without a name,
it runs once and is not stored. If you pass both `scenario` and `scenario_name`,
the controller stores/updates that named scenario and runs it. You can also pass
`start_after_ms` to delay the run and `agent_wait_timeout_ms` to wait for enough
agents before starting. If omitted, the controller runs the default scenario or
`--url` configured on startup.

Scenario registry (preload multiple named scenarios):

```toml
[scenario]
base_url = "http://localhost:3000"

[[scenario.steps]]
method = "get"
path = "/health"
assert_status = 200

[scenarios.login]
base_url = "http://localhost:3000"

[[scenarios.login.steps]]
method = "post"
path = "/login"
data = "{\"user\":\"demo\"}"
assert_status = 200
```

## Linux Systemd Service

Install a controller service (requires sudo):

```bash
sudo strest --controller-listen 0.0.0.0:9009 --controller-mode manual --control-listen 127.0.0.1:9010 --install-service --service-name strest-controller
```

Install an agent service:

```bash
sudo strest --agent-join 10.0.0.5:9009 --agent-standby --install-service --service-name strest-agent
```

Uninstall a service:

```bash
sudo strest --controller-listen 0.0.0.0:9009 --uninstall-service --service-name strest-controller
```

Systemd install/uninstall writes to `/etc/systemd/system` and runs `systemctl`, so it must be executed with sudo.

## Reproducible Builds

Use `--locked` to ensure the build uses the exact dependency versions in `Cargo.lock`:

```bash
cargo build --release --locked
```

## Testing

Run the full test suite with nextest:

```bash
cargo make test
```

Run the WASM end-to-end test:

```bash
cargo make test-wasm
```

## Formatting

Check formatting with:

```bash
cargo make format-check
```

Auto-format with:

```bash
cargo make format
```

## Motivation

strest was born to provide performance insight for stexs and the infrastructure behind it.
