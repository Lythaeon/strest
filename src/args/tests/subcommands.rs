use super::*;

#[test]
fn parse_args_quick_subcommand() -> AppResult<()> {
    let args = TesterArgs::try_parse_from([
        "strest",
        "quick",
        "--url",
        "http://localhost",
        "--duration",
        "20",
    ])?;
    match args.command {
        Some(Command::Quick(preset)) => {
            if preset.url != "http://localhost" {
                return Err(AppError::validation("Unexpected quick url"));
            }
            if preset.target_duration.get() != 20 {
                return Err(AppError::validation("Unexpected quick duration"));
            }
            Ok(())
        }
        _ => Err(AppError::validation("Expected quick subcommand")),
    }
}

#[test]
fn parse_args_distributed_subcommand() -> AppResult<()> {
    let args = TesterArgs::try_parse_from([
        "strest",
        "distributed",
        "--url",
        "http://localhost",
        "--agents",
        "3",
    ])?;
    match args.command {
        Some(Command::Distributed(preset)) => {
            if preset.agents.get() != 3 {
                return Err(AppError::validation("Unexpected agents value"));
            }
            if preset.url != "http://localhost" {
                return Err(AppError::validation("Unexpected distributed url"));
            }
            Ok(())
        }
        _ => Err(AppError::validation("Expected distributed subcommand")),
    }
}
