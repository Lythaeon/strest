use std::collections::HashSet;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};

use super::super::output::finalize_output;
use super::events::AutoRunOutcome;

pub(super) async fn finalize_auto_run(args: &TesterArgs, outcome: AutoRunOutcome) -> AppResult<()> {
    let AutoRunOutcome {
        run_id: _run_id,
        mut output_state,
        agent_states,
        mut runtime_errors,
        channel_closed,
        pending_agents,
    } = outcome;

    append_channel_closure_errors(channel_closed, &pending_agents, &mut runtime_errors);
    finalize_output(args, &mut output_state, &agent_states, &mut runtime_errors).await;

    if !runtime_errors.is_empty() {
        eprintln!("Runtime errors:");
        for err in runtime_errors {
            eprintln!("- {}", err);
        }
        return Err(AppError::distributed(
            DistributedError::RunCompletedWithErrors,
        ));
    }

    Ok(())
}

fn append_channel_closure_errors(
    channel_closed: bool,
    pending_agents: &HashSet<String>,
    runtime_errors: &mut Vec<String>,
) {
    if channel_closed && !pending_agents.is_empty() {
        for agent_id in pending_agents {
            runtime_errors.push(format!(
                "Agent {} disconnected before sending a report.",
                agent_id
            ));
        }
    }
}
