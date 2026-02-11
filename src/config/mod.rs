//! Configuration loading and application.
pub(crate) mod apply;
mod loader;
mod parse;
pub mod types;

#[cfg(any(test, feature = "fuzzing"))]
mod test_support;
#[cfg(test)]
mod tests;

pub use apply::apply_config;
pub use loader::load_config;
pub(crate) use parse::parse_duration_value;

#[cfg(any(test, feature = "fuzzing"))]
pub(crate) use test_support::load_config_file;
