use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::args::CleanupArgs;
use crate::charts;
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
    let cutoff = if let Some(age) = args.older_than {
        Some(
            SystemTime::now()
                .checked_sub(age)
                .ok_or_else(|| AppError::validation(ValidationError::InvalidOlderThanDuration))?,
        )
    } else {
        None
    };

    let mut candidates = collect_tmp_candidates(args, cutoff).await?;
    if args.with_charts {
        candidates.extend(collect_chart_candidates(args, cutoff).await?);
    }

    if candidates.is_empty() {
        println!("No entries matched cleanup criteria.");
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

async fn collect_tmp_candidates(
    args: &CleanupArgs,
    cutoff: Option<SystemTime>,
) -> AppResult<Vec<PathBuf>> {
    collect_candidates(
        Path::new(&args.tmp_path),
        cutoff,
        is_cleanup_candidate,
        |dir| AppError::service(ServiceError::ReadTmpDir { source: dir }),
    )
    .await
}

async fn collect_chart_candidates(
    args: &CleanupArgs,
    cutoff: Option<SystemTime>,
) -> AppResult<Vec<PathBuf>> {
    collect_candidates(
        Path::new(&args.charts_path),
        cutoff,
        is_chart_cleanup_candidate,
        |dir| AppError::service(ServiceError::ReadTmpDir { source: dir }),
    )
    .await
}

async fn collect_candidates<F, E>(
    root: &Path,
    cutoff: Option<SystemTime>,
    is_candidate: F,
    map_read_dir_error: E,
) -> AppResult<Vec<PathBuf>>
where
    F: Fn(&str, &std::fs::Metadata) -> bool,
    E: Fn(std::io::Error) -> AppError,
{
    let mut entries = match tokio::fs::read_dir(root).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(err) => {
            return Err(map_read_dir_error(err));
        }
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
        if !is_candidate(&file_name, &metadata) {
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
    Ok(candidates)
}

fn is_chart_cleanup_candidate(file_name: &str, metadata: &std::fs::Metadata) -> bool {
    metadata.is_dir() && charts::is_chart_run_dir_name(file_name)
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

    #[test]
    fn chart_cleanup_candidate_matches_run_dirs_only() -> AppResult<()> {
        let dir =
            tempdir().map_err(|err| AppError::service(ServiceError::TempDir { source: err }))?;
        let charts_root = dir.path().join("charts");
        std::fs::create_dir_all(&charts_root)
            .map_err(|err| AppError::service(ServiceError::CreateDirAll { source: err }))?;

        let valid = charts_root.join("run-2026-02-12_15-30-07_example.com-443");
        std::fs::create_dir_all(&valid)
            .map_err(|err| AppError::service(ServiceError::CreateDirAll { source: err }))?;
        let invalid = charts_root.join("api.example.com");
        std::fs::create_dir_all(&invalid)
            .map_err(|err| AppError::service(ServiceError::CreateDirAll { source: err }))?;

        let valid_meta = std::fs::metadata(&valid).map_err(|err| {
            AppError::service(ServiceError::ReadMetadata {
                path: valid.clone(),
                source: err,
            })
        })?;
        let invalid_meta = std::fs::metadata(&invalid).map_err(|err| {
            AppError::service(ServiceError::ReadMetadata {
                path: invalid.clone(),
                source: err,
            })
        })?;

        if !is_chart_cleanup_candidate("run-2026-02-12_15-30-07_example.com-443", &valid_meta) {
            return Err(AppError::validation(
                "expected chart run directory candidate",
            ));
        }
        if is_chart_cleanup_candidate("api.example.com", &invalid_meta) {
            return Err(AppError::validation(
                "unexpected non-run chart directory candidate",
            ));
        }

        Ok(())
    }
}
