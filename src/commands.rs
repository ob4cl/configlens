//! Command implementations: snapshot and diff (spec §4).

use std::path::Path;

use serde_json::Value;

use crate::error::CliError;
use crate::resolve::RunOutput;

/// Result of writing a snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotOutcome {
    pub location: String,
    pub bytes: u64,
    pub overwritten: bool,
    pub permissions: String,
}

/// Write a report to an explicit destination: create-new by default, atomic, owner-only on Unix.
///
/// Refuses a symlink destination and requires `--overwrite` to replace an existing file.
pub fn write_snapshot(
    path: &Path,
    content: &str,
    overwrite: bool,
) -> Result<SnapshotOutcome, CliError> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(CliError::usage(
                "the output path is a symlink; ConfigLens will not write through it",
            ));
        }
        if metadata.is_dir() {
            return Err(CliError::usage("the output path is a directory"));
        }
        if !overwrite {
            return Err(CliError::usage(
                "the output file already exists; pass --overwrite to replace it",
            ));
        }
    }
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temporary = directory.join(format!(".configlens-{stamp}-{}.tmp", std::process::id()));
    if let Err(error) = std::fs::write(&temporary, content) {
        return Err(CliError::file_problem(
            "snapshot_write_failed",
            &format!("{} ({error})", temporary.display()),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        if let Err(error) = std::fs::set_permissions(&temporary, permissions) {
            return Err(CliError::file_problem(
                "snapshot_permissions_failed",
                &format!("{} ({error})", temporary.display()),
            ));
        }
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        return Err(CliError::file_problem(
            "snapshot_rename_failed",
            &format!("{} ({error})", path.display()),
        ));
    }
    Ok(SnapshotOutcome {
        location: path.display().to_string(),
        bytes: content.len() as u64,
        overwritten: overwrite,
        permissions: crate::model::report::snapshot_acl_support().to_string(),
    })
}

/// One difference between two reports.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffEntry {
    pub pointer: String,
    pub kind: DiffKind,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Value,
    Provenance,
    Coverage,
    Status,
    RedactedUnknown,
}

impl DiffKind {
    pub fn name(self) -> &'static str {
        match self {
            DiffKind::Value => "value",
            DiffKind::Provenance => "provenance",
            DiffKind::Coverage => "coverage",
            DiffKind::Status => "status",
            DiffKind::RedactedUnknown => "unknown_redacted",
        }
    }
}

/// The result of comparing two reports.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffResult {
    pub client: String,
    pub schema_version: String,
    /// Proven differences (value, provenance, status, coverage).
    pub entries: Vec<DiffEntry>,
    /// Protected values that could not be compared at all: two masks prove nothing (spec §5).
    pub uncompared: Vec<DiffEntry>,
}

