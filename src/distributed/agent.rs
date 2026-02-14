mod command;
mod run_exec;
mod session;
mod wire;

use std::time::Duration;

use tracing::{info, warn};

use crate::args::TesterArgs;
use crate::error::AppResult;
pub(crate) use run_exec::{AgentLocalRunPort, AgentRunOutcome};

/// Runs the distributed agent loop.
///
/// # Errors
///
/// Returns an error if the agent cannot connect, negotiate, or execute a run.
pub async fn run_agent<TLocalRunPort>(
    args: TesterArgs,
    local_run_port: &TLocalRunPort,
) -> AppResult<()>
where
    TLocalRunPort: AgentLocalRunPort + Sync,
{
    let standby = args.agent_standby;
    let reconnect_delay = Duration::from_millis(args.agent_reconnect_ms.get());
    info!(
        "Agent starting (standby={}, reconnect={}ms)",
        standby,
        reconnect_delay.as_millis()
    );

    loop {
        let result = session::run_agent_session(&args, local_run_port).await;
        match result {
            Ok(()) => {
                if !standby {
                    return Ok(());
                }
            }
            Err(err) => {
                if !standby {
                    return Err(err);
                }
                warn!("Agent session error: {}", err);
            }
        }
        tokio::time::sleep(reconnect_delay).await;
    }
}
