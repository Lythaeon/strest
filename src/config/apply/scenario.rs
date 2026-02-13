use crate::args::{HttpMethod, Scenario, ScenarioStep, parse_header};
use crate::error::{AppError, AppResult, ConfigError};

use super::super::types::{SCENARIO_SCHEMA_VERSION, ScenarioConfig};

#[derive(Debug, Clone)]
pub(crate) struct ScenarioDefaults {
    pub(crate) base_url: Option<String>,
    pub(crate) method: HttpMethod,
    pub(crate) body: String,
    pub(crate) headers: Vec<(String, String)>,
}

impl ScenarioDefaults {
    #[must_use]
    pub(crate) const fn new(
        base_url: Option<String>,
        method: HttpMethod,
        body: String,
        headers: Vec<(String, String)>,
    ) -> Self {
        Self {
            base_url,
            method,
            body,
            headers,
        }
    }
}

pub(crate) fn parse_scenario(
    config: &ScenarioConfig,
    defaults: &ScenarioDefaults,
) -> AppResult<Scenario> {
    if let Some(schema_version) = config.schema_version
        && schema_version != SCENARIO_SCHEMA_VERSION
    {
        return Err(AppError::config(ConfigError::UnsupportedScenarioSchema {
            version: schema_version,
        }));
    }

    if config.steps.is_empty() {
        return Err(AppError::config(ConfigError::ScenarioMissingSteps));
    }

    let base_url = config
        .base_url
        .clone()
        .or_else(|| defaults.base_url.clone());
    let default_method = config.method.unwrap_or(defaults.method);
    let default_body = config.data.clone().unwrap_or_else(|| defaults.body.clone());

    let default_headers = if let Some(headers) = config.headers.as_ref() {
        let mut parsed = Vec::with_capacity(headers.len());
        for header in headers {
            parsed.push(
                parse_header(header)
                    .map_err(|err| AppError::config(ConfigError::InvalidHeader { source: err }))?,
            );
        }
        parsed
    } else {
        defaults.headers.clone()
    };

    let vars = config.vars.clone().unwrap_or_default();
    let mut steps = Vec::with_capacity(config.steps.len());

    for (idx, step) in config.steps.iter().enumerate() {
        let method = step.method.unwrap_or(default_method);
        let mut headers = default_headers.clone();
        if let Some(step_headers) = step.headers.as_ref() {
            for header in step_headers {
                headers.push(
                    parse_header(header).map_err(|err| {
                        AppError::config(ConfigError::InvalidHeader { source: err })
                    })?,
                );
            }
        }

        let think_time = match step.think_time.as_ref() {
            Some(value) => Some(value.to_duration()?),
            None => None,
        };

        let url = step.url.clone();
        let path = step.path.clone();
        if url.is_none() && path.is_none() && base_url.is_none() {
            return Err(AppError::config(
                ConfigError::ScenarioStepMissingUrlOrPath {
                    index: idx.saturating_add(1),
                },
            ));
        }

        steps.push(ScenarioStep {
            name: step.name.clone(),
            method,
            url,
            path,
            headers,
            body: step.data.clone().map_or_else(
                || {
                    if default_body.is_empty() {
                        None
                    } else {
                        Some(default_body.clone())
                    }
                },
                Some,
            ),
            assert_status: step.assert_status,
            assert_body_contains: step.assert_body_contains.clone(),
            think_time,
            vars: step.vars.clone().unwrap_or_default(),
        });
    }

    Ok(Scenario {
        base_url,
        vars,
        steps,
    })
}
