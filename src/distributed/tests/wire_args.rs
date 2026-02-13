use crate::error::{AppError, AppResult};

use super::{WireArgs, apply_wire_args, base_args, build_wire_args, positive_u64};

#[test]
fn wire_args_roundtrip_preserves_stream_settings() -> AppResult<()> {
    let tmp_path = "./tmp".to_owned();
    let mut args = base_args("http://localhost".to_owned(), tmp_path.clone())?;
    args.distributed_stream_summaries = true;
    args.distributed_stream_interval_ms = Some(positive_u64(150)?);

    let wire = build_wire_args(&args);
    let mut applied = base_args("http://localhost".to_owned(), tmp_path)?;
    apply_wire_args(&mut applied, wire)?;

    if !applied.distributed_stream_summaries {
        return Err(AppError::distributed(
            "Expected stream summaries to be true",
        ));
    }
    let interval = match applied.distributed_stream_interval_ms {
        Some(value) => value.get(),
        None => {
            return Err(AppError::distributed("Expected stream interval to be set"));
        }
    };
    if interval != 150 {
        return Err(AppError::distributed(format!(
            "Unexpected stream interval: {}",
            interval
        )));
    }
    Ok(())
}

#[test]
fn wire_args_deserialize_missing_stream_interval() -> AppResult<()> {
    let args = base_args("http://localhost".to_owned(), "./tmp".to_owned())?;
    let wire = build_wire_args(&args);
    let mut value = serde_json::to_value(&wire)
        .map_err(|err| AppError::distributed(format!("Serialize failed: {}", err)))?;
    match value.as_object_mut() {
        Some(map) => {
            map.remove("stream_interval_ms");
        }
        None => {
            return Err(AppError::distributed(
                "Expected wire args to serialize to object",
            ));
        }
    }
    let decoded: WireArgs = serde_json::from_value(value)
        .map_err(|err| AppError::distributed(format!("Deserialize failed: {}", err)))?;
    if decoded.stream_interval_ms.is_some() {
        return Err(AppError::distributed(
            "Expected stream_interval_ms to default to None",
        ));
    }
    Ok(())
}
