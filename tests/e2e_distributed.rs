mod support_distributed;

use std::fs;
use std::time::Duration;

use tempfile::tempdir;

use support_distributed::{
    pick_port, read_child_output, spawn_http_server, spawn_strest, spawn_strest_with_output,
    wait_for_exit,
};

fn prep_paths() -> Result<(tempfile::TempDir, String, String), String> {
    let dir = tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
    let charts = dir.path().join("charts");
    let tmp_dir_path = dir.path().join("tmp");
    fs::create_dir_all(&charts).map_err(|err| format!("create charts dir failed: {}", err))?;
    fs::create_dir_all(&tmp_dir_path).map_err(|err| format!("create tmp dir failed: {}", err))?;
    Ok((
        dir,
        charts.to_string_lossy().into_owned(),
        tmp_dir_path.to_string_lossy().into_owned(),
    ))
}

fn parse_summary_metric(output: &str, label: &str) -> Result<u64, String> {
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix(label) {
            let value = rest.trim();
            let number_str = value.split_whitespace().next().unwrap_or("");
            let parsed = number_str
                .parse::<u64>()
                .map_err(|err| format!("Failed to parse {}: {}", label, err))?;
            return Ok(parsed);
        }
    }
    Err(format!("Missing {} in output.", label))
}

fn run_distributed(streaming: bool) -> Result<(), String> {
    let (url, _server) = spawn_http_server()?;
    let (dir, charts_path, tmp_path) = prep_paths()?;

    let sink_path = dir.path().join("controller.prom");
    let config_path = dir.path().join("controller.json");
    let config = serde_json::json!({
        "sinks": {
            "update_interval_ms": 200,
            "prometheus": { "path": sink_path.to_string_lossy() }
        }
    });
    let config_bytes = serde_json::to_vec_pretty(&config)
        .map_err(|err| format!("serialize config failed: {}", err))?;
    fs::write(&config_path, config_bytes).map_err(|err| format!("write config failed: {}", err))?;

    let port = pick_port()?;
    let listen = format!("127.0.0.1:{}", port);

    let mut controller_args = vec![
        "--controller-listen".to_owned(),
        listen.clone(),
        "-u".to_owned(),
        url,
        "-t".to_owned(),
        "2".to_owned(),
        "--no-ui".to_owned(),
        "--summary".to_owned(),
        "--no-charts".to_owned(),
        "--min-agents".to_owned(),
        "2".to_owned(),
        "--max-tasks".to_owned(),
        "10".to_owned(),
        "--rate".to_owned(),
        "20".to_owned(),
        "--spawn-rate".to_owned(),
        "1".to_owned(),
        "--spawn-interval".to_owned(),
        "100".to_owned(),
        "--tmp-path".to_owned(),
        tmp_path,
        "--charts-path".to_owned(),
        charts_path,
        "--config".to_owned(),
        config_path.to_string_lossy().into_owned(),
    ];

    if streaming {
        controller_args.push("--stream-summaries".to_owned());
        controller_args.push("--stream-interval-ms".to_owned());
        controller_args.push("200".to_owned());
    }

    let mut controller = spawn_strest_with_output(controller_args)?;
    std::thread::sleep(Duration::from_millis(200));

    let agent_args = vec![
        "--agent-join".to_owned(),
        listen,
        "--no-ui".to_owned(),
        "--no-charts".to_owned(),
    ];

    let mut agent_1 = spawn_strest(agent_args.clone())?;
    let mut agent_2 = spawn_strest(agent_args)?;

    let timeout = Duration::from_secs(20);
    let status_controller = wait_for_exit(&mut controller, timeout)?;
    let (controller_stdout, controller_stderr) = read_child_output(&mut controller)?;
    let status_agent_1 = wait_for_exit(&mut agent_1, timeout)?;
    let status_agent_2 = wait_for_exit(&mut agent_2, timeout)?;

    if !status_controller.success() {
        return Err(format!(
            "Controller failed. stdout: {} stderr: {}",
            controller_stdout, controller_stderr
        ));
    }
    if !status_agent_1.success() {
        return Err("Agent 1 failed.".to_owned());
    }
    if !status_agent_2.success() {
        return Err("Agent 2 failed.".to_owned());
    }

    let total = parse_summary_metric(&controller_stdout, "Total Requests:")?;
    if total == 0 {
        return Err("Controller summary reported zero requests.".to_owned());
    }
    let successful = parse_summary_metric(&controller_stdout, "Successful:")?;
    if successful == 0 {
        return Err("Controller summary reported zero successes.".to_owned());
    }
    if !sink_path.exists() {
        return Err("Controller sink output missing.".to_owned());
    }
    let sink_meta =
        fs::metadata(&sink_path).map_err(|err| format!("sink metadata failed: {}", err))?;
    if sink_meta.len() == 0 {
        return Err("Controller sink output was empty.".to_owned());
    }
    Ok(())
}

#[test]
fn e2e_distributed_basic() -> Result<(), String> {
    run_distributed(false)
}

#[test]
fn e2e_distributed_streaming() -> Result<(), String> {
    run_distributed(true)
}
