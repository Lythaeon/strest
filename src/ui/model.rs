use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ReplayUi {
    pub playing: bool,
    pub window_start_ms: u64,
    pub window_end_ms: u64,
    pub cursor_ms: u64,
    pub snapshot_start_ms: Option<u64>,
    pub snapshot_end_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct UiData {
    pub elapsed_time: Duration,
    pub target_duration: Duration,
    pub current_requests: u64,
    pub successful_requests: u64,
    pub timeout_requests: u64,
    pub transport_errors: u64,
    pub non_expected_status: u64,
    pub ui_window_ms: u64,
    pub no_color: bool,
    pub latencies: Vec<(u64, u64)>,
    pub p50: u64,
    pub p90: u64,
    pub p99: u64,
    pub p50_ok: u64,
    pub p90_ok: u64,
    pub p99_ok: u64,
    pub rps: u64,
    pub rpm: u64,
    pub replay: Option<ReplayUi>,
}

#[derive(Clone)]
pub struct UiRenderData {
    pub elapsed_time: Duration,
    pub target_duration: Duration,
    pub current_request: u64,
    pub successful_requests: u64,
    pub timeout_requests: u64,
    pub transport_errors: u64,
    pub non_expected_status: u64,
    pub ui_window_ms: u64,
    pub no_color: bool,
    pub latencies: Vec<(u64, u64)>,
    pub p50: u64,
    pub p90: u64,
    pub p99: u64,
    pub p50_ok: u64,
    pub p90_ok: u64,
    pub p99_ok: u64,
    pub rps: u64,
    pub rpm: u64,
    pub replay: Option<ReplayUi>,
}

impl Default for UiData {
    fn default() -> Self {
        Self {
            elapsed_time: Duration::from_secs(0),
            target_duration: Duration::from_secs(0),
            current_requests: 0,
            successful_requests: 0,
            timeout_requests: 0,
            transport_errors: 0,
            non_expected_status: 0,
            ui_window_ms: 10_000,
            no_color: false,
            latencies: Vec::new(),
            p50: 0,
            p90: 0,
            p99: 0,
            p50_ok: 0,
            p90_ok: 0,
            p99_ok: 0,
            rps: 0,
            rpm: 0,
            replay: None,
        }
    }
}

impl From<&UiData> for UiRenderData {
    fn from(data: &UiData) -> Self {
        Self {
            elapsed_time: data.elapsed_time,
            target_duration: data.target_duration,
            current_request: data.current_requests,
            successful_requests: data.successful_requests,
            timeout_requests: data.timeout_requests,
            transport_errors: data.transport_errors,
            non_expected_status: data.non_expected_status,
            ui_window_ms: data.ui_window_ms,
            no_color: data.no_color,
            latencies: data.latencies.clone(),
            p50: data.p50,
            p90: data.p90,
            p99: data.p99,
            p50_ok: data.p50_ok,
            p90_ok: data.p90_ok,
            p99_ok: data.p99_ok,
            rps: data.rps,
            rpm: data.rpm,
            replay: data.replay.clone(),
        }
    }
}
