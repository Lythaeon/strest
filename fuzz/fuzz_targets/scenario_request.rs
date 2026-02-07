#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;
use strest::args::{HttpMethod, Scenario, ScenarioStep};

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

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mut cursor = 0usize;
    let method = match data[0] % 5 {
        0 => HttpMethod::Get,
        1 => HttpMethod::Post,
        2 => HttpMethod::Patch,
        3 => HttpMethod::Put,
        _ => HttpMethod::Delete,
    };
    cursor = cursor.saturating_add(1);

    let base_url = take_ascii_string(data, &mut cursor, 200);
    let url = take_ascii_string(data, &mut cursor, 200);
    let path = take_ascii_string(data, &mut cursor, 200);
    let header_key = take_ascii_string(data, &mut cursor, 64);
    let header_value = take_ascii_string(data, &mut cursor, 128);
    let body = take_ascii_string(data, &mut cursor, 256);
    let scenario_var_key = take_ascii_string(data, &mut cursor, 32);
    let scenario_var_value = take_ascii_string(data, &mut cursor, 64);
    let step_var_key = take_ascii_string(data, &mut cursor, 32);
    let step_var_value = take_ascii_string(data, &mut cursor, 64);

    let seq = take_u64(data, &mut cursor);
    let step_index = take_u64(data, &mut cursor) as usize;

    let mut scenario_vars = BTreeMap::new();
    if !scenario_var_key.is_empty() {
        scenario_vars.insert(scenario_var_key, scenario_var_value);
    }
    if !body.is_empty() {
        scenario_vars.insert("input".to_owned(), body.clone());
    }

    let mut step_vars = BTreeMap::new();
    if !step_var_key.is_empty() {
        step_vars.insert(step_var_key, step_var_value);
    }

    let headers = if header_key.is_empty() {
        Vec::new()
    } else {
        vec![(header_key, header_value)]
    };

    let step = ScenarioStep {
        name: None,
        method,
        url: if url.is_empty() { None } else { Some(url) },
        path: if path.is_empty() { None } else { Some(path) },
        headers,
        body: if body.is_empty() { None } else { Some(body) },
        assert_status: None,
        assert_body_contains: None,
        think_time: None,
        vars: step_vars,
    };

    let scenario = Scenario {
        base_url: if base_url.is_empty() { None } else { Some(base_url) },
        vars: scenario_vars,
        steps: vec![step],
    };

    if let Some(step_ref) = scenario.steps.first() {
        let _result = strest::fuzzing::build_scenario_request_input(
            &scenario,
            step_ref,
            seq,
            step_index,
        );
    }
});
