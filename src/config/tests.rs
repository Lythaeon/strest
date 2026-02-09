use super::{
    apply_config, load_config_file, parse_duration_value,
    types::{
        ConfigFile, DistributedConfig, DurationValue, LoadConfig, LoadStageConfig, ScenarioConfig,
        ScenarioStepConfig,
    },
};
use clap::{CommandFactory, FromArgMatches};
use std::time::Duration;
use tempfile::tempdir;

use crate::args::TesterArgs;

#[test]
fn parse_toml_config_with_load_stages() -> Result<(), String> {
    let dir = tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
    let path = dir.path().join("strest.toml");
    let content = r#"
url = "http://localhost:3000"
method = "get"
duration = 60
rate = 100

[load]
rate = 200

[[load.stages]]
duration = "10s"
target = 500
"#;
    std::fs::write(&path, content).map_err(|err| format!("write failed: {}", err))?;

    let config = load_config_file(&path)?;
    if config.url.as_deref() != Some("http://localhost:3000") {
        return Err("Unexpected url".to_owned());
    }
    let load = match config.load {
        Some(load) => load,
        None => return Err("Expected load".to_owned()),
    };
    let stages = match load.stages {
        Some(stages) => stages,
        None => return Err("Expected stages".to_owned()),
    };
    let first = match stages.first() {
        Some(stage) => stage,
        None => return Err("Missing stage".to_owned()),
    };
    if first.duration != "10s" {
        return Err(format!("Unexpected duration: {}", first.duration));
    }
    if first.target != Some(500) {
        return Err("Unexpected target".to_owned());
    }

    Ok(())
}

#[test]
fn parse_json_config_with_load_stages() -> Result<(), String> {
    let dir = tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
    let path = dir.path().join("strest.json");
    let content = r#"{
  "url": "http://localhost:3000",
  "method": "get",
  "duration": 60,
  "load": {
    "rate": 200,
    "stages": [
      { "duration": "10s", "target": 500 }
    ]
  }
}"#;
    std::fs::write(&path, content).map_err(|err| format!("write failed: {}", err))?;

    let config = load_config_file(&path)?;
    if config.url.as_deref() != Some("http://localhost:3000") {
        return Err("Unexpected url".to_owned());
    }
    let load = match config.load {
        Some(load) => load,
        None => return Err("Expected load".to_owned()),
    };
    if load.rate != Some(200) {
        return Err("Unexpected load rate".to_owned());
    }
    let stages = match load.stages {
        Some(stages) => stages,
        None => return Err("Expected stages".to_owned()),
    };
    let first = match stages.first() {
        Some(stage) => stage,
        None => return Err("Missing stage".to_owned()),
    };
    if first.target != Some(500) {
        return Err("Unexpected target".to_owned());
    }

    Ok(())
}

