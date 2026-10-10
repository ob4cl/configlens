//! Parsers: span-preserving, duplicate-key aware, no regex comment stripping.

pub mod json;
pub mod jsonc;

pub use json::{parse, DuplicateKey, Limits, ParseError, ParsedSource, RawDefinition, Syntax};
