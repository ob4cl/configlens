//! Command implementations: snapshot and diff (spec §4).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::error::CliError;
use crate::resolve::RunOutput;
use crate::security::display_pointer;

/// Maximum serialized report size, shared by snapshot writers and report readers.
pub const MAX_REPORT_BYTES: usize = 64 * 1024 * 1024;

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
/// Refuses symlinks and special files. Publication without overwrite uses a hard link, never a
/// replacing rename. Filesystems without hard-link support fail closed. Windows inherits ACLs;
/// no owner-only Windows ACL guarantee is made. The caller must trust the destination directory:
/// std's path-based operations do not pin ancestors or prevent hostile directory-entry swaps.
pub fn write_snapshot(
    path: &Path,
    content: &str,
    overwrite: bool,
) -> Result<SnapshotOutcome, CliError> {
    if content.len() > MAX_REPORT_BYTES {
        return Err(snapshot_error("resource_limit_exceeded"));
    }
    validate_snapshot_destination(path, overwrite)?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temporary = directory.join(format!(".configlens-{stamp}-{}.tmp", std::process::id()));
    #[cfg(test)]
    snapshot_test_hook(SnapshotStage::BeforeCreate, &temporary);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| snapshot_error("snapshot_write_failed"))?;
    // Only an entry we successfully created is ours to clean up. Close the file before cleanup
    // (including on Windows); the guard also handles later validation/publication failures.
    let mut cleanup = SnapshotTemporary {
        path: temporary,
        active: true,
    };
    #[cfg(test)]
    snapshot_test_hook(SnapshotStage::AfterCreate, &cleanup.path);
    use std::io::Write;
    let written = file.write_all(content.as_bytes());
    drop(file);
    written.map_err(|_| snapshot_error("snapshot_write_failed"))?;
    #[cfg(test)]
    snapshot_test_hook(SnapshotStage::BeforePublish, &cleanup.path);
    if overwrite {
        validate_snapshot_destination(path, true)?;
        std::fs::rename(&cleanup.path, path)
            .map_err(|_| snapshot_error("snapshot_rename_failed"))?;
    } else {
        // Hard-link creation refuses *any* existing destination atomically, including a symlink.
        std::fs::hard_link(&cleanup.path, path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                CliError::usage("the output file already exists; pass --overwrite to replace it")
            } else {
                snapshot_error("snapshot_rename_failed")
            }
        })?;
        std::fs::remove_file(&cleanup.path)
            .map_err(|_| snapshot_error("snapshot_cleanup_failed"))?;
    }
    cleanup.active = false;
    Ok(SnapshotOutcome {
        // Output destinations are not run-context paths, so cannot be safely relativized here.
        location: "<snapshot output>".to_string(),
        bytes: content.len() as u64,
        overwritten: overwrite,
        permissions: crate::model::report::snapshot_acl_support().to_string(),
    })
}

fn snapshot_error(code: &'static str) -> CliError {
    // Never pass raw paths or OS error text across the public error boundary.
    CliError::file_problem(code, "<snapshot output>")
}

fn validate_snapshot_destination(path: &Path, overwrite: bool) -> Result<(), CliError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(CliError::usage(
                    "the output path is a symlink; ConfigLens will not write through it",
                ));
            }
            if metadata.is_dir() {
                return Err(CliError::usage("the output path is a directory"));
            }
            if !metadata.is_file() {
                return Err(CliError::usage("the output path is not a regular file"));
            }
            if !overwrite {
                return Err(CliError::usage(
                    "the output file already exists; pass --overwrite to replace it",
                ));
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(snapshot_error("snapshot_write_failed")),
    }
}

struct SnapshotTemporary {
    path: std::path::PathBuf,
    active: bool,
}

