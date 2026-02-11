# Changelog

All notable changes to this project will be documented in this file.
The format is based on Keep a Changelog, and this project follows SemVer.

## Unreleased

## 0.1.8

Released: 2026-02-11

- Fixed Windows (`x86_64-pc-windows-msvc`) build failure caused by unconditional `ClientBuilder::unix_socket(...)` usage.
- Gated Unix socket client configuration behind `cfg(unix)` in the HTTP sender so Windows builds do not compile Unix-only APIs.
- Added a clear validation error (`--unix-socket is only supported on Unix targets`) when `--unix-socket` is provided on non-Unix platforms.

## 0.1.7

Released: 2026-02-11

- Split README into `docs/USAGE.md` and `docs/ADVANCED.md` and shortened the top-level README.
- Refactored internal error handling and reduced redundant allocations in request sources.
- Split large modules into smaller units across replay, distributed controller, wasm scripting, and app error paths for maintainability.
- Simplified startup wiring by moving entry/main orchestration into clearer module boundaries.
- Replaced magic numbers with documented constants in UI and runtime paths to make tuning and invariants explicit.
- Fixed replay JSON/JSONL parsing regressions and strengthened replay test coverage.
- Updated the live TUI with semantic metric colors, improved status code distribution rendering, compact/scaled number formatting, centered chart axis labels, and consistent themed splash/background rendering.

## 0.1.6

Released: 2026-02-11

- Fixed streaming latency percentile charts to render in chronological order and avoid diagonal artifacts.
- Accepted `NO_COLOR=1` (and other common boolean values) without CLI parse errors.
- Split streaming latency percentile charts into separate All vs OK plots for improved readability.
- Filled missing seconds in streaming percentile series to avoid misleading line interpolation.
- Switched streaming latency percentile buckets to 100ms with a `--charts-latency-bucket-ms` override.
- Filled missing buckets using the last observed percentile values for smoother charts.

## 0.1.5

Released: 2026-02-10

- Changed project license to Apache-2.0.
- Honored `--no-color` across TUI, progress bar, and logging output.

## 0.1.4

Released: 2026-02-09

- Added streaming chart aggregation to avoid unbounded in-memory metric growth.
- Added `--rss-log-ms` to periodically log RSS when the UI is disabled (Linux).
- Added `--alloc-profiler-ms`, `--alloc-profiler-dump-ms`, and `--alloc-profiler-dump-path` behind the `alloc-profiler` build feature.
- Added `--pool-max-idle-per-host` and `--pool-idle-timeout-ms` to tune the HTTP connection pool.
- Added `legacy-charts` build feature to keep the pre-streaming chart pipeline available.
- Switched response body handling to streaming drain to avoid buffering full bodies per request.
- Added `--requests` to stop after N total requests.
- Added `--connect-timeout` to control connection establishment timeout.
- Added `--accept` (`-A`) and `--content-type` (`-T`) header shortcuts.
- Added `--data-file` (`-D`) and `--data-lines` (`-Z`) for body sourcing.
- Added `--connections` alias for `--max-tasks`.
- Added `--no-tui` as the preferred UI disable flag (`--no-ui` remains as an alias).
- Added fuzz coverage for usability-oriented CLI flags.
- Added `--db-url` to persist per-request metrics into sqlite.
- Added `--output`/`--output-format` support for text/quiet output and JSON/JSONL/CSV exports.
- Added URL generation flags: `--urls-from-file`, `--rand-regex-url`, `--max-repeat`, `--dump-urls`.
- Added multipart form uploads via `--form` (`-F`).
- Added auth and signing flags: `--basic-auth`, `--aws-session`, `--aws-sigv4`.
- Added TLS flags: `--cacert`, `--cert`, `--key`, `--insecure`.
- Added HTTP version/proxy flags: `--http-version`, `--proxy-header`, `--proxy-http-version`, `--proxy-http2`.
- Added redirect/connection flags: `--redirect`, `--disable-keepalive`, `--disable-compression`.
- Added DNS and socket flags: `--connect-to`, `--host`, `--no-pre-lookup`, `--ipv4`, `--ipv6`, `--unix-socket`.
- Added UI/stat flags: `--no-color`, `--fps`, `--stats-success-breakdown`.
- Added deadline handling and time-unit controls: `--wait-ongoing-requests-after-deadline`, `--time-unit`.
- Added HTTP/2 concurrency and burst controls: `--http2-parallel`, `--burst-delay`, `--burst-rate`, `--latency-correction`.
- Added `-n` (requests) and `-q` (rate) short flags for parity.
- Reassigned `-n` from `--no-charts` to `--requests`.
- Added fuzz targets for rand-regex URL generation and multipart form entries.

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