impl DiffResult {
    /// True when nothing proven changed. Uncompared protected values are reported separately.
    pub fn is_identical(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Validate a snapshot document: it must be a ConfigLens report envelope of a compatible version.
pub fn parse_report(text: &str) -> Result<Value, CliError> {
    let value: Value = serde_json::from_str(text)
        .map_err(|_| CliError::incompatible_input("the file is not valid JSON"))?;
    let object = value
        .as_object()
        .ok_or_else(|| CliError::incompatible_input("the file is not a report object"))?;
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| CliError::incompatible_input("the report has no schema_version"))?;
    if schema != crate::REPORT_SCHEMA_VERSION {
        return Err(CliError::incompatible_input(
            "the report schema version is not supported by this build",
        ));
    }
    if !object.contains_key("resolutions") || !object.contains_key("client") {
        return Err(CliError::incompatible_input(
            "the report is missing resolutions or client",
        ));
    }
    Ok(value)
}

fn resolutions(report: &Value) -> Vec<&Value> {
    report
        .get("resolutions")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn pointer_of(resolution: &Value) -> String {
    resolution
        .get("pointer")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn public_value(resolution: &Value) -> &Value {
    resolution
        .get("effective_value")
        .filter(|v| !v.is_null())
        .or_else(|| resolution.get("observed_value"))
        .unwrap_or(&Value::Null)
}

fn is_redacted(value: &Value) -> bool {
    value
        .get("redacted")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn describe(value: &Value) -> String {
    if is_redacted(value) {
        let reason = value
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return format!("<redacted:{reason}>");
    }
    value
        .get("value")
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "<unavailable>".to_string()))
        .unwrap_or_else(|| "<absent>".to_string())
}

fn provenance(resolution: &Value) -> String {
    resolution
        .get("events")
        .and_then(Value::as_array)
        .map(|events| {
            events
                .iter()
                .map(|event| {
                    format!(
                        "{}@{}",
                        event.get("kind").and_then(Value::as_str).unwrap_or("?"),
                        event.get("location").and_then(Value::as_str).unwrap_or("-")
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default()
}

fn gaps(resolution: &Value) -> String {
    resolution
        .get("coverage_gaps")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(";")
        })
        .unwrap_or_default()
}

/// Compare two reports from the same client and schema version.
///
/// Distinguishes value, provenance, status and coverage changes. Two redacted values compare as
/// `unknown_redacted`: ConfigLens never hashes or guesses a protected value (spec §5).
pub fn diff_reports(before: &Value, after: &Value) -> Result<DiffResult, CliError> {
    let client_before = before
        .get("client")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let client_after = after
        .get("client")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if client_before != client_after {
        return Err(CliError::incompatible_input(
            "the two reports are from different clients",
        ));
    }
    let schema_version = before
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let mut pointers: Vec<String> = resolutions(before)
        .iter()
        .chain(resolutions(after).iter())
        .map(|r| pointer_of(r))
        .collect();
    pointers.sort();
    pointers.dedup();

    let mut entries = Vec::new();
    let mut uncompared = Vec::new();
    for pointer in pointers {
        let left = resolutions(before)
            .into_iter()
            .find(|r| pointer_of(r) == pointer);
        let right = resolutions(after)
            .into_iter()
            .find(|r| pointer_of(r) == pointer);
        let left_value = left.map(public_value);
        let right_value = right.map(public_value);
        let redacted = left_value.map(is_redacted).unwrap_or(false)
            || right_value.map(is_redacted).unwrap_or(false);
        if redacted {
            // Never assume two masks are equal, and never hash a protected value.
            uncompared.push(DiffEntry {
                pointer,
                kind: DiffKind::RedactedUnknown,
                before: left_value
                    .map(describe)
                    .unwrap_or_else(|| "<absent>".to_string()),
                after: right_value
                    .map(describe)
                    .unwrap_or_else(|| "<absent>".to_string()),
            });
            continue;
        }
        match (left, right) {
            (None, Some(_other)) | (Some(_other), None) => entries.push(DiffEntry {
                pointer,
                kind: DiffKind::Value,
                before: left_value
                    .map(describe)
                    .unwrap_or_else(|| "<absent>".to_string()),
                after: right_value
                    .map(describe)
                    .unwrap_or_else(|| "<absent>".to_string()),
            }),
            (Some(left), Some(right)) => {
                let before_status = left.get("status").and_then(Value::as_str).unwrap_or("");
                let after_status = right.get("status").and_then(Value::as_str).unwrap_or("");
                if before_status != after_status {
                    entries.push(DiffEntry {
                        pointer: pointer.clone(),
                        kind: DiffKind::Status,
                        before: before_status.to_string(),
                        after: after_status.to_string(),
                    });
                }
                let before_shown = describe(left_value.unwrap_or(&Value::Null));
                let after_shown = describe(right_value.unwrap_or(&Value::Null));
                if before_shown != after_shown {
                    entries.push(DiffEntry {
                        pointer: pointer.clone(),
                        kind: DiffKind::Value,
                        before: before_shown,
                        after: after_shown,
                    });
                }
                let before_provenance = provenance(left);
                let after_provenance = provenance(right);
                if before_provenance != after_provenance {
                    entries.push(DiffEntry {
                        pointer: pointer.clone(),
                        kind: DiffKind::Provenance,
                        before: before_provenance,
                        after: after_provenance,
                    });
                }
                let before_gaps = gaps(left);
                let after_gaps = gaps(right);
                if before_gaps != after_gaps {
                    entries.push(DiffEntry {
                        pointer,
                        kind: DiffKind::Coverage,
                        before: before_gaps,
                        after: after_gaps,
                    });
                }
            }
            (None, None) => {}
        }
    }

    Ok(DiffResult {
        client: client_before.to_string(),
        schema_version,
        entries,
        uncompared,
    })
}

/// Text rendering for `diff`.
pub fn render_diff(result: &DiffResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "ConfigLens diff — client {} (schema {})\n",
        result.client, result.schema_version
    ));
    if result.is_identical() {
        out.push_str("no differences in values, provenance, status or coverage\n");
    } else {
        for entry in &result.entries {
            out.push_str(&format!(
                "  {}  {}  {} -> {}\n",
                entry.pointer,
                entry.kind.name(),
                entry.before,
                entry.after
            ));
        }
    }
    for entry in &result.uncompared {
        out.push_str(&format!(
            "  {}  unknown_redacted  {} -> {}  (not compared: protected value)\n",
            entry.pointer, entry.before, entry.after
        ));
    }
    out
}

/// JSON rendering for `diff`.
pub fn render_diff_json(result: &DiffResult) -> Result<String, CliError> {
    let project = |entry: &DiffEntry| {
        serde_json::json!({
            "pointer": entry.pointer,
            "kind": entry.kind.name(),
            "before": entry.before,
            "after": entry.after,
        })
    };
    let entries: Vec<Value> = result.entries.iter().map(project).collect();
    let uncompared: Vec<Value> = result.uncompared.iter().map(project).collect();
    let document = serde_json::json!({
        "schema_version": crate::REPORT_SCHEMA_VERSION,
        "tool_version": crate::TOOL_VERSION,
        "command": "diff",
        "client": result.client,
        "identical": result.is_identical(),
        "entries": entries,
        "uncompared_redacted": uncompared,
        "errors": [],
    });
    let mut text = serde_json::to_string_pretty(&document)
        .map_err(|_| CliError::usage("the diff could not be serialized"))?;
    text.push('\n');
    Ok(text)
}

/// Findings summary used by `doctor`.
pub fn doctor_summary(run: &RunOutput) -> String {
    let errors = run
        .diagnostics
        .iter()
        .filter(|d| d.severity == crate::resolve::Severity::Error)
        .count();
    format!(
        "{} error(s), {} warning(s), {} coverage gap(s)",
        errors,
        run.diagnostics.len() - errors,
        run.coverage.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn report(resolutions: Value) -> Value {
        json!({
            "schema_version": "1.0",
            "client": "opencode",
            "resolutions": resolutions,
        })
    }

    fn resolution(pointer: &str, value: Value) -> Value {
        json!({
            "pointer": pointer,
            "status": "partial",
            "observed_value": { "value": value, "kind": "string" },
            "events": [{"kind": "selected", "location": "$HOME/x.json:1:1"}],
            "coverage_gaps": ["oc.opencode_dirs — not_a_settings_document (expected_unobservable)"],
        })
    }

    #[test]
    fn identical_reports_have_no_entries() {
        let left = report(json!([resolution("/model", json!("a"))]));
        let right = left.clone();
        let result = diff_reports(&left, &right).expect("diff");
        assert!(result.is_identical());
        assert!(render_diff(&result).contains("no differences"));
    }

    #[test]
    fn different_values_are_reported_as_value_changes() {
        let left = report(json!([resolution("/model", json!("a"))]));
        let right = report(json!([resolution("/model", json!("b"))]));
        let result = diff_reports(&left, &right).expect("diff");
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].kind, DiffKind::Value);
        assert_eq!(result.entries[0].before, "\"a\"");
        assert_eq!(result.entries[0].after, "\"b\"");
    }

    #[test]
    fn redacted_values_compare_as_unknown_and_never_as_equal() {
        let left = report(json!([{
            "pointer": "/env/TOKEN",
            "status": "partial",
            "observed_value": {"redacted": true, "reason": "environment_value", "kind": "string"},
            "events": [],
            "coverage_gaps": []
        }]));
        let right = left.clone();
        let result = diff_reports(&left, &right).expect("diff");
        assert!(result.is_identical(), "no proven change");
        assert_eq!(result.uncompared.len(), 1);
        assert_eq!(result.uncompared[0].kind, DiffKind::RedactedUnknown);
        assert!(render_diff(&result).contains("unknown_redacted"));
        assert!(render_diff(&result).contains("no differences"));
    }

    #[test]
    fn provenance_changes_are_distinguished_from_value_changes() {
        let left = report(json!([resolution("/model", json!("a"))]));
        let mut moved = resolution("/model", json!("a"));
        moved["events"] = json!([{"kind": "selected", "location": "$HOME/y.json:1:1"}]);
        let right = report(json!([moved]));
        let result = diff_reports(&left, &right).expect("diff");
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].kind, DiffKind::Provenance);
    }

