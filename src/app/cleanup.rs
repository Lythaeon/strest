use std::path::Path;

pub(crate) async fn cleanup_tmp(log_path: &Path, tmp_dir: &Path) -> Result<(), std::io::Error> {
    if let Err(err) = tokio::fs::remove_file(log_path).await
        && err.kind() != std::io::ErrorKind::NotFound
    {
        return Err(err);
    }

    let mut entries = match tokio::fs::read_dir(tmp_dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if entries.next_entry().await?.is_none()
        && let Err(err) = tokio::fs::remove_dir(tmp_dir).await
        && err.kind() != std::io::ErrorKind::NotFound
    {
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cleanup_tmp_removes_file_and_empty_dir() -> Result<(), String> {
        let dir = tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
        let tmp_path = dir.path().join("tmp");
        std::fs::create_dir_all(&tmp_path)
            .map_err(|err| format!("create_dir_all failed: {}", err))?;
        let log_path = tmp_path.join("metrics.log");
        std::fs::write(&log_path, "1,2,200\n").map_err(|err| format!("write failed: {}", err))?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("runtime build failed: {}", err))?;
        runtime
            .block_on(cleanup_tmp(&log_path, &tmp_path))
            .map_err(|err| format!("cleanup_tmp failed: {}", err))?;

        if log_path.exists() {
            return Err("Expected log file removed".to_owned());
        }
        if tmp_path.exists() {
            return Err("Expected tmp dir removed".to_owned());
        }

        Ok(())
    }

    #[test]
    fn cleanup_tmp_preserves_dir_with_other_files() -> Result<(), String> {
        let dir = tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
        let tmp_path = dir.path().join("tmp");
        std::fs::create_dir_all(&tmp_path)
            .map_err(|err| format!("create_dir_all failed: {}", err))?;
        let log_path = tmp_path.join("metrics.log");
        std::fs::write(&log_path, "1,2,200\n").map_err(|err| format!("write failed: {}", err))?;
        let other_path = tmp_path.join("keep.log");
        std::fs::write(&other_path, "x").map_err(|err| format!("write failed: {}", err))?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("runtime build failed: {}", err))?;
        runtime
            .block_on(cleanup_tmp(&log_path, &tmp_path))
            .map_err(|err| format!("cleanup_tmp failed: {}", err))?;

        if log_path.exists() {
            return Err("Expected log file removed".to_owned());
        }
        if !tmp_path.exists() {
            return Err("Expected tmp dir to remain".to_owned());
        }
        if !other_path.exists() {
            return Err("Expected other file to remain".to_owned());
        }

        Ok(())
    }
}
