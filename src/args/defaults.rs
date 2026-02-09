use std::path::PathBuf;

pub(crate) const DEFAULT_USER_AGENT: &str = concat!(
    "strest-loadtest/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Lythaeon/strest)"
);

pub(crate) fn default_charts_path() -> String {
    default_base_dir()
        .join("charts")
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn default_tmp_path() -> String {
    default_base_dir()
        .join("tmp")
        .to_string_lossy()
        .into_owned()
}

fn default_base_dir() -> PathBuf {
    if let Some(home) = user_home_dir() {
        return home.join(".strest");
    }

    PathBuf::from(".strest")
}

fn user_home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(value) = std::env::var_os("USERPROFILE") {
            return Some(PathBuf::from(value));
        }
        let drive = std::env::var_os("HOMEDRIVE");
        let path = std::env::var_os("HOMEPATH");
        match (drive, path) {
            (Some(drive), Some(path)) => {
                let mut full = PathBuf::from(drive);
                full.push(path);
                return Some(full);
            }
            _ => {}
        }
    }

    if let Some(value) = std::env::var_os("HOME") {
        return Some(PathBuf::from(value));
    }

    None
}
