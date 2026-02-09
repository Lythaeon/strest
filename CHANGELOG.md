# Changelog

All notable changes to this project will be documented in this file.
The format is based on Keep a Changelog, and this project follows SemVer.

## Unreleased

## 0.1.3

Released: 2026-02-09

- Added `strest cleanup` to prune temporary run logs (supports `--older-than`, `--dry-run`, and `--force`).
- Added `--replay` mode for post-mortem analysis from tmp logs or exported CSV/JSON/JSONL, with interactive controls and windowing flags (`--replay-start`, `--replay-end`, `--replay-step`).
- Added replay snapshotting with interval + range controls (`--replay-snapshot-*`) and default snapshot storage under `~/.strest/snapshots`.
- Added JSONL exports (`--export-jsonl`) and replay support for JSONL logs.
- Replay now renders using the default TUI (including the latency chart) and shows snapshot markers plus hotkeys.

## 0.1.2

Released: 2026-02-09

- Added a default `User-Agent: strest-loadtest/<version> (+https://github.com/Lythaeon/strest)` header; disabling it requires `--no-ua` plus `--authorized` (or config `no_ua = true` + `authorized = true`).

## 0.1.1

Released: 2026-02-09

- Added timeout rate chart (timeouts per second) to visualize timeout spikes over time.
- Added error-rate chart per second with breakdown by timeout vs non-2xx vs transport errors.
- Added latency percentile chart with ok vs all overlay for tail comparisons.
- Added UI metrics for timeouts, transport errors, non-expected status, and ok vs all percentiles.
- Added `--ui-window-ms` to control the live UI chart window (default 10000ms).
- Added HTTP status code distribution chart (stacked per-second counts).
- Added in-flight request/concurrency chart to correlate load with latency changes.

## 0.1.0

Released: 2026-02-07

- Added `--no-ui` and `--summary` for long-running/headless runs.
- Added metrics collection cap with `--metrics-max`.
- Added summary output and improved shutdown handling for non-TTY runs.
- Switched request execution to a fixed worker pool with ramp-up permits.
- Added metrics logging pipeline (tmp file) and post-run chart parsing.
- Defaulted charts and tmp paths to `~/.strest` (or `%USERPROFILE%\\.strest` on Windows).
- Added `--tmp-path` and `--keep-tmp` with automatic cleanup of run logs by default.
- Added optional global rate limiting with `--rate`.
- Added CSV/JSON export options (`--export-csv`, `--export-json`).
- Added log sharding via `--log-shards` to scale write throughput.
- Added clearer runtime error reporting with non-zero exit status when logging fails.
- Added a no-UI progress indicator using a terminal progress bar.
- Added separate timeout counts alongside error totals in summaries and sinks.
- Added success-only latency stats alongside overall latency metrics in summaries.
- Added config file support (`--config`, plus `strest.toml`/`strest.json` auto-load).
- Added load profile support in config (`[load]` with stages and ramped targets).
- Added scenario scripts with templated payloads and per-step asserts.
- Added warm-up period support to exclude early metrics.
- Added TLS controls (`--tls-min`, `--tls-max`) plus HTTP/2 and ALPN options.
- Added HDR histogram-based percentiles for summary output.
- Added pluggable output sinks (Prometheus, OTel JSON, Influx line protocol).
- Added distributed mode with TCP controller/agent coordination and weighted load splits.
- Added configurable request timeout via `--timeout` (and config `timeout`).
- Added `--proxy-url` and `--concurrency` aliases for `--proxy` and `--max-tasks`.
- Enabled log level control via `STREST_LOG`/`RUST_LOG`.
- Added fuzz targets for external-input parsers and tightened numeric invariants with positive-only types.
- Output sinks now update during the run (once per second) instead of only at the end.
- Added `sinks.update_interval_ms` to control live sink write frequency.
- Added `distributed.stream_summaries` to stream periodic summaries to the controller.
- Added `distributed.stream_interval_ms` to control agent stream cadence and UI aggregation in streamed runs.
- Added `--stream-summaries` and `--stream-interval-ms` CLI flags for distributed streaming.
- Switched to the Rust 2024 edition.
- Added experimental WASM scripting via `--script` to generate scenarios.
- Added WASM schema versioning, validation, and sandbox limits for script execution.
- Added WASM end-to-end test coverage for script-driven scenarios.
- Added docs.rs-friendly module docs and README guidance for WASM scripts.
- Added a `cargo make test-wasm` task for the WASM end-to-end test.
- Added end-to-end tests for single and distributed runs.
- Added manual controller mode with HTTP `/start` and `/stop` control plane.
- Added scenario registry support (`scenarios` map) for selecting named scenarios via HTTP.
- Added agent standby mode with automatic reconnects between runs.
- Added controller control-plane auth token (`--control-auth-token`).
- Added HTTP/3 support behind a build flag (`--features http3`, `reqwest_unstable`).
- Added Linux systemd install/uninstall helpers (`--install-service`, `--uninstall-service`).
- Added `--verbose` to enable debug logging for troubleshooting distributed runs.
- Fixed distributed auto controller closing agent connections before reports were sent.
- Added `--agent-wait-timeout-ms` / `distributed.agent_wait_timeout_ms` to bound controller wait time for min agents.
- Added aggregated charting for distributed runs (requires `--stream-summaries`).
- Changed project license to AGPL-3.0-only.
- Added README warning about authorized use only and clearer HTTP/3/WASM guidance.
- Added distributed agent heartbeat health checks with configurable interval/timeout.
- Manual `/start` now supports run-only inline scenarios (no storage) and storing named scenarios when `scenario_name` is provided.
- Updated the CLI description and help text for the expanded feature set.
- Running `strest` with no args (or `--`) now prints help unless a default config exists.
- Missing URL errors now emit a log entry before exiting.
- Restored the ratatui-based TUI after the fallback UI proved unreliable.
- Fixed the UI chart Y-axis when latency samples are zero.

## 0.0.0
