use std::time::Duration;

use crate::error::{AppError, AppResult};

use super::{
    allocate_port, base_args, positive_u64, run_async_test, run_distributed,
    spawn_http_server_or_skip,
};

#[test]
fn distributed_streaming_soak_multiple_runs_remains_stable() -> AppResult<()> {
    run_async_test(async {
        let Some((url, shutdown_tx)) = spawn_http_server_or_skip().await? else {
            return Ok(());
        };
        let tmp_dir = tempfile::tempdir()
            .map_err(|err| AppError::distributed(format!("Failed to create temp dir: {}", err)))?;

        for run_idx in 0..3 {
            let run_tmp = tmp_dir.path().join(format!("soak-run-{}", run_idx));
            std::fs::create_dir_all(&run_tmp).map_err(|err| {
                AppError::distributed(format!(
                    "Failed to create soak run directory {}: {}",
                    run_tmp.display(),
                    err
                ))
            })?;
            let tmp_path = run_tmp
                .to_str()
                .ok_or_else(|| AppError::distributed("Failed to convert tmp path"))?
                .to_owned();

            let controller_port = allocate_port()?;
            let controller_addr = format!("127.0.0.1:{}", controller_port);

            let mut controller_args = base_args(url.clone(), tmp_path.clone())?;
            controller_args.controller_listen = Some(controller_addr.clone());
            controller_args.distributed_stream_summaries = true;
            controller_args.distributed_stream_interval_ms = Some(positive_u64(200)?);
            controller_args.target_duration = positive_u64(1)?;

            let mut agent_args = base_args(url.clone(), tmp_path)?;
            agent_args.agent_join = Some(controller_addr);
            agent_args.distributed_stream_summaries = true;
            agent_args.distributed_stream_interval_ms = Some(positive_u64(200)?);
            agent_args.target_duration = positive_u64(1)?;

            let run_result = tokio::time::timeout(
                Duration::from_secs(15),
                run_distributed(controller_args, agent_args),
            )
            .await
            .map_err(|err| {
                AppError::distributed(format!("Timed out waiting for soak run: {}", err))
            })?;
            run_result.map_err(|err| {
                AppError::distributed(format!("Soak run {} failed: {}", run_idx, err))
            })?;
        }

        shutdown_tx
            .send(true)
            .map_err(|err| AppError::distributed(format!("Failed to shutdown server: {}", err)))?;

        Ok(())
    })
}

#[test]
fn distributed_auth_token_mismatch_fails_fast() -> AppResult<()> {
    run_async_test(async {
        let Some((url, shutdown_tx)) = spawn_http_server_or_skip().await? else {
            return Ok(());
        };
        let controller_port = allocate_port()?;
        let controller_addr = format!("127.0.0.1:{}", controller_port);
        let tmp_dir = tempfile::tempdir()
            .map_err(|err| AppError::distributed(format!("Failed to create temp dir: {}", err)))?;
        let tmp_path = tmp_dir
            .path()
            .to_str()
            .ok_or_else(|| AppError::distributed("Failed to convert tmp path"))?
            .to_owned();

        let mut controller_args = base_args(url.clone(), tmp_path.clone())?;
        controller_args.controller_listen = Some(controller_addr.clone());
        controller_args.auth_token = Some("controller-secret".to_owned());
        controller_args.agent_wait_timeout_ms = Some(positive_u64(500)?);

        let mut agent_args = base_args(url, tmp_path)?;
        agent_args.agent_join = Some(controller_addr);
        agent_args.auth_token = Some("agent-secret-wrong".to_owned());

        let run_result = tokio::time::timeout(
            Duration::from_secs(10),
            run_distributed(controller_args, agent_args),
        )
        .await
        .map_err(|err| {
            AppError::distributed(format!("Timed out waiting for auth failure: {}", err))
        })?;

        shutdown_tx
            .send(true)
            .map_err(|err| AppError::distributed(format!("Failed to shutdown server: {}", err)))?;

        let run_error = match run_result {
            Ok(()) => {
                return Err(AppError::distributed(
                    "Expected distributed auth token mismatch to fail",
                ));
            }
            Err(err) => err,
        };

        let error_text = run_error.to_string().to_ascii_lowercase();
        if !error_text.contains("auth token") {
            return Err(AppError::distributed(format!(
                "Expected auth token mismatch error, got: {}",
                run_error
            )));
        }

        Ok(())
    })
}
