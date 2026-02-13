use strest_wasm_plugin_sdk::{MetricsSummaryPayload, Plugin, RunEndPayload, run_plugin};

const DEFAULT_MIN_SUCCESS_PCT: u64 = 99;
const ENV_MIN_SUCCESS_PCT: &str = "STREST_MIN_SUCCESS_PCT";

struct SloGuard {
    min_success_pct: u64,
}

impl SloGuard {
    fn from_env() -> Self {
        let min_success_pct = std::env::var(ENV_MIN_SUCCESS_PCT)
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .map(|value| value.min(100))
            .unwrap_or(DEFAULT_MIN_SUCCESS_PCT);
        Self { min_success_pct }
    }
}

impl Plugin for SloGuard {
    fn on_metrics_summary(&mut self, payload: &MetricsSummaryPayload) -> Result<(), String> {
        if payload.total_requests == 0 {
            return Ok(());
        }

        let success_times_100 = payload.successful_requests.saturating_mul(100);
        let target = payload.total_requests.saturating_mul(self.min_success_pct);
        if success_times_100 < target {
            return Err(format!(
                "SLO breach: success={} total={} min_success_pct={}",
                payload.successful_requests, payload.total_requests, self.min_success_pct
            ));
        }
        Ok(())
    }

    fn on_run_end(&mut self, payload: &RunEndPayload) -> Result<(), String> {
        if payload.runtime_error_count > 0 {
            return Err(format!(
                "run ended with runtime errors: {}",
                payload.runtime_error_count
            ));
        }
        Ok(())
    }
}

fn main() {
    let mut plugin = SloGuard::from_env();
    std::process::exit(run_plugin(&mut plugin));
}
