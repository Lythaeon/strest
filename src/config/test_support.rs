use std::path::Path;

use crate::error::AppResult;

use super::loader;
use super::types::ConfigFile;

pub(crate) fn load_config_file(path: &Path) -> AppResult<ConfigFile> {
    loader::load_config_file(path)
}
