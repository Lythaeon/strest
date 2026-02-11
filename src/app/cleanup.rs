use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::args::CleanupArgs;
use crate::error::{AppError, AppResult, ServiceError, ValidationError};

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

pub(crate) async fn run_cleanup(args: &CleanupArgs) -> AppResult<()> {
    let tmp_dir = Path::new(&args.tmp_path);
    let mut entries = match tokio::fs::read_dir(tmp_dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            println!("No tmp directory found at {}", tmp_dir.display());
            return Ok(());
        }
        Err(err) => {
            return Err(AppError::service(ServiceError::ReadTmpDir { source: err }));
        }
    };

    let cutoff = if let Some(age) = args.older_than {
        Some(
            SystemTime::now()
                .checked_sub(age)
                .ok_or_else(|| AppError::validation(ValidationError::InvalidOlderThanDuration))?,
        )
    } else {
        None
    };

    let mut candidates: Vec<PathBuf> = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|err| AppError::service(ServiceError::ReadTmpEntry { source: err }))?
    {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        let metadata = entry.metadata().await.map_err(|err| {
            AppError::service(ServiceError::ReadMetadata {
                path: path.clone(),
                source: err,
            })
        })?;
        if !is_cleanup_candidate(&file_name, &metadata) {
            continue;
        }
        if let Some(cutoff) = cutoff {
            let modified = metadata.modified().map_err(|err| {
                AppError::service(ServiceError::ReadModifiedTime {
                    path: path.clone(),
                    source: err,
                })
            })?;
            if modified > cutoff {
                continue;
            }
        }
        candidates.push(path);
    }

    if candidates.is_empty() {
        println!("No tmp entries matched cleanup criteria.");
        return Ok(());
    }

    let dry_run = args.dry_run || !args.force;
    if dry_run && !args.dry_run {
        println!("Dry run only (use --force to delete).");
    }

    let mut removed = 0usize;
    for path in &candidates {
        if dry_run {
            println!("Would remove {}", path.display());
            continue;
        }
        if let Err(err) = remove_entry(path).await {
            eprintln!("Failed to remove {}: {}", path.display(), err);
            continue;
        }
        println!("Removed {}", path.display());
        removed = removed.saturating_add(1);
    }

    if !dry_run {
        println!("Removed {} item(s).", removed);
    }

    Ok(())
}

fn is_cleanup_candidate(file_name: &str, metadata: &std::fs::Metadata) -> bool {
    if metadata.is_file() {
        return file_name.starts_with("metrics-") && file_name.ends_with(".log");
    }
    if metadata.is_dir() {
        return file_name.starts_with("run-");
    }
    false
}

async fn remove_entry(path: &Path) -> Result<(), std::io::Error> {
    let metadata = tokio::fs::metadata(path).await?;
    if metadata.is_dir() {
        tokio::fs::remove_dir_all(path).await
    } else {
        tokio::fs::remove_file(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cleanup_tmp_removes_file_and_empty_dir() -> AppResult<()> {
        let dir =
            tempdir().map_err(|err| AppError::service(ServiceError::TempDir { source: err }))?;
        let tmp_path = dir.path().join("tmp");
        std::fs::create_dir_all(&tmp_path)
            .map_err(|err| AppError::service(ServiceError::CreateDirAll { source: err }))?;
        let log_path = tmp_path.join("metrics.log");
        std::fs::write(&log_path, "1,2,200\n")
            .map_err(|err| AppError::service(ServiceError::WriteFile { source: err }))?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| AppError::service(ServiceError::RuntimeBuild { source: err }))?;
        runtime
            .block_on(cleanup_tmp(&log_path, &tmp_path))
            .map_err(|err| AppError::service(ServiceError::CleanupTmp { source: err }))?;

        if log_path.exists() {
            return Err(AppError::service(ServiceError::ExpectedLogFileRemoved));
        }
        if tmp_path.exists() {
            return Err(AppError::service(ServiceError::ExpectedTmpDirRemoved));
        }

        Ok(())
    }

    #[test]
    fn cleanup_tmp_preserves_dir_with_other_files() -> AppResult<()> {
        let dir =
            tempdir().map_err(|err| AppError::service(ServiceError::TempDir { source: err }))?;
        let tmp_path = dir.path().join("tmp");
        std::fs::create_dir_all(&tmp_path)
            .map_err(|err| AppError::service(ServiceError::CreateDirAll { source: err }))?;
        let log_path = tmp_path.join("metrics.log");
        std::fs::write(&log_path, "1,2,200\n")
            .map_err(|err| AppError::service(ServiceError::WriteFile { source: err }))?;
        let other_path = tmp_path.join("keep.log");
        std::fs::write(&other_path, "x")
            .map_err(|err| AppError::service(ServiceError::WriteFile { source: err }))?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| AppError::service(ServiceError::RuntimeBuild { source: err }))?;
        runtime
            .block_on(cleanup_tmp(&log_path, &tmp_path))
            .map_err(|err| AppError::service(ServiceError::CleanupTmp { source: err }))?;

        if log_path.exists() {
            return Err(AppError::service(ServiceError::ExpectedLogFileRemoved));
        }
        if !tmp_path.exists() {
            return Err(AppError::service(ServiceError::ExpectedTmpDirRemain));
        }
        if !other_path.exists() {
            return Err(AppError::service(ServiceError::ExpectedOtherFileRemain));
        }

        Ok(())
    }
}
