//! Configuration loading and application.
pub(crate) mod apply;
mod loader;
mod parse;
pub mod types;

#[cfg(test)]
mod tests;

pub use apply::apply_config;
pub use loader::load_config;

#[cfg(test)]
pub(crate) use loader::load_config_file;
pub(crate) use parse::parse_duration_value;
