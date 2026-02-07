#![no_main]

use libfuzzer_sys::fuzz_target;
use strest::config::types::{LoadConfig, LoadStageConfig};

fn take_u64(data: &[u8], cursor: &mut usize) -> u64 {
    let mut value = 0u64;
    let mut shift = 0u32;
    while *cursor < data.len() && shift < 64 {
        value |= u64::from(data[*cursor]) << shift;
        *cursor = cursor.saturating_add(1);
        shift = shift.saturating_add(8);
    }
    value
}

fn take_bool(data: &[u8], cursor: &mut usize) -> bool {
    if *cursor >= data.len() {
        return false;
    }
    let value = data[*cursor] % 2 == 0;
    *cursor = cursor.saturating_add(1);
    value
}

fn take_duration_string(data: &[u8], cursor: &mut usize) -> String {
    let number = take_u64(data, cursor).saturating_add(1) % 10_000;
    let unit = match take_u64(data, cursor) % 4 {
        0 => "ms",
        1 => "s",
        2 => "m",
        _ => "h",
    };
    format!("{}{}", number, unit)
}

fn take_stage(data: &[u8], cursor: &mut usize) -> LoadStageConfig {
    let selector = take_u64(data, cursor) % 4;
    let duration = take_duration_string(data, cursor);
    let value = take_u64(data, cursor) % 10_000;

    let (target, rate, rpm) = match selector {
        0 => (Some(value), None, None),
        1 => (None, Some(value), None),
        2 => (None, None, Some(value)),
        _ => (None, None, None),
    };

    LoadStageConfig {
        duration,
        target,
        rate,
        rpm,
    }
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mut cursor = 0usize;
    let rate = if take_bool(data, &mut cursor) {
        Some(take_u64(data, &mut cursor) % 10_000)
    } else {
        None
    };
    let rpm = if take_bool(data, &mut cursor) {
        Some(take_u64(data, &mut cursor) % 10_000)
    } else {
        None
    };

    let stage_count = (take_u64(data, &mut cursor) % 4) as usize;
    let mut stages = Vec::with_capacity(stage_count);
    for _ in 0..stage_count {
        stages.push(take_stage(data, &mut cursor));
    }

    let load = LoadConfig {
        rate,
        rpm,
        stages: if stages.is_empty() { None } else { Some(stages) },
    };

    let _result = strest::fuzzing::apply_load_config_input(load);
});
