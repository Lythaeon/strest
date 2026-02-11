//! Core library for the `strest` CLI.
//!
//! This crate provides the internal building blocks used by the binary: CLI
//! argument types, configuration parsing, request execution, metrics
//! aggregation, and output sinks. The primary user-facing interface is the
//! `strest` command-line application; library APIs may evolve as the CLI
//! grows.
pub mod args;
pub mod config;
pub mod error;
pub mod http;
pub mod metrics;
pub mod sinks;
pub mod ui;

#[cfg(feature = "fuzzing")]
pub mod fuzzing;
