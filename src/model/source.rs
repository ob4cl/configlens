//! Sources: where definitions come from, and whether we could observe them.

use std::path::PathBuf;

use serde::Serialize;

use crate::parse::{ParseError, Syntax};

/// Stable, content-free source identity: assigned from discovery order (spec §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SourceId(pub u32);

/// Whether a candidate source could be observed and parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStatus {
    Observed,
    /// Expected by the adapter but not present on this machine. Not an error.
    Missing,
    /// Present but unreadable (permissions, I/O).
    Unreadable,
    /// Present but the wrong kind of filesystem object, or a symlink escaping the root.
    Blocked(String),
    /// Over a resource limit: reported, never silently truncated.
    ResourceLimit(String),
    /// Not valid JSON/JSONC for the adapter's dialect.
    Malformed(ParseError),
    /// Supplied by the user through the context file: user-provided evidence, not a runtime fact.
    Supplied,
    /// A layer that exists upstream but ConfigLens cannot read it (e.g. a plist or a registry key).
    Unobservable(String),
}

impl SourceStatus {
    pub fn is_usable(&self) -> bool {
        matches!(self, SourceStatus::Observed | SourceStatus::Supplied)
    }

    /// Short code used in diagnostics and reports.
    pub fn code(&self) -> &'static str {
        match self {
            SourceStatus::Observed => "observed",
            SourceStatus::Missing => "missing",
            SourceStatus::Unreadable => "unreadable",
            SourceStatus::Blocked(_) => "blocked",
            SourceStatus::ResourceLimit(_) => "resource_limit_exceeded",
            SourceStatus::Malformed(_) => "malformed",
            SourceStatus::Supplied => "supplied",
            SourceStatus::Unobservable(_) => "unobservable",
        }
    }

    pub fn detail(&self) -> Option<String> {
        match self {
            SourceStatus::Blocked(r)
            | SourceStatus::ResourceLimit(r)
            | SourceStatus::Unobservable(r) => Some(r.clone()),
            SourceStatus::Malformed(e) => Some(e.to_string()),
            _ => None,
        }
    }
}

/// One discovered source, ready for reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub id: SourceId,
    /// Registry layer id, e.g. `cc.project_local`.
    pub layer_id: String,
    /// Precedence rank of the layer. Higher wins.
    pub rank: u32,
    pub class: crate::adapters::SourceClass,
    /// Sanitized, aliased display location (`$HOME/...` in local mode).
    pub display: String,
    /// Present only in local mode; share mode keeps the alias-form display string.
    pub path: Option<PathBuf>,
    pub status: SourceStatus,
    pub syntax: Syntax,
    pub applicability: crate::adapters::Applicability,
    pub bytes: u64,
    pub note: Option<String>,
}

impl Source {
    pub fn is_applicable(&self) -> bool {
        self.applicability.is_applicable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ParseError;

    #[test]
    fn usable_statuses_are_observed_and_supplied_only() {
        assert!(SourceStatus::Observed.is_usable());
        assert!(SourceStatus::Supplied.is_usable());
        assert!(!SourceStatus::Missing.is_usable());
        assert!(!SourceStatus::Malformed(ParseError {
            code: "trailing_comma",
            offset: 1,
            line: 1,
            column: 2
        })
        .is_usable());
    }

    #[test]
    fn status_codes_are_stable_strings() {
        assert_eq!(SourceStatus::Missing.code(), "missing");
        assert_eq!(
            SourceStatus::Blocked("symlink_escape".into()).code(),
            "blocked"
        );
        assert_eq!(
            SourceStatus::Blocked("symlink_escape".into())
                .detail()
                .as_deref(),
            Some("symlink_escape")
        );
        assert_eq!(SourceStatus::Observed.detail(), None);
        assert_eq!(
            SourceStatus::Malformed(ParseError {
                code: "unexpected_token",
                offset: 0,
                line: 2,
                column: 5
            })
            .detail()
            .as_deref(),
            Some("unexpected_token at line 2, column 5")
        );
    }

    #[test]
    fn source_ids_are_ordered_so_reports_are_deterministic() {
        let mut ids = vec![SourceId(3), SourceId(1), SourceId(2)];
        ids.sort();
        assert_eq!(ids, vec![SourceId(1), SourceId(2), SourceId(3)]);
    }
}
