use rand::distributions::Distribution;
use rand::thread_rng;

use crate::app::{self, run_cleanup, run_compare, run_local, run_replay};
use crate::error::{AppError, AppResult, ValidationError};
use crate::system::banner;

use super::types::{DumpUrlsPlan, RunPlan};

pub(crate) async fn execute_plan(plan: RunPlan) -> AppResult<()> {
    match plan {
        RunPlan::Cleanup(cleanup_args) => run_cleanup(&cleanup_args).await,
        RunPlan::Compare(compare_args) => run_compare(&compare_args).await,
        RunPlan::Replay(args) => {
            banner::print_cli_banner(args.no_color);
            println!();
            run_replay(&args).await
        }
        RunPlan::DumpUrls(plan) => dump_urls(plan),
        RunPlan::Service(args) => {
            crate::service::handle_service_action(&args)?;
            Ok(())
        }
        RunPlan::Controller { args, scenarios } => {
            banner::print_cli_banner(args.no_color);
            println!();
            crate::distributed::run_controller(&args, scenarios).await
        }
        RunPlan::Agent(args) => {
            banner::print_cli_banner(args.no_color);
            println!();
            crate::distributed::run_agent(args).await
        }
        RunPlan::Local(local) => {
            banner::print_cli_banner(local.args.no_color);
            println!();
            let outcome = match run_local(local.args, None, None).await {
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
