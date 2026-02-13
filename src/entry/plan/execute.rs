use rand::distributions::Distribution;
use rand::thread_rng;

use crate::app::{self, run_cleanup, run_compare, run_local, run_replay};
use crate::domain::run::RunConfig;
use crate::error::{AppError, AppResult, ValidationError};
use crate::system::banner;

use super::types::{DumpUrlsPlan, RunPlan};

pub(crate) async fn execute_plan(plan: RunPlan) -> AppResult<()> {
    match plan {
        RunPlan::Cleanup(cleanup_args) => run_cleanup(&cleanup_args).await,
        RunPlan::Compare(compare_args) => run_compare(&compare_args).await,
        RunPlan::Replay(command) => {
            log_run_command("replay", command.run_config());
            banner::print_cli_banner(command.no_color());
            println!();
            run_replay(command.as_args()).await
        }
        RunPlan::DumpUrls(plan) => dump_urls(plan),
        RunPlan::Service(command) => {
            crate::service::handle_service_action(command.as_args())?;
            Ok(())
        }
        RunPlan::Controller(command) => {
            log_run_command("controller", command.run_config());
            banner::print_cli_banner(command.no_color());
            println!();
            let (args, scenarios) = command.into_parts();
            crate::distributed::run_controller(&args, scenarios).await
        }
        RunPlan::Agent(command) => {
            log_run_command("agent", command.run_config());
            banner::print_cli_banner(command.no_color());
            println!();
            crate::distributed::run_agent(command.into_args()).await
        }
        RunPlan::Local(command) => {
            log_run_command("local", command.run_config());
            banner::print_cli_banner(command.no_color());
            println!();
            let outcome = match run_local(command.into_args(), None, None).await {
                Ok(outcome) => outcome,
                Err(AppError::Validation(ValidationError::RunCancelled)) => return Ok(()),
                Err(err) => return Err(err),
            };
            if !outcome.runtime_errors.is_empty() {
                app::print_runtime_errors(&outcome.runtime_errors);
                return Err(AppError::validation(ValidationError::RuntimeErrors));
            }
            Ok(())
        }
    }
}

fn log_run_command(kind: &str, run_config: &RunConfig) {
    let target = run_config.target_url.as_deref().unwrap_or("<scenario>");
    let scenario_steps = run_config.scenario_step_count();
    let scenario_vars = run_config.scenario_vars_count();
    let scenario_base_url = run_config.scenario_base_url().unwrap_or("<none>");
    tracing::debug!(
        "Executing {} command: protocol={}, load_mode={}, target={}, scenario_steps={}, scenario_vars={}, scenario_base_url={}",
        kind,
        run_config.protocol.as_str(),
        run_config.load_mode.as_str(),
        target,
        scenario_steps,
        scenario_vars,
        scenario_base_url
    );
}

fn dump_urls(plan: DumpUrlsPlan) -> AppResult<()> {
    let regex = rand_regex::Regex::compile(&plan.pattern, plan.max_repeat).map_err(|err| {
        AppError::validation(ValidationError::InvalidRandRegex {
            pattern: plan.pattern,
            source: err,
        })
    })?;
    let mut rng = thread_rng();
    for _ in 0..plan.count {
        let url: String = regex.sample(&mut rng);
        println!("{}", url);
    }
    Ok(())
}
