use crate::args::{LoadProfile, LoadStage};
use crate::error::{AppError, AppResult, ConfigError};

use super::super::types::{LoadConfig, LoadStageConfig};

pub(super) fn parse_load_profile(load: &LoadConfig) -> AppResult<LoadProfile> {
    let initial_rpm = resolve_rpm(load.rate, load.rpm, "load")?.unwrap_or(0);

    let mut stages = Vec::new();
    if let Some(stage_configs) = load.stages.as_ref() {
        for (idx, stage) in stage_configs.iter().enumerate() {
            let duration = super::super::parse_duration_value(&stage.duration)?;
            let target_rpm = resolve_stage_rpm(stage, idx)?;
            stages.push(LoadStage {
                duration,
                target_rpm,
            });
        }
    }

    if initial_rpm == 0 && stages.is_empty() {
        return Err(AppError::config(
            ConfigError::LoadProfileMissingRateOrStages,
        ));
    }

    Ok(LoadProfile {
        initial_rpm,
        stages,
    })
}

pub(super) fn parse_simple_load(rate: Option<u64>, rpm: Option<u64>) -> AppResult<LoadProfile> {
    let initial_rpm = resolve_rpm(rate, rpm, "rate/rpm")?.unwrap_or(0);
    if initial_rpm == 0 {
        return Err(AppError::config(ConfigError::RateRpmMustBePositive));
    }

    Ok(LoadProfile {
        initial_rpm,
        stages: Vec::new(),
    })
}

fn resolve_stage_rpm(stage: &LoadStageConfig, idx: usize) -> AppResult<u64> {
    let mut configured = 0u8;
    if stage.target.is_some() {
        configured = configured.saturating_add(1);
    }
    if stage.rate.is_some() {
        configured = configured.saturating_add(1);
    }
    if stage.rpm.is_some() {
        configured = configured.saturating_add(1);
    }

    let stage_index = idx.saturating_add(1);
    if configured == 0 {
        return Err(AppError::config(ConfigError::StageMissingTargetRateRpm {
            index: stage_index,
        }));
    }
    if configured > 1 {
        return Err(AppError::config(
            ConfigError::StageConflictingTargetRateRpm { index: stage_index },
        ));
    }

    if let Some(rpm) = stage.rpm {
        return Ok(rpm);
    }

    let rate = stage.target.or(stage.rate).unwrap_or(0);
    Ok(rate.saturating_mul(60))
}

fn resolve_rpm(rate: Option<u64>, rpm: Option<u64>, context: &str) -> AppResult<Option<u64>> {
    if rate.is_some() && rpm.is_some() {
        return Err(AppError::config(ConfigError::RateRpmConflict {
            context: context.to_owned(),
        }));
    }
    if let Some(rpm) = rpm {
        return Ok(Some(rpm));
    }
    if let Some(rate) = rate {
        return Ok(Some(rate.saturating_mul(60)));
    }
    Ok(None)
}
