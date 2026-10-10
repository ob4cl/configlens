//! JSON rendering: one valid report envelope on stdout, no ANSI, deterministic key order.

use crate::model::report::{to_json, Report};

/// Serialize a report as the single JSON envelope.
pub fn render(report: &Report) -> String {
    match to_json(report) {
        Ok(text) => text,
        Err(_) => {
            // A serialization failure must still produce one valid JSON document.
            "{\"schema_version\":\"1.0\",\"errors\":[{\"code\":\"serialize_failed\",\"message\":\"report could not be serialized\"}]}\n".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::report::{Compatibility, ContextReport, RedactionReport};
    use crate::resolve::{Diagnostic, Severity};

    fn minimal() -> Report {
        Report {
            schema_version: "1.0".into(),
            tool_version: "0.1.0".into(),
            command: "clients".into(),
            client: "claude".into(),
            adapter_revision: "0.1.0".into(),
            compatibility: Compatibility {
                conformance_status: "fixture_verified_max".into(),
                tested_versions: vec!["2.1.296".into()],
                documented_versions: vec!["2.1.296".into()],
                pinned_commit: None,
                evidence: Vec::new(),
                provisional: false,
            },
            context: ContextReport {
                os: "linux".into(),
                cwd: "$CWD".into(),
                project_root: None,
                client_version: "unknown".into(),
                trust: "unknown".into(),
                paths: "local".into(),
                simulated: false,
                strict: false,
                environment_keys: Vec::new(),
            },
            sources: Vec::new(),
            resolutions: Vec::new(),
            diagnostics: vec![Diagnostic {
                severity: Severity::Warning,
                code: "managed_state_unknown".into(),
                location: None,
                line: None,
                column: None,
                message: "managed sources are not observable here".into(),
            }],
            coverage: Vec::new(),
            redaction: RedactionReport {
                mode: "local".into(),
                rules_applied: vec!["sensitive_field_names".into()],
                snapshot_acl: "owner_only_0600".into(),
                values_redacted: 0,
            },
            errors: Vec::new(),
        }
    }

    #[test]
    fn output_is_one_valid_json_document_with_a_trailing_newline() {
        let text = render(&minimal());
        assert!(text.ends_with('\n'));
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(parsed["schema_version"], "1.0");
    }

    #[test]
    fn output_contains_no_ansi_escape_sequences() {
        let text = render(&minimal());
        assert!(!text.contains('\u{1b}'));
    }

    #[test]
    fn key_order_is_sorted_and_stable() {
        let a = render(&minimal());
        let b = render(&minimal());
        assert_eq!(a, b);
        let parsed: serde_json::Value = serde_json::from_str(&a).expect("json");
        let keys: Vec<&str> = parsed
            .as_object()
            .map(|m| m.keys().map(String::as_str).collect())
            .unwrap_or_default();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted, "top-level keys must be sorted");
    }
}
