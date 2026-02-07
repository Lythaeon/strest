use crate::args::TesterArgs;

#[cfg(target_os = "linux")]
/// Handles install/uninstall requests for system services.
///
/// # Errors
///
/// Returns an error if the service operation fails or the arguments are invalid.
pub(crate) fn handle_service_action(args: &TesterArgs) -> Result<(), String> {
    if args.install_service && args.uninstall_service {
        return Err("Cannot combine --install-service and --uninstall-service.".to_owned());
    }
    if args.install_service {
        return install_service(args);
    }
    if args.uninstall_service {
        return uninstall_service(args);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
/// Handles install/uninstall requests for system services.
///
/// # Errors
///
/// Returns an error when service actions are requested on non-Linux targets.
pub(crate) fn handle_service_action(args: &TesterArgs) -> Result<(), String> {
    if args.install_service || args.uninstall_service {
        return Err("Service install/uninstall is only supported on Linux.".to_owned());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_service(args: &TesterArgs) -> Result<(), String> {
    let service_name = resolve_service_name(args, true)?;
    let unit_path = format!("/etc/systemd/system/{}.service", service_name);
    let exec_args = build_exec_args();
    let exec_start = build_exec_start(&exec_args)?;
    let working_dir = std::env::current_dir()
        .map_err(|err| format!("Failed to resolve working directory: {}", err))?;
    let working_dir = working_dir
        .to_str()
        .ok_or_else(|| "Working directory is not valid UTF-8.".to_owned())?;

    let description = if args.controller_listen.is_some() {
        "strest controller"
    } else {
        "strest agent"
    };

    let unit_contents = format!(
        "[Unit]\nDescription={}\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nWorkingDirectory={}\nExecStart={}\nRestart=on-failure\nRestartSec=2\nKillSignal=SIGINT\nTimeoutStopSec=30\n\n[Install]\nWantedBy=multi-user.target\n",
        description, working_dir, exec_start,
    );

    std::fs::write(&unit_path, unit_contents)
        .map_err(|err| format!("Failed to write {}: {}", unit_path, err))?;

    run_systemctl(&["daemon-reload"])?;
    run_systemctl(&["enable", "--now", &format!("{}.service", service_name)])?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn uninstall_service(args: &TesterArgs) -> Result<(), String> {
    let service_name = resolve_service_name(args, false)?;
    let unit_path = format!("/etc/systemd/system/{}.service", service_name);

    if let Err(err) = run_systemctl(&["disable", "--now", &format!("{}.service", service_name)]) {
        eprintln!("Failed to disable service: {}", err);
    }
    if std::fs::metadata(&unit_path).is_ok() {
        std::fs::remove_file(&unit_path)
            .map_err(|err| format!("Failed to remove {}: {}", unit_path, err))?;
    }
    run_systemctl(&["daemon-reload"])?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn resolve_service_name(args: &TesterArgs, require_role: bool) -> Result<String, String> {
    if args.controller_listen.is_some() && args.agent_join.is_some() {
        return Err("Cannot install service with both controller and agent roles.".to_owned());
    }

    let default_name = if args.controller_listen.is_some() {
        Some("strest-controller")
    } else if args.agent_join.is_some() {
        Some("strest-agent")
    } else {
        None
    };

    if require_role && default_name.is_none() {
        return Err("Service install requires --controller-listen or --agent-join.".to_owned());
    }

    let name = args
        .service_name
        .as_deref()
        .or(default_name)
        .unwrap_or("strest")
        .trim();
    let name = name.trim_end_matches(".service");
    if name.is_empty() {
        return Err("Service name cannot be empty.".to_owned());
    }

    Ok(name.to_owned())
}

#[cfg(target_os = "linux")]
fn build_exec_args() -> Vec<String> {
    let mut filtered = Vec::new();
    let mut skip_next = false;
    for arg in std::env::args().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--install-service" || arg == "--uninstall-service" {
            continue;
        }
        if arg == "--service-name" {
            skip_next = true;
            continue;
        }
        if arg.starts_with("--service-name=") {
            continue;
        }
        filtered.push(arg);
    }
    filtered
}

#[cfg(target_os = "linux")]
fn build_exec_start(args: &[String]) -> Result<String, String> {
    let exe = std::env::current_exe()
        .map_err(|err| format!("Failed to resolve executable path: {}", err))?;
    let exe = exe
        .to_str()
        .ok_or_else(|| "Executable path is not valid UTF-8.".to_owned())?;
    let mut parts = Vec::new();
    parts.push(escape_systemd_arg(exe));
    for arg in args {
        parts.push(escape_systemd_arg(arg));
    }
    Ok(parts.join(" "))
}

#[cfg(target_os = "linux")]
fn escape_systemd_arg(arg: &str) -> String {
    if arg
        .chars()
        .all(|ch| !ch.is_whitespace() && ch != '"' && ch != '\\')
    {
        return arg.to_owned();
    }
    let mut escaped = String::with_capacity(arg.len().saturating_add(2));
    escaped.push('"');
    for ch in arg.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            _ => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(target_os = "linux")]
fn run_systemctl(args: &[&str]) -> Result<(), String> {
    let status = std::process::Command::new("systemctl")
        .args(args)
        .status()
        .map_err(|err| format!("Failed to run systemctl {:?}: {}", args, err))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "systemctl {:?} failed with status {}",
            args, status
        ))
    }
}