#[test]
fn apply_config_parses_aliases_and_timeout() -> Result<(), String> {
    let dir = tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
    let path = dir.path().join("strest.toml");
    let content = r#"
url = "http://localhost:3000"
proxy = "http://127.0.0.1:8080"
concurrency = 42
timeout = "5s"
connect_timeout = "3s"
accept = "application/json"
content_type = "text/plain"
requests = 12
redirect = 0
disable_keepalive = true
disable_compression = true
http_version = "2"
proxy_headers = ["Proxy-Auth: secret"]
proxy_http2 = true
connect_to = ["example.com:443:localhost:8443"]
host = "example.com"
ipv4 = true
no_pre_lookup = true
no_color = true
fps = 24
stats_success_breakdown = true
unix_socket = "/tmp/strest.sock"
insecure = true
cacert = "ca.pem"
cert = "client.pem"
key = "client.key"
basic_auth = "user:pass"
aws_session = "token"
aws_sigv4 = "aws:amz:us-east-1:service"
"#;
    std::fs::write(&path, content).map_err(|err| format!("write failed: {}", err))?;

    let config = load_config_file(&path)?;

    let cmd = TesterArgs::command();
    let matches = cmd.get_matches_from(["strest"]);
    let mut args = TesterArgs::from_arg_matches(&matches)
        .map_err(|err| format!("parse args failed: {}", err))?;

    apply_config(&mut args, &matches, &config)?;

    if args.proxy_url.as_deref() != Some("http://127.0.0.1:8080") {
        return Err("Unexpected proxy_url".to_owned());
    }
    if args.max_tasks.get() != 42 {
        return Err(format!("Unexpected max_tasks: {}", args.max_tasks.get()));
    }
    if args.request_timeout != Duration::from_secs(5) {
        return Err(format!(
            "Unexpected request_timeout: {:?}",
            args.request_timeout
        ));
    }
    if args.connect_timeout != Duration::from_secs(3) {
        return Err(format!(
            "Unexpected connect_timeout: {:?}",
            args.connect_timeout
        ));
    }
    if args.accept_header.as_deref() != Some("application/json") {
        return Err("Unexpected accept_header".to_owned());
    }
    if args.content_type.as_deref() != Some("text/plain") {
        return Err("Unexpected content_type".to_owned());
    }
    if args.requests.map(u64::from) != Some(12) {
        return Err("Unexpected requests".to_owned());
    }
    if args.redirect_limit != 0 {
        return Err(format!(
            "Unexpected redirect_limit: {}",
            args.redirect_limit
        ));
    }
    if !args.disable_keepalive {
        return Err("Expected disable_keepalive to be true".to_owned());
    }
    if !args.disable_compression {
        return Err("Expected disable_compression to be true".to_owned());
    }
    if args.http_version != Some(crate::args::HttpVersion::V2) {
        return Err("Unexpected http_version".to_owned());
    }
    if args.proxy_headers.len() != 1 {
        return Err("Unexpected proxy_headers".to_owned());
    }
    if !args.proxy_http2 {
        return Err("Expected proxy_http2 to be true".to_owned());
    }
    if args.connect_to.len() != 1 {
        return Err("Unexpected connect_to".to_owned());
    }
    if args.host_header.as_deref() != Some("example.com") {
        return Err("Unexpected host_header".to_owned());
    }
    if !args.ipv4_only {
        return Err("Expected ipv4_only to be true".to_owned());
    }
    if !args.no_pre_lookup {
        return Err("Expected no_pre_lookup to be true".to_owned());
    }
    if !args.no_color {
        return Err("Expected no_color to be true".to_owned());
    }
    if args.ui_fps != 24 {
        return Err("Unexpected ui_fps".to_owned());
    }
    if !args.stats_success_breakdown {
        return Err("Expected stats_success_breakdown to be true".to_owned());
    }
    if args.unix_socket.as_deref() != Some("/tmp/strest.sock") {
        return Err("Unexpected unix_socket".to_owned());
    }
    if args.basic_auth.as_deref() != Some("user:pass") {
        return Err("Unexpected basic_auth".to_owned());
    }
    if args.aws_session.as_deref() != Some("token") {
        return Err("Unexpected aws_session".to_owned());
    }
    if args.aws_sigv4.as_deref() != Some("aws:amz:us-east-1:service") {
        return Err("Unexpected aws_sigv4".to_owned());
    }
    if !args.insecure {
        return Err("Expected insecure to be true".to_owned());
    }
    if args.cacert.as_deref() != Some("ca.pem") {
        return Err("Unexpected cacert".to_owned());
    }
    if args.cert.as_deref() != Some("client.pem") {
        return Err("Unexpected cert".to_owned());
    }
    if args.key.as_deref() != Some("client.key") {
        return Err("Unexpected key".to_owned());
    }

    Ok(())
}

#[test]
fn apply_config_rejects_ipv4_ipv6_conflict() -> Result<(), String> {
    let config = ConfigFile {
        ipv4: Some(true),
        ipv6: Some(true),
        ..ConfigFile::default()
    };

    let cmd = TesterArgs::command();
    let matches = cmd.get_matches_from(["strest"]);
    let mut args = TesterArgs::from_arg_matches(&matches)
        .map_err(|err| format!("parse args failed: {}", err))?;

    if apply_config(&mut args, &matches, &config).is_ok() {
        return Err("Expected ipv4/ipv6 conflict error".to_owned());
    }

    Ok(())
}

#[test]
fn apply_config_rejects_conflicting_body_sources() -> Result<(), String> {
    let config = ConfigFile {
        data: Some("inline".to_owned()),
        data_file: Some("payload.txt".to_owned()),
        ..ConfigFile::default()
    };

    let cmd = TesterArgs::command();
    let matches = cmd.get_matches_from(["strest"]);
    let mut args = TesterArgs::from_arg_matches(&matches)
        .map_err(|err| format!("parse args failed: {}", err))?;

    if apply_config(&mut args, &matches, &config).is_ok() {
        return Err("Expected conflict error".to_owned());
    }

    Ok(())
}

#[test]
fn apply_config_respects_cli_overrides() -> Result<(), String> {
    let config = ConfigFile {
        url: Some("http://from-config".to_owned()),
        no_charts: Some(false),
        ..ConfigFile::default()
    };

    let cmd = TesterArgs::command();
    let matches = cmd.get_matches_from(["strest", "--url", "http://from-cli", "--no-charts"]);
    let mut args = TesterArgs::from_arg_matches(&matches)
        .map_err(|err| format!("parse args failed: {}", err))?;

    apply_config(&mut args, &matches, &config)?;

    if args.url.as_deref() != Some("http://from-cli") {
        return Err("Expected CLI url to win".to_owned());
    }
    if !args.no_charts {
        return Err("Expected CLI no_charts to win".to_owned());
    }

    Ok(())
}