impl Drop for SnapshotTemporary {
    fn drop(&mut self) {
        // Best effort on failure: a removed/unwritable parent can make cleanup impossible.
        if self.active {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum SnapshotStage {
    BeforeCreate,
    AfterCreate,
    BeforePublish,
}

#[cfg(test)]
type SnapshotHook = Box<dyn FnMut(SnapshotStage, &Path)>;

#[cfg(test)]
std::thread_local! {
    static SNAPSHOT_HOOK: std::cell::RefCell<Option<SnapshotHook>> = const {
        std::cell::RefCell::new(None)
    };
}

#[cfg(test)]
fn snapshot_test_hook(stage: SnapshotStage, path: &Path) {
    SNAPSHOT_HOOK.with(|hook| {
        if let Some(callback) = hook.borrow_mut().as_mut() {
            callback(stage, path);
        }
    });
}

#[cfg(test)]
#[path = "../tests/snapshot_write/unit.rs"]
mod snapshot_write_tests;

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
    /// A source (file/layer) was added, removed or changed status/applicability.
    Source,
    RedactedUnknown,
}

impl DiffKind {
    pub fn name(self) -> &'static str {
        match self {
            DiffKind::Value => "value",
            DiffKind::Provenance => "provenance",
            DiffKind::Coverage => "coverage",
            DiffKind::Status => "status",
            DiffKind::Source => "source",
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
    if text.len() > MAX_REPORT_BYTES {
        return Err(CliError::file_problem(
            "resource_limit_exceeded",
            "<report input>",
        ));
    }
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

    // --- Report-level provenance: the source inventory and the run coverage gaps ----
    // Spec §4: `diff` distinguishes value, provenance and coverage changes. Changes to *which*
    // sources exist (a file added/removed, a status or applicability change) are provenance changes
    // at the report level, and so are changes to the run-wide coverage gaps.
    let left_sources = source_entries(before);
    let right_sources = source_entries(after);
    let mut source_ids: Vec<String> = left_sources
        .keys()
        .chain(right_sources.keys())
        .cloned()
        .collect();
    source_ids.sort();
    source_ids.dedup();
    for id in source_ids {
        match (left_sources.get(&id), right_sources.get(&id)) {
            (Some(left), None) => entries.push(DiffEntry {
                pointer: format!("source:{id}"),
                kind: DiffKind::Source,
                before: source_summary(left),
                after: "<removed>".to_string(),
            }),
            (None, Some(right)) => entries.push(DiffEntry {
                pointer: format!("source:{id}"),
                kind: DiffKind::Source,
                before: "<added>".to_string(),
                after: source_summary(right),
            }),
            (Some(left), Some(right)) => {
                let before_summary = source_summary(left);
                let after_summary = source_summary(right);
                if before_summary != after_summary {
                    entries.push(DiffEntry {
                        pointer: format!("source:{id}"),
                        kind: DiffKind::Source,
                        before: before_summary,
                        after: after_summary,
                    });
                }
            }
            (None, None) => {}
        }
    }
    let before_run_coverage = run_coverage(before);
    let after_run_coverage = run_coverage(after);
    if before_run_coverage != after_run_coverage {
        entries.push(DiffEntry {
            pointer: "run".to_string(),
            kind: DiffKind::Coverage,
            before: before_run_coverage,
            after: after_run_coverage,
        });
    }

    Ok(DiffResult {
        client: client_before.to_string(),
        schema_version,
        entries,
        uncompared,
    })
}

/// Index a report's `sources` array by id (falling back to its display location).
fn source_entries(report: &Value) -> BTreeMap<String, &Value> {
    let mut map = BTreeMap::new();
    if let Some(items) = report.get("sources").and_then(Value::as_array) {
        for item in items {
            let key = match item.get("id") {
                Some(id) => id.to_string(),
                None => item
                    .get("location")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string(),
            };
            map.insert(key, item);
        }
    }
    map
}

/// A one-line, source-level summary used for source diffs.
fn source_summary(source: &Value) -> String {
    format!(
        "{} {} status={} applicability={}",
        source.get("layer").and_then(Value::as_str).unwrap_or("?"),
        source
            .get("location")
            .and_then(Value::as_str)
            .unwrap_or("?"),
        source.get("status").and_then(Value::as_str).unwrap_or("?"),
        source
            .get("applicability")
            .and_then(Value::as_str)
            .unwrap_or("?")
    )
}

/// The run-wide coverage gaps as a stable, sorted string.
fn run_coverage(report: &Value) -> String {
    let mut items: Vec<String> = report
        .get("coverage")
        .and_then(Value::as_array)
        .map(|gaps| {
            gaps.iter()
                .map(|gap| {
                    format!(
                        "{}:{}:{}",
                        gap.get("kind").and_then(Value::as_str).unwrap_or("?"),
                        gap.get("subject").and_then(Value::as_str).unwrap_or("?"),
                        gap.get("reason").and_then(Value::as_str).unwrap_or("?")
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    items.sort();
    items.dedup();
    items.join(";")
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
                display_pointer(&entry.pointer),
                entry.kind.name(),
                entry.before,
                entry.after
            ));
        }
    }
    for entry in &result.uncompared {
        out.push_str(&format!(
            "  {}  unknown_redacted  {} -> {}  (not compared: protected value)\n",
            display_pointer(&entry.pointer),
            entry.before,
            entry.after
        ));
    }
    out
}

/// JSON rendering for `diff`.
pub fn render_diff_json(result: &DiffResult) -> Result<String, CliError> {
    let project = |entry: &DiffEntry| {
        serde_json::json!({
            "pointer": display_pointer(&entry.pointer),
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
    fn source_and_run_coverage_changes_are_reported() {
        let left = json!({
            "schema_version": "1.0",
            "client": "opencode",
            "resolutions": [],
            "sources": [{
                "id": 1, "layer": "oc.global", "location": "$HOME/x.json",
                "status": "observed", "applicability": "applicable"
            }],
            "coverage": [{
                "kind": "expected_unobservable", "subject": "oc.remote.wellknown",
                "reason": "remote_source_not_fetched"
            }],
        });
        let right = json!({
            "schema_version": "1.0",
            "client": "opencode",
            "resolutions": [],
            "sources": [
                {
                    "id": 1, "layer": "oc.global", "location": "$HOME/x.json",
                    "status": "malformed", "applicability": "applicable"
                },
                {
                    "id": 2, "layer": "oc.project", "location": "$PROJECT/opencode.json",
                    "status": "observed", "applicability": "applicable"
                }
            ],
            "coverage": [{
                "kind": "managed_state_unknown", "subject": "oc.managed_file",
                "reason": "managed_file_absent_does_not_prove_policy_absent"
            }],
        });
        let result = diff_reports(&left, &right).expect("diff");
        let kinds: Vec<&str> = result.entries.iter().map(|e| e.kind.name()).collect();
        assert!(kinds.contains(&"source"), "source changes: {kinds:?}");
        assert!(kinds.contains(&"coverage"), "coverage changes: {kinds:?}");
        let text = render_diff(&result);
        assert!(text.contains("status=malformed"), "got: {text}");
        assert!(text.contains("<added>"));
    }

    #[test]
    fn snapshot_create_new_and_overwrite_are_explicit() {
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
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_refuses_a_symlink_destination() {
        let dir = tempfile::tempdir().expect("temp");
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
