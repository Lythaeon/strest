use std::path::PathBuf;
use std::process::ExitStatus;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Cannot combine --install-service and --uninstall-service.")]
    InstallUninstallConflict,
    #[cfg(not(target_os = "linux"))]
    #[error("Service install/uninstall is only supported on Linux.")]
    UnsupportedPlatform,
    #[error("Cannot install service with both controller and agent roles.")]
    ControllerAgentConflict,
    #[error("Service install requires --controller-listen or --agent-join.")]
    InstallRequiresRole,
    #[error("Service name cannot be empty.")]
    ServiceNameEmpty,
    #[error("Failed to resolve working directory: {source}")]
    WorkingDirResolve {
        #[source]
        source: std::io::Error,
    },
    #[error("Working directory is not valid UTF-8.")]
    WorkingDirNotUtf8,
    #[error("Failed to write {path}: {source}")]
    WriteUnitFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to remove {path}: {source}")]
    RemoveUnitFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to resolve executable path: {source}")]
    ExecutablePathResolve {
        #[source]
        source: std::io::Error,
    },
    #[error("Executable path is not valid UTF-8.")]
    ExecutablePathNotUtf8,
    #[error("Failed to run systemctl {args:?}: {source}")]
    SystemctlRun {
        args: Vec<String>,
        #[source]
        source: std::io::Error,
    },
    #[error("systemctl {args:?} failed with status {status}")]
    SystemctlFailed {
        args: Vec<String>,
        status: ExitStatus,
    },
    #[error("Failed to read tmp directory: {source}")]
    ReadTmpDir {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to read tmp entry: {source}")]
    ReadTmpEntry {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to read metadata for {path}: {source}")]
    ReadMetadata {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to read modified time for {path}: {source}")]
    ReadModifiedTime {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[cfg(test)]
    #[error("tempdir failed: {source}")]
    TempDir {
        #[source]
        source: std::io::Error,
    },
    #[cfg(test)]
    #[error("create_dir_all failed: {source}")]
    CreateDirAll {
        #[source]
        source: std::io::Error,
    },
    #[cfg(test)]
    #[error("write failed: {source}")]
    WriteFile {
        #[source]
        source: std::io::Error,
    },
    #[cfg(test)]
    #[error("runtime build failed: {source}")]
    RuntimeBuild {
        #[source]
        source: std::io::Error,
    },
    #[cfg(test)]
    #[error("cleanup_tmp failed: {source}")]
    CleanupTmp {
        #[source]
        source: std::io::Error,
    },
    #[cfg(test)]
    #[error("Expected log file removed")]
    ExpectedLogFileRemoved,
    #[cfg(test)]
    #[error("Expected tmp dir removed")]
    ExpectedTmpDirRemoved,
    #[cfg(test)]
    #[error("Expected tmp dir to remain")]
    ExpectedTmpDirRemain,
    #[cfg(test)]
    #[error("Expected other file to remain")]
    ExpectedOtherFileRemain,
    #[cfg(test)]
    #[error("Test expectation failed: {message}")]
    TestExpectation { message: &'static str },
    #[cfg(test)]
    #[error("Test expectation failed: {message}: {value}")]
    TestExpectationValue {
        message: &'static str,
        value: String,
    },
}
