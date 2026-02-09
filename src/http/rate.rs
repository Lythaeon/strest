use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::time::interval;

use crate::args::LoadProfile;

#[derive(Clone)]
pub(crate) struct RatePlan {
    pub initial_rpm: u64,
    pub stages: Vec<RateStage>,
}

#[derive(Clone)]
pub(crate) struct RateStage {
    pub duration_secs: u64,
    pub target_rpm: u64,
}

pub(crate) struct RateController {
    pub plan: RatePlan,
    pub stage_idx: usize,
    pub stage_elapsed_secs: u64,
    pub stage_start_rpm: u64,
    pub remainder: u64,
}

impl RateController {
    pub(crate) fn next_tokens(&mut self) -> usize {
        let rpm = self.current_rpm();
        let (base, rem) = div_mod_u64(rpm, 60);
        let (carry, new_rem) = div_mod_u64(self.remainder.saturating_add(rem), 60);
        self.remainder = new_rem;
        let tokens = base.saturating_add(carry);
        usize::try_from(tokens).unwrap_or(usize::MAX)
    }

    pub(crate) fn current_rpm(&mut self) -> u64 {
        let stage = match self.plan.stages.get(self.stage_idx) {
            Some(stage) => stage,
            None => return self.stage_start_rpm,
        };

        let stage_secs = stage.duration_secs.max(1);
        let elapsed = self.stage_elapsed_secs.min(stage_secs);

        let start = i128::from(self.stage_start_rpm);
        let target = i128::from(stage.target_rpm);
        let elapsed_i128 = i128::from(elapsed);
        let stage_secs_i128 = i128::from(stage_secs);

        let delta = target.saturating_sub(start);
        let step = delta
            .saturating_mul(elapsed_i128)
            .checked_div(stage_secs_i128)
            .unwrap_or(0);
        let rpm_i128 = start.saturating_add(step);
        let rpm = if rpm_i128 < 0 {
            0
        } else {
            u64::try_from(rpm_i128).unwrap_or(u64::MAX)
        };

        self.stage_elapsed_secs = self.stage_elapsed_secs.saturating_add(1);
        if self.stage_elapsed_secs >= stage_secs {
            self.stage_start_rpm = stage.target_rpm;
            self.stage_idx = self.stage_idx.saturating_add(1);
            self.stage_elapsed_secs = 0;
        }

        rpm
    }
}

pub(super) fn build_rate_limiter(
    rate_limit: Option<u64>,
    load_profile: Option<&LoadProfile>,
    burst_delay: Option<Duration>,
    burst_rate: usize,
) -> Option<Arc<Semaphore>> {
    if let Some(profile) = load_profile {
        let plan = RatePlan::from(profile);
        let limiter = Arc::new(Semaphore::new(0));
        spawn_rate_controller(limiter.clone(), plan);
        return Some(limiter);
    }

    if let Some(rate) = rate_limit {
        let limiter = Arc::new(Semaphore::new(0));
        spawn_fixed_rate_controller(limiter.clone(), rate);
        return Some(limiter);
    }

    if let Some(delay) = burst_delay {
        let limiter = Arc::new(Semaphore::new(0));
        spawn_burst_rate_controller(limiter.clone(), delay, burst_rate);
        return Some(limiter);
    }

    None
}

fn spawn_fixed_rate_controller(limiter: Arc<Semaphore>, rate: u64) {
    tokio::spawn(async move {
        let rate_per_sec = usize::try_from(rate).unwrap_or(usize::MAX);
        limiter.add_permits(rate_per_sec);
        let mut rate_tick = interval(Duration::from_secs(1));
        loop {
            rate_tick.tick().await;
            let available = limiter.available_permits();
            if available < rate_per_sec {
                limiter.add_permits(rate_per_sec.saturating_sub(available));
            }
        }
    });
}

fn spawn_rate_controller(limiter: Arc<Semaphore>, plan: RatePlan) {
    tokio::spawn(async move {
        let initial_rpm = plan.initial_rpm;
        let mut controller = RateController {
            plan,
            stage_idx: 0,
            stage_elapsed_secs: 0,
            stage_start_rpm: initial_rpm,
            remainder: 0,
        };
        let initial = controller.next_tokens();
        limiter.add_permits(initial);

        let mut rate_tick = interval(Duration::from_secs(1));
        loop {
            rate_tick.tick().await;
            let available = limiter.available_permits();
            let target = controller.next_tokens();
            if available < target {
                limiter.add_permits(target.saturating_sub(available));
            }
        }
    });
}

fn spawn_burst_rate_controller(limiter: Arc<Semaphore>, delay: Duration, burst_rate: usize) {
    tokio::spawn(async move {
        let burst = burst_rate.max(1);
        limiter.add_permits(burst);
        let mut burst_tick = interval(delay.max(Duration::from_millis(1)));
        loop {
            burst_tick.tick().await;
            let available = limiter.available_permits();
            if available < burst {
                limiter.add_permits(burst.saturating_sub(available));
            }
        }
    });
}

fn div_mod_u64(value: u64, divisor: u64) -> (u64, u64) {
    if divisor == 0 {
        return (0, 0);
    }
    let div = value.checked_div(divisor).unwrap_or(0);
    let rem = value.checked_rem(divisor).unwrap_or(0);
    (div, rem)
}

impl RatePlan {
    fn from(profile: &LoadProfile) -> Self {
        let stages = profile
            .stages
            .iter()
            .map(|stage| RateStage {
                duration_secs: stage.duration.as_secs().max(1),
                target_rpm: stage.target_rpm,
            })
            .collect();
        Self {
            initial_rpm: profile.initial_rpm,
            stages,
        }
    }
}
