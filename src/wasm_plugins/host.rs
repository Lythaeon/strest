use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::json;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, ScriptError};
use crate::metrics::MetricsSummary;

use super::constants::{
    HOOK_ARTIFACT, HOOK_METRICS_SUMMARY, HOOK_RUN_END, HOOK_RUN_START, MAX_PLUGIN_PAYLOAD_BYTES,
    PLUGIN_API_VERSION, WASMER_BIN,
};
use super::validate::validate_plugin_module;

pub(crate) struct WasmPluginHost {
    plugin_paths: Vec<String>,
}

impl WasmPluginHost {
    pub(crate) fn from_paths(paths: &[String]) -> AppResult<Option<Self>> {
        if paths.is_empty() {
            return Ok(None);
        }

        verify_wasmer_available()?;
        let mut plugin_paths = Vec::with_capacity(paths.len());
        for path in paths {
            let wasm_bytes = std::fs::read(path).map_err(|err| {
                AppError::script(ScriptError::WasmPlugin {
                    message: format!("failed to read plugin '{}': {}", path, err),
                })
            })?;
            validate_plugin_module(path, &wasm_bytes)?;
            plugin_paths.push(path.clone());
        }

        Ok(Some(Self { plugin_paths }))
    }

    pub(crate) fn on_run_start(&mut self, args: &TesterArgs) -> AppResult<()> {
        let payload = json!({
            "event": "run_start",
            "protocol": args.protocol.as_str(),
            "load_mode": args.load_mode.as_str(),
            "url": args.url,
            "duration_seconds": args.target_duration.get(),
            "max_tasks": args.max_tasks.get(),
            "summary": args.summary,
            "no_ui": args.no_ui
        })
        .to_string();
        self.dispatch(HOOK_RUN_START, &payload)
    }

    pub(crate) fn on_metrics_summary(&mut self, summary: &MetricsSummary) -> AppResult<()> {
        let duration_ms = u64::try_from(summary.duration.as_millis()).unwrap_or(u64::MAX);
        let payload = json!({
            "event": "metrics_summary",
            "duration_ms": duration_ms,
            "total_requests": summary.total_requests,
            "successful_requests": summary.successful_requests,
            "error_requests": summary.error_requests,
            "timeout_requests": summary.timeout_requests,
            "transport_errors": summary.transport_errors,
            "non_expected_status": summary.non_expected_status,
            "min_latency_ms": summary.min_latency_ms,
            "max_latency_ms": summary.max_latency_ms,
            "avg_latency_ms": summary.avg_latency_ms,
            "success_min_latency_ms": summary.success_min_latency_ms,
            "success_max_latency_ms": summary.success_max_latency_ms,
            "success_avg_latency_ms": summary.success_avg_latency_ms
        })
        .to_string();
        self.dispatch(HOOK_METRICS_SUMMARY, &payload)
    }

    pub(crate) fn on_artifact(&mut self, kind: &str, path: &str) -> AppResult<()> {
        let payload = json!({
            "event": "artifact",
            "kind": kind,
            "path": path
        })
        .to_string();
        self.dispatch(HOOK_ARTIFACT, &payload)
    }

    pub(crate) fn on_run_end(&mut self, runtime_errors: &[String]) -> AppResult<()> {
        let payload = json!({
            "event": "run_end",
            "runtime_error_count": runtime_errors.len(),
            "runtime_errors": runtime_errors
        })
        .to_string();
        self.dispatch(HOOK_RUN_END, &payload)
    }

    fn dispatch(&self, hook_name: &'static str, payload: &str) -> AppResult<()> {
        if payload.len() > MAX_PLUGIN_PAYLOAD_BYTES {
            return Err(AppError::script(ScriptError::WasmPlugin {
                message: format!(
                    "payload for '{}' exceeds max size ({})",
                    hook_name, MAX_PLUGIN_PAYLOAD_BYTES
                ),
            }));
        }

        for plugin in &self.plugin_paths {
            invoke_plugin(plugin, hook_name, payload)?;
        }
        Ok(())
    }
}

fn verify_wasmer_available() -> AppResult<()> {
    let status = Command::new(WASMER_BIN)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            AppError::script(ScriptError::WasmPlugin {
                message: format!("failed to execute '{}' binary: {}", WASMER_BIN, err),
            })
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::script(ScriptError::WasmPlugin {
            message: format!("'{} --version' returned failure", WASMER_BIN),
        }))
    }
}

fn invoke_plugin(plugin_path: &str, hook_name: &'static str, payload: &str) -> AppResult<()> {
    let mut child = Command::new(WASMER_BIN)
        .arg("run")
        .arg(plugin_path)
        .arg("--")
        .arg(hook_name)
        .env("STREST_PLUGIN_API_VERSION", PLUGIN_API_VERSION.to_string())
        .env("STREST_PLUGIN_HOOK", hook_name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            AppError::script(ScriptError::WasmPlugin {
                message: format!("failed to start plugin '{}': {}", plugin_path, err),
            })
        })?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(payload.as_bytes()).map_err(|err| {
            AppError::script(ScriptError::WasmPlugin {
                message: format!(
                    "failed writing payload to plugin '{}': {}",
                    plugin_path, err
                ),
            })
        })?;
    }

    let output = child.wait_with_output().map_err(|err| {
        AppError::script(ScriptError::WasmPlugin {
            message: format!("failed waiting for plugin '{}': {}", plugin_path, err),
        })
    })?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(AppError::script(ScriptError::WasmPlugin {
            message: format!(
                "plugin '{}' hook '{}' failed (status={}): {}",
                plugin_path, hook_name, output.status, stderr
            ),
        }))
    }
}
