use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Url;

use crate::{
    args::{Scenario, ScenarioStep},
    error::{AppError, AppResult, HttpError},
};

pub(super) fn resolve_step_url(
    scenario: &Scenario,
    step: &ScenarioStep,
    vars: &BTreeMap<String, String>,
) -> AppResult<Url> {
    if let Some(url) = step.url.as_ref() {
        let rendered = render_template(url, vars);
        return Url::parse(&rendered).map_err(|err| {
            AppError::http(HttpError::InvalidScenarioUrl {
                url: rendered,
                source: err,
            })
        });
    }

    let path = step
        .path
        .as_ref()
        .ok_or_else(|| AppError::http(HttpError::ScenarioStepMissingUrlOrPath))?;
    let base_url = scenario
        .base_url
        .as_ref()
        .ok_or_else(|| AppError::http(HttpError::ScenarioBaseUrlRequired))?;
    let rendered_path = render_template(path, vars);
    let base = Url::parse(base_url).map_err(|err| {
        AppError::http(HttpError::InvalidScenarioBaseUrl {
            url: base_url.to_owned(),
            source: err,
        })
    })?;
    let joined = base.join(&rendered_path).map_err(|err| {
        AppError::http(HttpError::JoinUrlFailed {
            url: rendered_path,
            source: err,
        })
    })?;
    Ok(joined)
}

pub(super) fn build_template_vars(
    scenario: &Scenario,
    step: &ScenarioStep,
    seq: u64,
    step_index: usize,
) -> BTreeMap<String, String> {
    let mut vars = scenario.vars.clone();
    for (key, value) in &step.vars {
        vars.insert(key.clone(), value.clone());
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    vars.insert("seq".to_owned(), seq.to_string());
    vars.insert("step".to_owned(), step_index.saturating_add(1).to_string());
    vars.insert("timestamp_ms".to_owned(), now_ms.to_string());
    vars.insert("timestamp_s".to_owned(), (now_ms / 1000).to_string());

    vars
}

pub(crate) fn render_template(input: &str, vars: &BTreeMap<String, String>) -> String {
    let mut rest = input;
    let mut output = String::with_capacity(input.len());

    loop {
        let start = match rest.find("{{") {
            Some(start) => start,
            None => {
                output.push_str(rest);
                break;
            }
        };
        let (before, after_start) = rest.split_at(start);
        output.push_str(before);
        let after = match after_start.strip_prefix("{{") {
            Some(after) => after,
            None => {
                output.push_str(after_start);
                break;
            }
        };
        let end = match after.find("}}") {
            Some(end) => end,
            None => {
                output.push_str("{{");
                output.push_str(after);
                break;
            }
        };
        let (key_part, after_end) = after.split_at(end);
        let key = key_part.trim();
        if let Some(value) = vars.get(key) {
            output.push_str(value);
        } else {
            output.push_str("{{");
            output.push_str(key);
            output.push_str("}}");
        }
        rest = match after_end.strip_prefix("}}") {
            Some(remaining) => remaining,
            None => {
                output.push_str(after_end);
                break;
            }
        };
    }

    output
}

pub(super) fn step_label(step: &ScenarioStep, step_index: usize) -> String {
    step.name
        .clone()
        .unwrap_or_else(|| format!("step {}", step_index.saturating_add(1)))
}
