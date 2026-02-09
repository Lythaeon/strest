#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;
use strest::args::HttpMethod;
use strest::config::types::{DurationValue, ScenarioConfig, ScenarioStepConfig, SCENARIO_SCHEMA_VERSION};

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

fn take_ascii_string(data: &[u8], cursor: &mut usize, max_len: usize) -> String {
    if *cursor >= data.len() {
        return String::new();
    }
    let len = usize::from(data[*cursor]);
    *cursor = cursor.saturating_add(1);
    let end = (*cursor).saturating_add(len).min(data.len());
    let slice = &data[*cursor..end];
    *cursor = end;

    let mut out = String::with_capacity(slice.len().min(max_len));
    for &byte in slice.iter().take(max_len) {
        let ch = if byte.is_ascii_graphic() || byte == b' ' {
            byte as char
        } else {
            '.'
        };
        out.push(ch);
    }
    out
}

fn take_method(data: &[u8], cursor: &mut usize) -> HttpMethod {
    let selector = take_u64(data, cursor) % 5;
    match selector {
        0 => HttpMethod::Get,
        1 => HttpMethod::Post,
        2 => HttpMethod::Patch,
        3 => HttpMethod::Put,
        _ => HttpMethod::Delete,
    }
}

fn take_headers(data: &[u8], cursor: &mut usize) -> Option<Vec<String>> {
    if !take_bool(data, cursor) {
        return None;
    }

    let count = (take_u64(data, cursor) % 3) as usize;
    if count == 0 {
        return Some(Vec::new());
    }

    let mut headers = Vec::with_capacity(count);
    for _ in 0..count {
        let key = take_ascii_string(data, cursor, 32);
        let value = take_ascii_string(data, cursor, 64);
        headers.push(format!("{}: {}", key, value));
    }
    Some(headers)
}

fn take_vars(data: &[u8], cursor: &mut usize) -> Option<BTreeMap<String, String>> {
    if !take_bool(data, cursor) {
        return None;
    }

    let count = (take_u64(data, cursor) % 3) as usize;
    let mut vars = BTreeMap::new();
    for _ in 0..count {
        let key = take_ascii_string(data, cursor, 32);
        let value = take_ascii_string(data, cursor, 64);
        if !key.is_empty() {
            vars.insert(key, value);
        }
    }

    Some(vars)
}

fn take_duration_value(data: &[u8], cursor: &mut usize) -> Option<DurationValue> {
    if !take_bool(data, cursor) {
        return None;
    }

    if take_bool(data, cursor) {
        Some(DurationValue::Seconds(take_u64(data, cursor) % 10_000))
    } else {
        let number = (take_u64(data, cursor) % 10_000).saturating_add(1);
        let unit = match take_u64(data, cursor) % 4 {
            0 => "ms",
            1 => "s",
            2 => "m",
            _ => "h",
        };
        Some(DurationValue::Text(format!("{}{}", number, unit)))
    }
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mut cursor = 0usize;
    let base_url = take_ascii_string(data, &mut cursor, 200);
    let base_url = if base_url.is_empty() { None } else { Some(base_url) };

    let method = if take_bool(data, &mut cursor) {
        Some(take_method(data, &mut cursor))
    } else {
        None
    };

    let headers = take_headers(data, &mut cursor);
    let data_body = take_ascii_string(data, &mut cursor, 256);
    let data_body = if data_body.is_empty() { None } else { Some(data_body) };

    let vars = take_vars(data, &mut cursor);

    let step_count = ((take_u64(data, &mut cursor) % 3) + 1) as usize;
    let mut steps = Vec::with_capacity(step_count);

    for _ in 0..step_count {
        let step_method = if take_bool(data, &mut cursor) {
            Some(take_method(data, &mut cursor))
        } else {
            None
        };
        let url = take_ascii_string(data, &mut cursor, 200);
        let path = take_ascii_string(data, &mut cursor, 200);
        let mut url_opt = if url.is_empty() { None } else { Some(url) };
        let mut path_opt = if path.is_empty() { None } else { Some(path) };

        if base_url.is_none() && url_opt.is_none() && path_opt.is_none() {
            path_opt = Some("/".to_owned());
        }

        let step_headers = take_headers(data, &mut cursor);
        let step_body = take_ascii_string(data, &mut cursor, 256);
        let step_body = if step_body.is_empty() { None } else { Some(step_body) };
        let assert_status = if take_bool(data, &mut cursor) {
            Some((take_u64(data, &mut cursor) % 600) as u16)
        } else {
            None
        };
        let assert_body_contains = if take_bool(data, &mut cursor) {
            let body = take_ascii_string(data, &mut cursor, 128);
            if body.is_empty() { None } else { Some(body) }
        } else {
            None
        };

        let think_time = take_duration_value(data, &mut cursor);
        let step_vars = take_vars(data, &mut cursor);

        steps.push(ScenarioStepConfig {
            name: None,
            method: step_method,
            url: url_opt,
            path: path_opt,
            headers: step_headers,
            data: step_body,
            assert_status,
            assert_body_contains,
            think_time,
            vars: step_vars,
        });
    }

    let config = ScenarioConfig {
        base_url,
        method,
        headers,
        data: data_body,
        vars,
        steps,
    };

    let result = strest::fuzzing::parse_scenario_config_input(&config);
    if result.is_ok() {
        if let Some(schema_version) = config.schema_version {
            debug_assert_eq!(schema_version, SCENARIO_SCHEMA_VERSION);
        }
        debug_assert!(!config.steps.is_empty());
        if config.base_url.is_none() {
            for step in &config.steps {
                debug_assert!(step.url.is_some() || step.path.is_some());
            }
        }
    }
});
