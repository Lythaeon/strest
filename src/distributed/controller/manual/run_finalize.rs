use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};

use super::super::output::finalize_output;
use super::state::ManualRunState;

pub(super) async fn finalize_manual_run(
    args: &TesterArgs,
    state: &mut ManualRunState,
) -> AppResult<()> {
    finalize_output(
        args,
        &mut state.output_state,
        &state.agent_states,
        &mut state.runtime_errors,
    )
    .await;

    if !state.runtime_errors.is_empty() {
        eprintln!("Runtime errors:");
        for err in &state.runtime_errors {
            eprintln!("- {}", err);
        }
        return Err(AppError::distributed(
            DistributedError::RunCompletedWithErrors,
        ));
    }

    Ok(())
}
