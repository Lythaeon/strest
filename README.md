# strest

⚠️ Warning: Only use strest for testing infrastructure you own or have explicit permission to test. Unauthorized use may be illegal.

Strest is a command-line tool for stress testing web servers by sending a large number of HTTP requests. It provides insights into server performance by measuring average response times, reporting observed requests per minute (RPM), and other relevant metrics.

# Screenshot Overview  
These screenshots showcase key metrics and real-time statistics from Strest’s stress testing, including charts with response time, error rate, request count, latency percentile, and throughput.

<div style="text-align: center;">
  <img src="docs/screenshot.png" alt="CLI Screenshot" width="1000" />
</div>

<p align="center" width="100%">
  <span style="display: inline-block; width: 150px;">
    <a href="docs/average_response_time.png" target="_blank">
      <img src="docs/average_response_time.png" alt="Average Response Time" width="150" style="border: 1px solid #ddd; border-radius: 4px;" />
    </a>
  </span>
  <span style="display: inline-block; width: 150px;">
    <a href="docs/cumulative_error_rate.png" target="_blank">
      <img src="docs/cumulative_error_rate.png" alt="Cumulative Error Rate" width="150" style="border: 1px solid #ddd; border-radius: 4px;" />
    </a>
  </span>
  <span style="display: inline-block; width: 150px;">
    <a href="docs/cumulative_successful_requests.png" target="_blank">
      <img src="docs/cumulative_successful_requests.png" alt="Cumulative Successful Requests" width="150" style="border: 1px solid #ddd; border-radius: 4px;" />
    </a>
  </span>
  <span style="display: inline-block; width: 150px;">
    <a href="docs/cumulative_total_requests.png" target="_blank">
      <img src="docs/cumulative_total_requests.png" alt="Cumulative Total Requests" width="150" style="border: 1px solid #ddd; border-radius: 4px;" />
    </a>
  </span>
  <span style="display: inline-block; width: 150px;">
    <a href="docs/latency_percentiles_P50.png" target="_blank">
      <img src="docs/latency_percentiles_P50.png" alt="Latency Percentiles P50" width="150" style="border: 1px solid #ddd; border-radius: 4px;" />
    </a>
  </span>
  <span style="display: inline-block; width: 150px;">
    <a href="docs/latency_percentiles_P90.png" target="_blank">
      <img src="docs/latency_percentiles_P90.png" alt="Latency Percentiles P90" width="150" style="border: 1px solid #ddd; border-radius: 4px;" />
    </a>
  </span>
  <span style="display: inline-block; width: 150px;">
    <a href="docs/latency_percentiles_P99.png" target="_blank">
      <img src="docs/latency_percentiles_P99.png" alt="Latency Percentiles P99" width="150" style="border: 1px solid #ddd; border-radius: 4px;" />
    </a>
  </span>
  <span style="display: inline-block; width: 150px;">
    <a href="docs/requests_per_second.png" target="_blank">
      <img src="docs/requests_per_second.png" alt="Requests Per Second" width="150" style="border: 1px solid #ddd; border-radius: 4px;" />
    </a>
  </span>
</p>

## Features

- Send HTTP requests to a specified URL for a specified duration.
- Customize the HTTP method, headers, and request payload data.
- Measure the average response time of successful requests.
- Report the observed requests per minute (RPM) metric.
- Display real-time statistics and progress in the terminal.
- Optional non-interactive summary output for long-running tests.
- Streams run metrics to disk while aggregating summary and chart data during the run.
- Optional rate limiting for controlled load generation.
- Optional CSV/JSON exports for pipeline integration.
- Scenario scripts with multi-step flows, dynamic templates, and per-step asserts.
- Experimental WASM scripting to generate scenarios programmatically.
- Warm-up period support to exclude early metrics from summaries and charts.
- TLS/HTTP/2 controls (TLS min/max, HTTP/2 toggle, ALPN selection).
- HDR histogram percentiles for accurate end-of-run latency stats.
- Pluggable output sinks (Prometheus textfile, OTel JSON, Influx line protocol).
- Distributed mode with controller/agent coordination and weighted load splits.
- Distributed streaming summaries for live aggregation and sink updates.
- Manual controller mode with HTTP start/stop control and scenario registry.
- Agent standby mode with automatic reconnects between runs.
- Experimental HTTP/3 support (build flag required).
- Linux systemd install/uninstall helpers for controller/agent services.