#[test]
fn apply_config_load_profile_rate_to_rpm() -> Result<(), String> {
    let config = ConfigFile {
        load: Some(LoadConfig {
            rate: Some(10),
            rpm: None,
            stages: Some(vec![LoadStageConfig {
                duration: "5s".to_owned(),
                target: Some(20),
                rate: None,
                rpm: None,
            }]),
        }),
        ..ConfigFile::default()
    };

    let cmd = TesterArgs::command();
    let matches = cmd.get_matches_from(["strest"]);
    let mut args = TesterArgs::from_arg_matches(&matches)
        .map_err(|err| format!("parse args failed: {}", err))?;

    apply_config(&mut args, &matches, &config)?;

    let load = match args.load_profile {
        Some(load) => load,
        None => return Err("Expected load_profile".to_owned()),
    };
    if load.initial_rpm != 600 {
        return Err(format!(
            "Expected initial_rpm 600, got {}",
            load.initial_rpm
        ));
    }
    let stage = load
        .stages
        .first()
        .ok_or_else(|| "Missing stage".to_owned())?;
    if stage.target_rpm != 1200 {
        return Err(format!(
            "Expected target_rpm 1200, got {}",
            stage.target_rpm
        ));
    }

    Ok(())
}

#[test]
fn apply_config_rejects_load_and_rate_conflict() -> Result<(), String> {
    let config = ConfigFile {
        load: Some(LoadConfig {
            rate: Some(10),
            rpm: None,
            stages: None,
        }),
        rate: Some(5),
        ..ConfigFile::default()
    };

    let cmd = TesterArgs::command();
    let matches = cmd.get_matches_from(["strest"]);
    let mut args = TesterArgs::from_arg_matches(&matches)
        .map_err(|err| format!("parse args failed: {}", err))?;

    let result = apply_config(&mut args, &matches, &config);
    if result.is_err() {
        Ok(())
    } else {
        Err("Expected error when load and rate/rpm configured".to_owned())
    }
}

#[test]
fn parse_duration_value_accepts_units() -> Result<(), String> {
    let secs = parse_duration_value("10s")?;
    if secs != Duration::from_secs(10) {
        return Err("Unexpected seconds duration".to_owned());
    }
    let ms = parse_duration_value("500ms")?;
    if ms != Duration::from_millis(500) {
        return Err("Unexpected milliseconds duration".to_owned());
    }
    let mins = parse_duration_value("2m")?;
    if mins != Duration::from_secs(120) {
        return Err("Unexpected minutes duration".to_owned());
    }
    let hours = parse_duration_value("1h")?;
    if hours != Duration::from_secs(3600) {
        return Err("Unexpected hours duration".to_owned());
    }
    Ok(())
}

#[test]
fn apply_config_sets_warmup_and_tls() -> Result<(), String> {
    let config = ConfigFile {
        warmup: Some(DurationValue::Text("10s".to_owned())),
        tls_min: Some(crate::args::TlsVersion::V1_2),
        tls_max: Some(crate::args::TlsVersion::V1_3),
        ..ConfigFile::default()
    };

    let cmd = TesterArgs::command();
    let matches = cmd.get_matches_from(["strest"]);
    let mut args = TesterArgs::from_arg_matches(&matches)
        .map_err(|err| format!("parse args failed: {}", err))?;

    apply_config(&mut args, &matches, &config)?;

    if args.warmup != Some(Duration::from_secs(10)) {
        return Err("Expected warmup to be 10s".to_owned());
    }
    if args.tls_min != Some(crate::args::TlsVersion::V1_2) {
        return Err("Expected tls_min V1_2".to_owned());
    }
    if args.tls_max != Some(crate::args::TlsVersion::V1_3) {
        return Err("Expected tls_max V1_3".to_owned());
    }

    Ok(())
}

