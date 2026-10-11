//! ConfigLens library root.
//!
//! Design rule (spec §6): raw configuration values live behind a non-serializable sensitive type;
//! only the redaction projection produces report values, and renderers only ever see those.

#![forbid(unsafe_code)]
// Panic-style helpers are fine in tests (assertions are the point); production code stays on
// `Result` and the `unwrap_used`/`expect_used`/`panic` lints remain active outside `cfg(test)`.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod adapters;
pub mod cli;
pub mod commands;
pub mod completions;
pub mod context;
pub mod error;
pub mod filesystem;
pub mod model;
pub mod parse;
pub mod pattern;
pub mod render;
pub mod resolve;
pub mod security;

/// Crate version reported as `tool_version` in reports.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Report schema version emitted by this build.
pub const REPORT_SCHEMA_VERSION: &str = "1.0";