    #[test]
    fn coverage_changes_are_reported_separately() {
        let left = report(json!([resolution("/model", json!("a"))]));
        let mut changed = resolution("/model", json!("a"));
        changed["coverage_gaps"] = json!([]);
        let right = report(json!([changed]));
        let result = diff_reports(&left, &right).expect("diff");
        assert_eq!(result.entries[0].kind, DiffKind::Coverage);
    }

    #[test]
    fn incompatible_documents_are_rejected() {
        assert_eq!(
            parse_report("{\"schema_version\":\"2.0\"}")
                .unwrap_err()
                .exit_code(),
            2
        );
        assert_eq!(parse_report("not json").unwrap_err().exit_code(), 2);
        let other_client = json!({"schema_version":"1.0","client":"claude","resolutions":[]});
        let opencode = report(json!([]));
        assert!(diff_reports(&opencode, &other_client).is_err());
    }

    #[test]
    fn snapshot_refuses_a_symlink_and_requires_overwrite() {
        let dir = tempfile::tempdir().expect("temp");
        let target = dir.path().join("out.json");
        let first = write_snapshot(&target, "{}", false).expect("create");
        assert!(!first.overwritten);
        assert_eq!(
            write_snapshot(&target, "{}", false)
                .unwrap_err()
                .exit_code(),
            2
        );
        assert!(write_snapshot(&target, "{}", true).is_ok());

        let real = dir.path().join("real.json");
        std::fs::write(&real, "{}").expect("write");
        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        assert_eq!(
            write_snapshot(&link, "{}", true).unwrap_err().exit_code(),
            2
        );
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temp");
        let target = dir.path().join("out.json");
        write_snapshot(&target, "{}", false).expect("create");
        let mode = std::fs::metadata(&target)
            .expect("meta")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}
