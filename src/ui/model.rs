use std::time::Duration;

#[derive(Debug, Clone)]
pub struct UiData {
    pub elapsed_time: Duration,
    pub target_duration: Duration,
    pub current_requests: u64,
    pub successful_requests: u64,
    pub latencies: Vec<(u64, u64)>,
    pub p50: u64,
    pub p90: u64,
    pub p99: u64,
    pub rps: u64,
    pub rpm: u64,
}

#[derive(Clone)]
pub struct UiRenderData {
    pub elapsed_time: Duration,
    pub target_duration: Duration,
    pub current_request: u64,
    pub successful_requests: u64,
    pub latencies: Vec<(u64, u64)>,
    pub p50: u64,
    pub p90: u64,
    pub p99: u64,
    pub rps: u64,
    pub rpm: u64,
}

impl Default for UiData {
    fn default() -> Self {
        Self {
            elapsed_time: Duration::from_secs(0),
            target_duration: Duration::from_secs(0),
            current_requests: 0,
            successful_requests: 0,
            latencies: Vec::new(),
            p50: 0,
            p90: 0,
            p99: 0,
            rps: 0,
            rpm: 0,
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
            latencies: data.latencies.clone(),
            p50: data.p50,
            p90: data.p90,
            p99: data.p99,
            rps: data.rps,
            rpm: data.rpm,
        }
    }
}
