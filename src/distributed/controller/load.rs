use crate::args::{LoadProfile, TesterArgs};

pub(super) fn apply_load_share(
    agent_args: &mut super::super::protocol::WireArgs,
    args: &TesterArgs,
    weights: &[u64],
    idx: usize,
) {
    let use_weights = weights.iter().any(|value| *value != 1);
    let share_weights: Vec<u64> = if use_weights {
        weights.to_vec()
    } else {
        vec![1; weights.len()]
    };

    if let Some(profile) = args.load_profile.as_ref() {
        let split = split_load_profile(profile, &share_weights);
        if let Some(agent_profile) = split.get(idx) {
            agent_args.load_profile = Some(agent_profile.clone());
            agent_args.rate_limit = None;
        }
        return;
    }

    if let Some(rate) = args.rate_limit.map(u64::from) {
        let shares = split_total(rate, &share_weights);
        if let Some(share) = shares.get(idx) {
            agent_args.rate_limit = Some(*share);
        }
    }
}

fn split_load_profile(
    profile: &LoadProfile,
    weights: &[u64],
) -> Vec<super::super::protocol::WireLoadProfile> {
    let initial_shares = split_total(profile.initial_rpm, weights);
    let mut stage_shares: Vec<Vec<u64>> = Vec::new();
    for stage in &profile.stages {
        stage_shares.push(split_total(stage.target_rpm, weights));
    }

    let mut per_agent = Vec::with_capacity(weights.len());
    for idx in 0..weights.len() {
        let mut stages = Vec::with_capacity(profile.stages.len());
        for (stage_idx, stage) in profile.stages.iter().enumerate() {
            let share = stage_shares
                .get(stage_idx)
                .and_then(|values| values.get(idx))
                .copied()
                .unwrap_or(0);
            stages.push(super::super::protocol::WireLoadStage {
                duration_secs: stage.duration.as_secs(),
                target_rpm: share,
            });
        }
        let initial_rpm = initial_shares.get(idx).copied().unwrap_or(0);
        per_agent.push(super::super::protocol::WireLoadProfile {
            initial_rpm,
            stages,
        });
    }

    per_agent
}

fn split_total(total: u64, weights: &[u64]) -> Vec<u64> {
    if weights.is_empty() {
        return Vec::new();
    }
    let total_weight: u128 = weights.iter().map(|value| u128::from(*value)).sum();
    if total_weight == 0 {
        return vec![0; weights.len()];
    }

    let mut shares = vec![0u64; weights.len()];
    let mut remainder = u128::from(total);
    for (idx, weight) in weights.iter().enumerate() {
        let share = u128::from(total)
            .saturating_mul(u128::from(*weight))
            .checked_div(total_weight)
            .unwrap_or(0);
        if let Some(slot) = shares.get_mut(idx) {
            *slot = u64::try_from(share).unwrap_or(u64::MAX);
        }
        remainder = remainder.saturating_sub(share);
    }

    let mut idx = 0usize;
    while remainder > 0 {
        if let Some(value) = shares.get_mut(idx) {
            *value = value.saturating_add(1);
        }
        remainder = remainder.saturating_sub(1);
        idx = idx.saturating_add(1);
        if idx >= shares.len() {
            idx = 0;
        }
    }

    shares
}