## Prerequisites

- Make sure you have Rust and Cargo installed on your system. You can install Rust from [rustup.rs](https://rustup.rs/).

## Installation

### From crates.io (recommended)

```bash
cargo install strest
```

### Prebuilt binaries

Prebuilt binaries are attached to GitHub Releases for tagged versions (Linux, macOS, Windows).

### From source

To use Strest from source, follow these installation instructions:

1. Clone the repository to your local machine:

    ```bash
    git clone https://github.com/Lythaeon/strest.git
    ```

2. Change to the project directory:

    ```bash
    cd strest
    ```

3. Build the project:

    ```bash
    cargo build --release --locked
    ```

4. Once the build is complete, you can find the executable binary in the `/target/release/` directory.

5. Copy the binary to a directory in your system's PATH to make it globally accessible:

    ```bash
    sudo cp ./target/release/strest /usr/local/bin/
    ```

Alternatively, install from the local path using Cargo:

```bash
cargo install --path . --locked
```

## Getting Started

Quick smoke test:

```bash
strest -u http://localhost:3000 -t 30
```

Scenario quickstart:

```toml
# strest.toml
[scenario]
base_url = "http://localhost:3000"

[[scenario.steps]]
method = "get"
path = "/health"
assert_status = 200
```

```bash
strest --config strest.toml -t 30 --no-ui --summary --no-charts
```

## Usage

Strest is used via the command line. Here's a basic example of how to use it:

```bash
strest -u http://localhost:3000 -t 60 --no-charts
```

This command sends GET requests to `http://localhost:3000` for 60 seconds.

For long-running or CI runs, disable the UI and print a summary:

```bash
strest -u http://localhost:3000 -t 600 --no-ui --summary --no-charts
```

For more options and customization, use the --help flag to see the available command-line options and their descriptions.

```bash
strest --help
```

### Logging

Use `--verbose` to enable debug logging (useful for distributed controller/agent handshakes). You can also override the log level via `STREST_LOG` or `RUST_LOG`.

### Presets

Smoke (quick validation, low load):

```bash
strest -u http://localhost:3000 -t 15 --rate 50 --max-tasks 50 --spawn-rate 10 --spawn-interval 100 --no-charts
```

Steady (sustained load, CI-friendly):

```bash
strest -u http://localhost:3000 -t 300 --rate 500 --max-tasks 500 --spawn-rate 20 --spawn-interval 100 --no-ui --summary --no-charts
```

Ramp (gradual increase):

```toml
# ramp.toml
url = "http://localhost:3000"
duration = 300

[load]
rate = 100

[[load.stages]]
duration = "60s"
target = 300

[[load.stages]]
duration = "120s"
target = 800
```

```bash
strest --config ramp.toml --no-ui --summary --no-charts
```

### Charts

By default charts are stored in `~/.strest/charts` (or `%USERPROFILE%\\.strest\\charts` on Windows). You can change the location via `--charts-path` (`-c`).

To disable charts use the `--no-charts` flag.

### Temp Data

Run data is logged to a temporary file during the test while summary and chart data are aggregated during the run. This keeps the request pipeline from blocking on metrics in long runs. By default this lives in `~/.strest/tmp` (or `%USERPROFILE%\\.strest\\tmp` on Windows). You can change the location via `--tmp-path`. Temporary data is deleted after the run unless `--keep-tmp` is set.

Charts collection can be bounded for long runs:

- `--metrics-range` limits chart collection to a time window (e.g., `10-30` seconds).
- `--metrics-max` caps the total number of metrics kept for charts (default: `1000000`).

### Common Options

- `--method` (`-X`) sets the HTTP method.
- `--url` (`-u`) sets the target URL.
- `--headers` (`-H`) adds request headers (repeatable, `Key: Value`).
- `--data` (`-d`) sets the request body data (POST/PUT/PATCH).
- `--duration` (`-t`) sets the test duration in seconds.
- `--no-ui` disables the interactive UI and shows a progress bar in the terminal (summary output is printed automatically).
- `--summary` prints an end-of-run summary.
- `--status` (`-s`) sets the expected HTTP status code.
- `--timeout` sets the request timeout (supports `ms`, `s`, `m`, `h`).
- `--warmup` ignores the first N seconds for summary/charts/exports (supports `ms`, `s`, `m`, `h`).
- `--proxy` (`-p`) sets a proxy URL.
- `--max-tasks` (`-m`) limits concurrent request tasks (`--concurrency` alias).
- `--spawn-rate` (`-r`) and `--spawn-interval` (`-i`) control how quickly tasks are spawned.
- `--rate` sets a global requests-per-second limit.
- `--controller-listen` starts a distributed controller (e.g., `0.0.0.0:9009`).
- `--controller-mode` selects controller mode (`auto` or `manual`).
- `--control-listen` sets the manual control-plane HTTP listen address.
- `--control-auth-token` sets the control-plane Bearer token.
- `--agent-join` joins a distributed controller as an agent.
- `--auth-token` sets a shared token for controller/agent authentication.
- `--agent-weight` sets an agent weight for load distribution.
- `--agent-id` sets an explicit agent id.
- `--min-agents` sets how many agents the controller waits for before starting.
- `--agent-wait-timeout-ms` sets a max wait time for min agents (auto mode; manual start honors this too).
- `--agent-standby` keeps agents connected between distributed runs.
- `--agent-reconnect-ms` sets the standby reconnect interval.
- `--agent-heartbeat-interval-ms` sets the agent heartbeat interval.
- `--agent-heartbeat-timeout-ms` sets the controller heartbeat timeout.
- `--stream-interval-ms` sets the stream snapshot interval for distributed mode.
- `--script` runs a WASM script that produces a scenario (requires `--features wasm` build).
- `--tls-min` and `--tls-max` set the TLS version floor/ceiling.
- `--http2` enables HTTP/2 (adaptive).
- `--http3` enables HTTP/3 (requires `--features http3` and `RUSTFLAGS=--cfg reqwest_unstable`).
- `--alpn` sets the advertised protocols (repeatable, e.g. `--alpn h2`).
- `--tmp-path` sets where temporary run data is written.
- `--keep-tmp` keeps temporary run data after completion.
- `--log-shards` controls the number of log writers (default `1`).
- `--export-csv` writes metrics to a CSV file (bounded by `--metrics-range` and `--metrics-max`).
- `--export-json` writes summary and metrics to a JSON file (bounded by `--metrics-range` and `--metrics-max`).
- `--install-service` installs a Linux systemd service for controller/agent.
- `--uninstall-service` removes a Linux systemd service for controller/agent.
- `--service-name` overrides the systemd service name.

HTTP/3 is experimental and requires building with `--features http3` plus
`RUSTFLAGS="--cfg reqwest_unstable"` (reqwest requirement):

```bash
RUSTFLAGS="--cfg reqwest_unstable" cargo build --release --features http3
```

### Configuration File

You can provide a config file with `--config path`. If no config is specified, `strest` will look for `./strest.toml` or `./strest.json` (TOML is preferred if both exist). CLI flags override config values.

Example `strest.toml`:

```toml
url = "http://localhost:3000"
method = "get"
duration = 60
timeout = "10s"
warmup = "5s"
status = 200
no_ui = true
summary = true
no_charts = true

tls_min = "1.2"
tls_max = "1.3"
http2 = true
alpn = ["h2"]

headers = [
  "Content-Type: application/json",
  "X-Env: local",
]

metrics_range = "10-30"
metrics_max = 1000000

[load]
rate = 1000

[[load.stages]]
duration = "10s"
target = 500

[[load.stages]]
duration = "20s"
target = 1500
```

Load profiles are optional. `load.rate` is the initial RPS, and each stage linearly ramps to its `target` RPS over the stage `duration`. You can use `rpm` instead of `rate/target` for RPM-based control.

Example `strest.json`:

```json
{
  "url": "http://localhost:3000",
  "method": "get",
  "duration": 60,
  "warmup": "5s",
  "status": 200,
  "no_ui": true,
  "summary": true,
  "no_charts": true,
  "tls_min": "1.2",
  "tls_max": "1.3",
  "http2": true,
  "alpn": ["h2"],
  "headers": [
    "Content-Type: application/json",
    "X-Env: local"
  ],
  "metrics_range": "10-30",
  "metrics_max": 1000000,
  "load": {
    "rate": 1000,
    "stages": [
      { "duration": "10s", "target": 500 },
      { "duration": "20s", "target": 1500 }
    ]
  }
}
```

### Scenario Scripts

Scenario scripts model multi-step flows with per-step asserts and templated payloads. If `scenario.base_url` is set you can omit the top-level `url`. Templates use `{{var}}` placeholders from `scenario.vars`, `step.vars`, and built-ins: `seq`, `step`, `timestamp_ms`, `timestamp_s`.
`think_time` adds a delay after a step completes before the next step starts (supports `ms`, `s`, `m`, `h`).

Example `strest.toml`:

```toml
[scenario]
base_url = "http://localhost:3000"
vars = { user = "demo" }

[[scenario.steps]]
name = "login"
method = "post"
path = "/login"
headers = ["Content-Type: application/json"]
data = "{\"user\":\"{{user}}\",\"seq\":\"{{seq}}\"}"
assert_status = 200
assert_body_contains = "token"
think_time = "500ms"

[[scenario.steps]]
name = "profile"
method = "get"
path = "/profile"
headers = ["Authorization: Bearer {{seq}}"]
```

Example `strest.json`:

```json
{
  "scenario": {
    "base_url": "http://localhost:3000",
    "vars": { "user": "demo" },
    "steps": [
      {
        "name": "login",
        "method": "post",
        "path": "/login",
        "headers": ["Content-Type: application/json"],
        "data": "{\"user\":\"{{user}}\",\"seq\":\"{{seq}}\"}",
        "assert_status": 200,
        "assert_body_contains": "token",
        "think_time": "500ms"
      },
      {
        "name": "profile",
        "method": "get",
        "path": "/profile",
        "headers": ["Authorization: Bearer {{seq}}"]
      }
    ]
  }
}
```

### WASM Scripts (Experimental)

You can generate scenarios from a WASM module and run them with `--script`. This is useful when you want programmable test setup while still using strest’s scenario engine.

Build with the optional feature:

```bash
cargo build --release --features wasm
```

Run with a WASM script:

```bash
strest --script ./script.wasm --no-ui --summary --no-charts
```

Example WASM script (prebuilt in this repo):

```bash
# Optional: regenerate from WAT
wasm-tools parse examples/wasm/interesting.wat -o examples/wasm/interesting.wasm

# Run the example scenario
cargo run --features wasm -- --script examples/wasm/interesting.wasm -t 20 --no-ui --summary --no-charts
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

### Output Sinks

Configure output sinks in the config file to emit summary metrics periodically during the run
and once after the run completes. The default update interval is 1000ms.

Example `strest.toml`:

```toml
[sinks]
# Optional. Controls periodic updates (defaults to 1000ms).
update_interval_ms = 1000

[sinks.prometheus]
path = "./out/strest.prom"

[sinks.otel]
path = "./out/strest.otel.json"

[sinks.influx]
path = "./out/strest.influx"
```

### Distributed Mode

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

### Linux Systemd Service

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

### k6 Migration (Quick Mapping)

Strest is not a drop-in replacement for k6. k6 uses JS scripts and its own execution model, while strest is config + CLI. That said, for simple single-URL workloads, you can map a few common settings:

```
k6             -> strest
vus            -> --max-tasks
duration       -> -t / --duration
rate (RPS)     -> --rate
stages         -> [load] + [[load.stages]]
```

Example k6 snippet:

```js
export const options = {
  vus: 200,
  duration: "2m",
  rate: 400
};
```

Approximate strest equivalent:

```bash
strest -u http://localhost:3000 -t 120 --max-tasks 200 --rate 400 --no-ui --summary --no-charts
```

For k6 scripts with multiple requests, custom JS logic, or checks, use strest scenarios instead. There is no automatic conversion for those today.

### Reproducible Builds

Use `--locked` to ensure the build uses the exact dependency versions in `Cargo.lock`:

```bash
cargo build --release --locked
```

### Testing

Run the full test suite with nextest:

```bash
cargo make test
```

Run the WASM end-to-end test:

```bash
cargo make test-wasm
```

### Formatting

Check formatting with:

```bash
cargo make format-check
```

Auto-format with:

```bash
cargo make format
```

## Contributions

If you'd like to contribute, I appreciate it. Please:

1. Fork the repo and create a dedicated branch.
2. Implement changes and keep them aligned with existing conventions.
3. Open a pull request and include context for the change.

I review contributions as time allows and will respond when I can.


This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Motivation 

Strest was born out of the need to stress test web servers and gain valuable insights into their performance.
