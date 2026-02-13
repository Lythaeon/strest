use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::sinks::config::{PrometheusSinkConfig, SinksConfig};

use super::{
    allocate_port, base_args, positive_u64, run_async_test, run_distributed,
    spawn_http_server_or_skip,
};

#[test]
fn tcp_streaming_controller_writes_sink() -> AppResult<()> {
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
        let sink_path = tmp_dir.path().join("controller.prom");
        let sink_path_str = sink_path
            .to_str()
            .ok_or_else(|| AppError::distributed("Failed to convert sink path"))?
            .to_owned();

        let mut controller_args = base_args(url.clone(), tmp_path.clone())?;
        controller_args.controller_listen = Some(controller_addr.clone());
        controller_args.sinks = Some(SinksConfig {
            update_interval_ms: Some(200),
            prometheus: Some(PrometheusSinkConfig {
                path: sink_path_str.clone(),
            }),
            otel: None,
            influx: None,
        });
        controller_args.distributed_stream_summaries = true;
        controller_args.distributed_stream_interval_ms = Some(positive_u64(200)?);

        let mut agent_args = base_args(url, tmp_path)?;
        agent_args.agent_join = Some(controller_addr);

        let run_result = tokio::time::timeout(
            Duration::from_secs(12),
            run_distributed(controller_args, agent_args),
        )
        .await
        .map_err(|err| {
            AppError::distributed(format!("Timed out waiting for distributed run: {}", err))
        })?;
        run_result?;

        shutdown_tx
            .send(true)
            .map_err(|err| AppError::distributed(format!("Failed to shutdown server: {}", err)))?;

        let metadata = tokio::fs::metadata(&sink_path_str)
            .await
            .map_err(|err| AppError::distributed(format!("Missing controller sink: {}", err)))?;
        if metadata.len() == 0 {
            return Err(AppError::distributed(
                "Expected controller sink to be non-empty",
            ));
        }
        Ok(())
    })
}

#[test]
fn tcp_non_streaming_writes_agent_and_controller_sinks() -> AppResult<()> {
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
        let controller_sink = tmp_dir.path().join("controller.prom");
        let controller_sink_str = controller_sink
            .to_str()
            .ok_or_else(|| AppError::distributed("Failed to convert controller sink path"))?
            .to_owned();
        let agent_sink = tmp_dir.path().join("agent.prom");
        let agent_sink_str = agent_sink
            .to_str()
            .ok_or_else(|| AppError::distributed("Failed to convert agent sink path"))?
            .to_owned();

        let mut controller_args = base_args(url.clone(), tmp_path.clone())?;
        controller_args.controller_listen = Some(controller_addr.clone());
        controller_args.sinks = Some(SinksConfig {
            update_interval_ms: None,
            prometheus: Some(PrometheusSinkConfig {
                path: controller_sink_str.clone(),
            }),
            otel: None,
            influx: None,
        });

        let mut agent_args = base_args(url, tmp_path)?;
        agent_args.agent_join = Some(controller_addr);
        agent_args.sinks = Some(SinksConfig {
            update_interval_ms: None,
            prometheus: Some(PrometheusSinkConfig {
                path: agent_sink_str.clone(),
            }),
            otel: None,
            influx: None,
        });

        let run_result = tokio::time::timeout(
            Duration::from_secs(12),
            run_distributed(controller_args, agent_args),
        )
        .await
        .map_err(|err| {
            AppError::distributed(format!("Timed out waiting for distributed run: {}", err))
        })?;
        run_result?;

        shutdown_tx
            .send(true)
            .map_err(|err| AppError::distributed(format!("Failed to shutdown server: {}", err)))?;

        let controller_meta = tokio::fs::metadata(&controller_sink_str)
            .await
            .map_err(|err| AppError::distributed(format!("Missing controller sink: {}", err)))?;
        if controller_meta.len() == 0 {
            return Err(AppError::distributed(
                "Expected controller sink to be non-empty",
            ));
        }
        let agent_meta = tokio::fs::metadata(&agent_sink_str)
            .await
            .map_err(|err| AppError::distributed(format!("Missing agent sink: {}", err)))?;
        if agent_meta.len() == 0 {
            return Err(AppError::distributed("Expected agent sink to be non-empty"));
        }
        Ok(())
    })
}
