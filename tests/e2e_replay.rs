mod support_single;

use std::fs;

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
fn e2e_replay_from_tmp_logs() -> Result<(), String> {
    let Some((url, _server)) = spawn_http_server_or_skip()? else {
        return Ok(());
    };
    let (_dir, charts_path, tmp_path) = prep_paths()?;

    let args = vec![
        "-u".to_owned(),
        url,
        "-t".to_owned(),
        "2".to_owned(),
        "--no-tui".to_owned(),
        "--summary".to_owned(),
        "--no-charts".to_owned(),
        "--keep-tmp".to_owned(),
        "--tmp-path".to_owned(),
        tmp_path.clone(),
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

    let replay_args = vec!["--replay".to_owned(), "--tmp-path".to_owned(), tmp_path];
    let replay_output = run_strest(replay_args)?;
    if !replay_output.status.success() {
        return Err(format!(
            "replay stdout: {}\nreplay stderr: {}",
            String::from_utf8_lossy(&replay_output.stdout),
            String::from_utf8_lossy(&replay_output.stderr)
        ));
    }

    Ok(())
}

#[test]
fn e2e_replay_from_exports() -> Result<(), String> {
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
        "--no-tui".to_owned(),
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

    let replay_csv_args = vec![
        "--replay".to_owned(),
        "--export-csv".to_owned(),
        export_csv.to_string_lossy().into_owned(),
    ];
    let replay_csv = run_strest(replay_csv_args)?;
    if !replay_csv.status.success() {
        return Err(format!(
            "replay csv stdout: {}\nreplay csv stderr: {}",
            String::from_utf8_lossy(&replay_csv.stdout),
            String::from_utf8_lossy(&replay_csv.stderr)
        ));
    }

    let replay_json_args = vec![
        "--replay".to_owned(),
        "--export-json".to_owned(),
        export_json.to_string_lossy().into_owned(),
    ];
    let replay_json = run_strest(replay_json_args)?;
    if !replay_json.status.success() {
        return Err(format!(
            "replay json stdout: {}\nreplay json stderr: {}",
            String::from_utf8_lossy(&replay_json.stdout),
            String::from_utf8_lossy(&replay_json.stderr)
        ));
    }

    let replay_jsonl_args = vec![
        "--replay".to_owned(),
        "--export-jsonl".to_owned(),
        export_jsonl.to_string_lossy().into_owned(),
    ];
    let replay_jsonl = run_strest(replay_jsonl_args)?;
    if !replay_jsonl.status.success() {
        return Err(format!(
            "replay jsonl stdout: {}\nreplay jsonl stderr: {}",
            String::from_utf8_lossy(&replay_jsonl.stdout),
            String::from_utf8_lossy(&replay_jsonl.stderr)
        ));
    }

    Ok(())
}
