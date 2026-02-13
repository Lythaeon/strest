use strest_wasm_plugin_sdk::{
    ArtifactPayload, MetricsSummaryPayload, Plugin, RunEndPayload, RunStartPayload, run_plugin,
};

struct EchoHookLogger;

impl Plugin for EchoHookLogger {
    fn on_run_start(&mut self, payload: &RunStartPayload) -> Result<(), String> {
        eprintln!(
            "[echo-hook-logger] start protocol={} mode={} url={}",
            payload.protocol,
            payload.load_mode,
            payload.url.as_deref().unwrap_or("<none>")
        );
        Ok(())
    }

    fn on_metrics_summary(&mut self, payload: &MetricsSummaryPayload) -> Result<(), String> {
        eprintln!(
            "[echo-hook-logger] summary total={} success={} errors={} avg_latency_ms={}",
            payload.total_requests,
            payload.successful_requests,
            payload.error_requests,
            payload.avg_latency_ms
        );
        Ok(())
    }

    fn on_artifact(&mut self, payload: &ArtifactPayload) -> Result<(), String> {
        eprintln!(
            "[echo-hook-logger] artifact kind={} path={}",
            payload.kind, payload.path
        );
        Ok(())
    }

    fn on_run_end(&mut self, payload: &RunEndPayload) -> Result<(), String> {
        eprintln!(
            "[echo-hook-logger] run_end runtime_error_count={}",
            payload.runtime_error_count
        );
        Ok(())
    }
}

fn main() {
    let mut plugin = EchoHookLogger;
    std::process::exit(run_plugin(&mut plugin));
}
