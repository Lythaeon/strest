use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult, ConfigError};

use super::types::ConfigFile;

/// Loads a configuration file from the provided path or default locations.
///
/// # Errors
///
/// Returns an error when the config file cannot be read or parsed.
pub fn load_config(path: Option<&str>) -> AppResult<Option<ConfigFile>> {
    if let Some(path) = path {
        let path = PathBuf::from(path);
        return Ok(Some(load_config_file(&path)?));
    }

    let toml_path = PathBuf::from("strest.toml");
    if toml_path.exists() {
        return Ok(Some(load_config_file(&toml_path)?));
    }

    let json_path = PathBuf::from("strest.json");
    if json_path.exists() {
        return Ok(Some(load_config_file(&json_path)?));
    }

    Ok(None)
}

pub(crate) fn load_config_file(path: &Path) -> AppResult<ConfigFile> {
    let content = std::fs::read_to_string(path).map_err(|err| {
        AppError::config(ConfigError::ReadConfig {
            path: path.to_path_buf(),
            source: err,
        })
    })?;
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("toml") => toml::from_str(&content).map_err(|err| {
            AppError::config(ConfigError::ParseToml {
                path: path.to_path_buf(),
                source: err,
            })
        }),
        Some("json") => serde_json::from_str(&content).map_err(|err| {
            AppError::config(ConfigError::ParseJson {
                path: path.to_path_buf(),
                source: err,
            })
        }),
        Some(ext) => Err(AppError::config(ConfigError::UnsupportedExtension {
            ext: ext.to_owned(),
        })),
        None => Err(AppError::config(ConfigError::MissingExtension)),
    }
}
