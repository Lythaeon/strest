mod events;
mod finalize;
mod setup;

use crate::args::TesterArgs;
use crate::error::AppResult;

pub(super) async fn run_controller_auto(args: &TesterArgs) -> AppResult<()> {
    let setup = setup::prepare_auto_run(args).await?;
    let outcome = events::collect_auto_run_events(args, setup).await;
    finalize::finalize_auto_run(args, outcome).await
}
