use std::path::{Path, PathBuf};

use super::types::ConfigFile;

/// Loads a configuration file from the provided path or default locations.
///
/// # Errors
///
/// Returns an error when the config file cannot be read or parsed.
pub fn load_config(path: Option<&str>) -> Result<Option<ConfigFile>, String> {
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

pub(crate) fn load_config_file(path: &Path) -> Result<ConfigFile, String> {
    let content =
        std::fs::read_to_string(path).map_err(|err| format!("Failed to read config: {}", err))?;
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("toml") => {
            toml::from_str(&content).map_err(|err| format!("Failed to parse TOML config: {}", err))
        }
        Some("json") => serde_json::from_str(&content)
            .map_err(|err| format!("Failed to parse JSON config: {}", err)),
        Some(ext) => Err(format!(
            "Unsupported config extension '{}'. Use .toml or .json.",
            ext
        )),
        None => Err("Config file must have .toml or .json extension.".to_owned()),
    }
}
