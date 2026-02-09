mod support_single;

use std::fs;
use std::path::PathBuf;

use tempfile::tempdir;

use support_single::run_strest;
use support_single::spawn_http_server_or_skip;

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

#[test]
fn e2e_single_cli_basic() -> Result<(), String> {
    let Some((url, _server)) = spawn_http_server_or_skip()? else {
        return Ok(());
    };
    let (_dir, charts_path, tmp_path) = prep_paths()?;

    let args = vec![
        "-u".to_owned(),
        url,
        "-t".to_owned(),
        "2".to_owned(),
        "--no-ui".to_owned(),
        "--summary".to_owned(),
        "--no-charts".to_owned(),
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
    ];

    let output = run_strest(args)?;
    if !output.status.success() {
        return Err(format!(
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[test]
fn e2e_single_exports() -> Result<(), String> {
    let Some((url, _server)) = spawn_http_server_or_skip()? else {
        return Ok(());
    };
    let (dir, charts_path, tmp_path) = prep_paths()?;

    let export_csv = dir.path().join("metrics.csv");
    let export_json = dir.path().join("metrics.json");
    let export_jsonl = dir.path().join("metrics.jsonl");

    let args = vec![
        "-u".to_owned(),
        url,
        "-t".to_owned(),
        "2".to_owned(),
        "--no-ui".to_owned(),
        "--summary".to_owned(),
        "--no-charts".to_owned(),
        "--metrics-max".to_owned(),
        "200".to_owned(),
        "--metrics-range".to_owned(),
        "0-1".to_owned(),
        "--export-csv".to_owned(),
        export_csv.to_string_lossy().into_owned(),
        "--export-json".to_owned(),
        export_json.to_string_lossy().into_owned(),
        "--export-jsonl".to_owned(),
        export_jsonl.to_string_lossy().into_owned(),
        "--tmp-path".to_owned(),
        tmp_path,
        "--charts-path".to_owned(),
        charts_path,
    ];

    let output = run_strest(args)?;
    if !output.status.success() {
        return Err(format!(
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if !export_csv.exists() {
        return Err("Expected CSV export to exist.".to_owned());
    }
    if !export_json.exists() {
        return Err("Expected JSON export to exist.".to_owned());
    }
    if !export_jsonl.exists() {
        return Err("Expected JSONL export to exist.".to_owned());
    }
    let csv_meta =
        fs::metadata(&export_csv).map_err(|err| format!("csv metadata failed: {}", err))?;
    let json_meta =
        fs::metadata(&export_json).map_err(|err| format!("json metadata failed: {}", err))?;
    let jsonl_meta =
        fs::metadata(&export_jsonl).map_err(|err| format!("jsonl metadata failed: {}", err))?;
    if csv_meta.len() == 0 {
        return Err("CSV export was empty.".to_owned());
    }
    if json_meta.len() == 0 {
        return Err("JSON export was empty.".to_owned());
    }
    if jsonl_meta.len() == 0 {
        return Err("JSONL export was empty.".to_owned());
    }
    Ok(())
}

#[test]
fn e2e_single_config_toml_load_and_sinks() -> Result<(), String> {
    let Some((url, _server)) = spawn_http_server_or_skip()? else {
        return Ok(());
    };
    let (dir, charts_path, tmp_path) = prep_paths()?;

    let sink_path = dir.path().join("sink.prom");
    let config_path = dir.path().join("strest.toml");
    let config = format!(
        r#"url = "{url}"
duration = 2
no_ui = true
summary = true
no_charts = true
metrics_max = 200

[load]
rate = 10

[[load.stages]]
duration = "1s"
target = 20

[sinks]
update_interval_ms = 200

[sinks.prometheus]
path = "{sink}"
"#,
        url = url,
        sink = sink_path.to_string_lossy()
    );
    fs::write(&config_path, config).map_err(|err| format!("write config failed: {}", err))?;

    let args = vec![
        "--config".to_owned(),
        config_path.to_string_lossy().into_owned(),
        "--tmp-path".to_owned(),
        tmp_path,
        "--charts-path".to_owned(),
        charts_path,
    ];

    let output = run_strest(args)?;
    if !output.status.success() {
        return Err(format!(
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if !sink_path.exists() {
        return Err("Expected sink output to exist.".to_owned());
    }
    let sink_meta =
        fs::metadata(&sink_path).map_err(|err| format!("sink metadata failed: {}", err))?;
    if sink_meta.len() == 0 {
        return Err("Sink output was empty.".to_owned());
    }
    Ok(())
}

#[test]
fn e2e_single_config_json_scenario() -> Result<(), String> {
    let Some((url, _server)) = spawn_http_server_or_skip()? else {
        return Ok(());
    };
    let (_dir, charts_path, tmp_path) = prep_paths()?;

    let config_path = PathBuf::from(tmp_path.clone()).join("strest.json");
    let config = serde_json::json!({
        "url": url,
        "duration": 2,
        "no_ui": true,
        "summary": true,
        "no_charts": true,
        "scenario": {
            "base_url": url,
            "steps": [
                { "method": "get", "path": "/health", "assert_status": 200 }
            ]
        }
    });
    let json_bytes = serde_json::to_vec_pretty(&config)
        .map_err(|err| format!("serialize config failed: {}", err))?;
    fs::write(&config_path, json_bytes).map_err(|err| format!("write config failed: {}", err))?;

    let args = vec![
        "--config".to_owned(),
        config_path.to_string_lossy().into_owned(),
        "--tmp-path".to_owned(),
        tmp_path,
        "--charts-path".to_owned(),
        charts_path,
    ];

    let output = run_strest(args)?;
    if !output.status.success() {
        return Err(format!(
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