#[test]
fn apply_config_parses_scenario() -> Result<(), String> {
    let config = ConfigFile {
        url: Some("http://localhost".to_owned()),
        scenario: Some(ScenarioConfig {
            schema_version: None,
            base_url: Some("http://example.com".to_owned()),
            method: Some(crate::args::HttpMethod::Post),
            headers: Some(vec!["X-Test: 123".to_owned()]),
            data: Some("body".to_owned()),
            vars: None,
            steps: vec![ScenarioStepConfig {
                name: Some("step 1".to_owned()),
                method: None,
                url: None,
                path: Some("/test".to_owned()),
                headers: None,
                data: None,
                assert_status: Some(201),
                assert_body_contains: Some("ok".to_owned()),
                think_time: Some(DurationValue::Text("1s".to_owned())),
                vars: None,
            }],
        }),
        ..ConfigFile::default()
    };

    let cmd = TesterArgs::command();
    let matches = cmd.get_matches_from(["strest"]);
    let mut args = TesterArgs::from_arg_matches(&matches)
        .map_err(|err| format!("parse args failed: {}", err))?;

    apply_config(&mut args, &matches, &config)?;

    let scenario = match args.scenario {
        Some(scenario) => scenario,
        None => return Err("Expected scenario".to_owned()),
    };
    if scenario.base_url.as_deref() != Some("http://example.com") {
        return Err("Unexpected base_url".to_owned());
    }
    let step = scenario
        .steps
        .first()
        .ok_or_else(|| "Missing step".to_owned())?;
    if step.method != crate::args::HttpMethod::Post {
        return Err("Unexpected step method".to_owned());
    }
    if step.path.as_deref() != Some("/test") {
        return Err("Unexpected step path".to_owned());
    }
    if step.assert_status != Some(201) {
        return Err("Unexpected step assert_status".to_owned());
    }
    if step.think_time != Some(Duration::from_secs(1)) {
        return Err("Unexpected step think_time".to_owned());
    }

    Ok(())
}

#[test]
fn apply_config_sets_distributed_fields() -> Result<(), String> {
    let config = ConfigFile {
        distributed: Some(DistributedConfig {
            role: Some("agent".to_owned()),
            controller_mode: Some(crate::args::ControllerMode::Manual),
            listen: None,
            control_listen: Some("127.0.0.1:9010".to_owned()),
            control_auth_token: Some("control-token".to_owned()),
            join: Some("127.0.0.1:9009".to_owned()),
            auth_token: Some("token".to_owned()),
            agent_id: Some("agent-1".to_owned()),
            weight: Some(2),
            min_agents: Some(3),
            agent_wait_timeout_ms: Some(2500),
            agent_standby: Some(true),
            agent_reconnect_ms: Some(1500),
            agent_heartbeat_interval_ms: Some(900),
            agent_heartbeat_timeout_ms: Some(3200),
            ..DistributedConfig::default()
        }),
        ..ConfigFile::default()
    };

    let cmd = TesterArgs::command();
    let matches = cmd.get_matches_from(["strest"]);
    let mut args = TesterArgs::from_arg_matches(&matches)
        .map_err(|err| format!("parse args failed: {}", err))?;

    apply_config(&mut args, &matches, &config)?;

    if args.agent_join.as_deref() != Some("127.0.0.1:9009") {
        return Err("Unexpected agent_join".to_owned());
    }
    if args.controller_mode != crate::args::ControllerMode::Manual {
        return Err("Unexpected controller_mode".to_owned());
    }
    if args.control_listen.as_deref() != Some("127.0.0.1:9010") {
        return Err("Unexpected control_listen".to_owned());
    }
    if args.control_auth_token.as_deref() != Some("control-token") {
        return Err("Unexpected control_auth_token".to_owned());
    }
    if args.auth_token.as_deref() != Some("token") {
        return Err("Unexpected auth_token".to_owned());
    }
    if args.agent_id.as_deref() != Some("agent-1") {
        return Err("Unexpected agent_id".to_owned());
    }
    if args.agent_weight.get() != 2 {
        return Err("Unexpected agent_weight".to_owned());
    }
    if args.min_agents.get() != 3 {
        return Err("Unexpected min_agents".to_owned());
    }
    let wait_timeout = match args.agent_wait_timeout_ms {
        Some(value) => value.get(),
        None => return Err("Expected agent_wait_timeout_ms to be set".to_owned()),
    };
    if wait_timeout != 2500 {
        return Err(format!(
            "Unexpected agent_wait_timeout_ms: {}",
            wait_timeout
        ));
    }
    if !args.agent_standby {
        return Err("Unexpected agent_standby".to_owned());
    }
    if args.agent_reconnect_ms.get() != 1500 {
        return Err("Unexpected agent_reconnect_ms".to_owned());
    }
    if args.agent_heartbeat_interval_ms.get() != 900 {
        return Err("Unexpected agent_heartbeat_interval_ms".to_owned());
    }
    if args.agent_heartbeat_timeout_ms.get() != 3200 {
        return Err("Unexpected agent_heartbeat_timeout_ms".to_owned());
    }

    Ok(())
}
