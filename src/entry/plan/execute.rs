use rand::distributions::Distribution;
use rand::thread_rng;

use crate::adapters::runtime::{
    RuntimeCleanupPort, RuntimeComparePort, RuntimeDistributedPort, RuntimeLocalPort,
    RuntimeReplayPort, RuntimeServicePort, print_runtime_errors,
};
use crate::application::{distributed_run, slice_execution};
use crate::domain::run::RunConfig;
use crate::error::{AppError, AppResult, ValidationError};
use crate::system::banner;

use super::types::{DumpUrlsPlan, RunPlan};

pub(crate) async fn execute_plan(plan: RunPlan) -> AppResult<()> {
    match plan {
        RunPlan::Cleanup(cleanup_args) => {
            let cleanup_port = RuntimeCleanupPort;
            slice_execution::execute_cleanup(cleanup_args, &cleanup_port).await
        }
        RunPlan::Compare(compare_args) => {
            let compare_port = RuntimeComparePort;
            slice_execution::execute_compare(compare_args, &compare_port).await
        }
        RunPlan::Replay { command, args } => {
            log_run_command("replay", command.run_config());
            banner::print_cli_banner(command.no_color());
            println!();
            let replay_port = RuntimeReplayPort;
            slice_execution::execute_replay(args, &replay_port).await
        }
        RunPlan::DumpUrls(plan) => dump_urls(plan),
        RunPlan::Service(args) => {
            let service_port = RuntimeServicePort;
            slice_execution::execute_service(args, &service_port)
        }
        RunPlan::Distributed { command, args } => {
            log_run_command(command.mode_name(), command.run_config());
            banner::print_cli_banner(command.no_color());
            println!();
            let distributed_port = RuntimeDistributedPort;
            distributed_run::execute(command, args, &distributed_port).await
        }
        RunPlan::Local { command, args } => {
            log_run_command("local", command.run_config());
            banner::print_cli_banner(command.no_color());
            println!();
            let local_port = RuntimeLocalPort;
            let outcome = match slice_execution::execute_local(args, &local_port).await {
                Ok(outcome) => outcome,
                Err(AppError::Validation(ValidationError::RunCancelled)) => return Ok(()),
                Err(err) => return Err(err),
            };
            if !outcome.runtime_errors.is_empty() {
                print_runtime_errors(&outcome.runtime_errors);
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
