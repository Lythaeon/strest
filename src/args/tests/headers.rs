use super::*;

#[test]
fn parse_header_valid() -> AppResult<()> {
    let parsed = parse_header("Content-Type: application/json");
    match parsed {
        Ok((key, value)) => {
            if key != "Content-Type" {
                return Err(AppError::validation(format!("Unexpected key: {}", key)));
            }
            if value != "application/json" {
                return Err(AppError::validation(format!("Unexpected value: {}", value)));
            }
            Ok(())
        }
        Err(err) => Err(AppError::validation(format!(
            "Expected Ok, got Err: {}",
            err
        ))),
    }
}

#[test]
fn parse_header_invalid() -> AppResult<()> {
    let parsed = parse_header("MissingDelimiter");
    if parsed.is_err() {
        Ok(())
    } else {
        Err(AppError::validation("Expected Err for invalid header"))
    }
}
