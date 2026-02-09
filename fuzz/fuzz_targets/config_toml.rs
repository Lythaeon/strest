#![no_main]

use libfuzzer_sys::fuzz_target;
use strest::config::types::{ConfigFile, SCENARIO_SCHEMA_VERSION};
use strest::metrics::MetricsRange;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let parsed: Option<ConfigFile> = toml::from_str(input).ok();
        let applied = strest::fuzzing::apply_config_from_toml(input);
        if applied.is_ok() {
            if let Some(config) = parsed {
                if let Some(range) = config.metrics_range.as_ref() {
                    debug_assert!(range.parse::<MetricsRange>().is_ok());
                }
                if let Some(scenario) = config.scenario.as_ref() {
                    if let Some(schema_version) = scenario.schema_version {
                        debug_assert_eq!(schema_version, SCENARIO_SCHEMA_VERSION);
                    }
                    debug_assert!(!scenario.steps.is_empty());
                    if scenario.base_url.is_none() {
                        for step in &scenario.steps {
                            debug_assert!(step.url.is_some() || step.path.is_some());
                        }
                    }
                }
                if let Some(scenarios) = config.scenarios.as_ref() {
                    for scenario in scenarios.values() {
                        if let Some(schema_version) = scenario.schema_version {
                            debug_assert_eq!(schema_version, SCENARIO_SCHEMA_VERSION);
                        }
                        debug_assert!(!scenario.steps.is_empty());
                        if scenario.base_url.is_none() {
                            for step in &scenario.steps {
                                debug_assert!(step.url.is_some() || step.path.is_some());
                            }
                        }
                    }
                }
            }
        }
    }
});
