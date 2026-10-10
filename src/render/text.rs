//! Text rendering: the explanation a person reads.
//!
//! Shape follows the specification's synthetic example (§4):
//!
//! ```text
//! /model = "model-b"
//! Observed winner: $PROJECT/.claude/settings.local.json:2:12
//!   "model-a"  $PROJECT/.claude/settings.json:2:12  replaced_by_later_layer
//!   "model-b"  $PROJECT/.claude/settings.local.json:2:12  observed_winner
//! Rule: claude.scalar.layer-override
//! Coverage: partial — cc.managed.server — remote_source_not_fetched (expected_unobservable)
//! Support: fixture_verified; confidence: evidence_class:fixture_verified
//! Note: ...
//! Conclusion: model-b wins among the observed applicable sources; effective value withheld.
//! ```

use crate::context::ResolutionContext;
use crate::model::event::EventKind;
use crate::resolve::{Resolution, ResolutionStatus, RunOutput};

/// Pretty value for text output: JSON scalars render as JSON, redacted values show their reason.
fn shown(value: &crate::security::PublicValue) -> String {
    match value.as_value() {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "<unavailable>".to_string()),
        None => match value.reason_str() {
            "" => "<redacted>".to_string(),
            reason => format!("<redacted:{reason}>"),
        },
    }
}

/// Render one resolution as the user-facing explanation.
pub fn render_explain(
    run: &RunOutput,
    resolution: &Resolution,
    _ctx: &ResolutionContext,
) -> String {
    let mut out = String::new();
    match &resolution.effective_value {
        Some(value) => out.push_str(&format!("{} = {}\n", resolution.pointer, shown(value))),
        None => out.push_str(&format!(
            "{} = {} (observed)\n",
            resolution.pointer,
            shown(&resolution.observed_value)
        )),
    }
    if let Some(location) = resolution.selected_location() {
        out.push_str(&format!("Observed winner: {location}\n"));
    }
    for event in &resolution.events {
        let value = event
            .value
            .as_ref()
            .map(shown)
            .unwrap_or_else(|| "-".to_string());
        let location = event.location.as_deref().unwrap_or("-");
        let reason = event.reason_code.as_deref().unwrap_or("");
        match event.kind {
            EventKind::Selected | EventKind::Replaced | EventKind::Contributed => {
                out.push_str(&format!("  {value}  {location}  {reason}\n"));
            }
            EventKind::Ignored => {
                out.push_str(&format!("  ignored: {location}  {reason}\n"));
            }
            EventKind::Constrained => {
                out.push_str(&format!("  constrained by: {location}  {reason}\n"));
            }
            EventKind::Transformed => {
                out.push_str(&format!("  transformed: {location}  {reason}\n"));
            }
            EventKind::Unresolved => {
                out.push_str(&format!("  unresolved: {location}  {reason}\n"));
            }
        }
    }
    if let Some(rule_id) = &resolution.rule_id {
        out.push_str(&format!("Rule: {rule_id}\n"));
    }
    if resolution.coverage_gaps.is_empty() {
        out.push_str("Coverage: complete\n");
    } else {
        out.push_str(&format!(
            "Coverage: {} — {}\n",
            resolution.status.name(),
            resolution.coverage_gaps.join("; ")
        ));
    }
    out.push_str(&format!(
        "Support: {}; confidence: {}\n",
        resolution.support.name(),
        resolution.confidence.name()
    ));
    for note in &resolution.notes {
        out.push_str(&format!("Note: {note}\n"));
    }
    if let Some(caveat) = resolution.authorization_note() {
        out.push_str(&format!("Caveat: {caveat}\n"));
    }
    let observed = shown(&resolution.observed_value);
    match resolution.status {
        ResolutionStatus::Resolved => {
            out.push_str("Conclusion: this is the effective value within the analysed context.\n")
        }
        ResolutionStatus::Partial => out.push_str(&format!(
            "Conclusion: {observed} wins among the observed applicable sources; effective value withheld.\n"
        )),
        ResolutionStatus::Unsupported => out.push_str(
            "Conclusion: unsupported key, client or version — inventory only, no merge result is claimed.\n",
        ),
        ResolutionStatus::Invalid => out.push_str(
            "Conclusion: a source at or above the winner is malformed; the observed value is not the runtime value.\n",
        ),
        ResolutionStatus::Absent => {
            out.push_str("Conclusion: no observed source defines this key.\n")
        }
    }
    // A run-level diagnostic (a malformed or blocked high-priority file, a duplicate key) must be
    // visible in the human-readable explanation too, not only in `doctor` and the JSON envelope.
    // Otherwise a malformed higher layer could be silently reported as "no source defines this key"
    // (spec §9 example 6). Diagnostics never carry raw content (spec §7).
    if !run.diagnostics.is_empty() {
        out.push_str(&format!("Diagnostics ({}):\n", run.diagnostics.len()));
        for diagnostic in &run.diagnostics {
            let location = diagnostic.location.as_deref().unwrap_or("-");
            let position = match (diagnostic.line, diagnostic.column) {
                (Some(line), Some(column)) => format!("line {line}, column {column}"),
                _ => "location unavailable".to_string(),
            };
            out.push_str(&format!(
                "  [{}] {}  {}  {}\n",
                diagnostic.severity.name(),
                diagnostic.code,
                location,
                position
            ));
        }
    }
    out
}

/// Render the inventory: sources with their status and applicability.
pub fn render_scan(run: &RunOutput) -> String {
    let mut out = String::new();
    let revision = run
        .adapter
        .as_ref()
        .map(|a| a.revision.clone())
        .unwrap_or_else(|| "unknown".to_string());
    out.push_str(&format!(
        "ConfigLens scan — client {} (adapter revision {})\n",
        run.client, revision
    ));
    out.push_str(&format!("Sources ({}):\n", run.sources.len()));
    for source in &run.sources {
        out.push_str(&format!(
            "  [{}] {}  {}  {}  {}\n",
            source.rank,
            source.layer_id,
            source.display,
            source.status.code(),
            match &source.applicability {
                crate::adapters::Applicability::Applicable => "applicable".to_string(),
                crate::adapters::Applicability::Skipped(r)
                | crate::adapters::Applicability::Conditional(r) =>
                    format!("not unconditional ({r})"),
            }
        ));
    }
    if run.coverage.is_empty() {
        out.push_str("Coverage gaps: none reported\n");
    } else {
        out.push_str(&format!("Coverage gaps ({}):\n", run.coverage.len()));
        for gap in &run.coverage {
            out.push_str(&format!("  - {}\n", gap.describe()));
        }
    }
    out
}

/// Render doctor findings. Never claims a configuration is "secure".
pub fn render_doctor(run: &RunOutput) -> String {
    let mut out = String::new();
    let errors = run
        .diagnostics
        .iter()
        .filter(|d| d.severity == crate::resolve::Severity::Error)
        .count();
    let warnings = run
        .diagnostics
        .iter()
        .filter(|d| d.severity == crate::resolve::Severity::Warning)
        .count();
    out.push_str(&format!(
        "ConfigLens doctor — client {}: {} error(s), {} warning(s), {} coverage gap(s)\n",
        run.client,
        errors,
        warnings,
        run.coverage.len()
    ));
    for diagnostic in &run.diagnostics {
        out.push_str(&format!(
            "  [{}] {}  {}  {}\n",
            diagnostic.severity.name(),
            diagnostic.code,
            diagnostic.location.as_deref().unwrap_or("-"),
            match (diagnostic.line, diagnostic.column) {
                (Some(line), Some(column)) => format!("line {line}, column {column}"),
                _ => "location unavailable".to_string(),
            }
        ));
    }
    for gap in &run.coverage {
        out.push_str(&format!("  [coverage] {}\n", gap.describe()));
    }
    if run.diagnostics.is_empty() && run.coverage.is_empty() {
        out.push_str("  no findings\n");
    }
    out.push_str(
        "Note: ConfigLens reports what it could read. It never certifies a configuration as safe.\n",
    );
    out
}

/// Render the `clients` support matrix.
pub fn render_clients() -> String {
    let mut out = String::new();
    out.push_str("ConfigLens clients\n\nImplemented adapters:\n");
    for client in crate::adapters::Client::all() {
        match crate::adapters::Registry::load(*client) {
            Ok(registry) => {
                let metadata = registry.metadata();
                out.push_str(&format!(
                    "  {}  revision {}  tested {}  documented {}  conformance {}\n",
                    client.id(),
                    metadata.revision,
                    metadata.tested_versions.join(","),
                    metadata.documented_versions.join(","),
                    metadata.conformance_status
                ));
                for layer in registry.coverage_gaps() {
                    out.push_str(&format!(
                        "    always-incomplete layer: {} ({})\n",
                        layer.0, layer.1
                    ));
                }
            }
            Err(error) => out.push_str(&format!("  {}  UNAVAILABLE ({})\n", client.id(), error)),
        }
    }
    out.push_str("\nExplicitly unsupported clients (no compatibility claim):\n");
    for (id, why) in crate::adapters::Client::unsupported() {
        out.push_str(&format!("  {}  {}\n", id, why));
    }
    out
}

/// One-line summary, used when a single line is enough.
pub fn render_summary(resolution: &Resolution) -> String {
    format!(
        "{}: {} — {} (rule {}; support {})",
        resolution.pointer,
        resolution.status.name(),
        shown(&resolution.observed_value),
        resolution.rule_id.as_deref().unwrap_or("none"),
        resolution.support.name()
    )
}

/// Punctuation used between the status and its explanation.
pub const COVERAGE_SEPARATOR: &str = " — ";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::SupportLevel;
    use crate::model::definition::ValueKind;
    use crate::model::event::{Confidence, Event};
    use crate::security::PublicValue;
    use serde_json::json;

    fn resolution() -> Resolution {
        let mut r = Resolution::new("/model");
        r.status = ResolutionStatus::Partial;
        r.support = SupportLevel::FixtureVerified;
        r.confidence = Confidence::FixtureVerified;
        r.rule_id = Some("claude.scalar.layer-override".to_string());
        r.evidence = vec!["S3".to_string()];
        r.observed_value = PublicValue::revealed(json!("model-b"));
        r.events = vec![
            Event::new(EventKind::Replaced)
                .with_definition(
                    crate::model::event::DefinitionId(1),
                    crate::model::source::SourceId(1),
                )
                .with_location("$PROJECT/.claude/settings.json:2:12".to_string())
                .with_value(PublicValue::revealed(json!("model-a")))
                .with_reason("replaced_by_later_layer"),
            Event::new(EventKind::Selected)
                .with_definition(
                    crate::model::event::DefinitionId(2),
                    crate::model::source::SourceId(2),
                )
                .with_location("$PROJECT/.claude/settings.local.json:2:12".to_string())
                .with_value(PublicValue::revealed(json!("model-b")))
                .with_reason("observed_winner"),
        ];
        r.coverage_gaps = vec![
            "cc.managed.server — remote_source_not_fetched (expected_unobservable)".to_string(),
        ];
        r
    }

    fn ctx() -> ResolutionContext {
        ResolutionContext::new("claude", "/p", "/home/dev", "/home/dev/.config")
    }

    #[test]
    fn explain_lists_every_definition_with_its_location_and_fate() {
        let run = RunOutput::default();
        let text = render_explain(&run, &resolution(), &ctx());
        assert!(text.contains("/model = \"model-b\""), "got: {text}");
        assert!(text.contains("Observed winner: $PROJECT/.claude/settings.local.json:2:12"));
        assert!(text
            .contains("\"model-a\"  $PROJECT/.claude/settings.json:2:12  replaced_by_later_layer"));
        assert!(text.contains("Rule: claude.scalar.layer-override"));
        assert!(text.contains("Coverage: partial"));
        assert!(text.contains("cc.managed.server — remote_source_not_fetched"));
        assert!(text.contains("effective value withheld"));
    }

    #[test]
    fn explain_marks_redacted_values_with_the_reason() {
        let mut r = resolution();
        r.observed_value = PublicValue::redacted("sensitive_field", ValueKind::String);
        let text = render_explain(&RunOutput::default(), &r, &ctx());
        assert!(text.contains("<redacted:sensitive_field>"));
    }

    #[test]
    fn a_resolved_key_states_that_the_value_is_effective() {
        let mut r = resolution();
        r.status = ResolutionStatus::Resolved;
        r.effective_value = Some(PublicValue::revealed(json!("model-b")));
        r.coverage_gaps.clear();
        let text = render_explain(&RunOutput::default(), &r, &ctx());
        assert!(text.contains("Coverage: complete"));
        assert!(text.contains("Conclusion: this is the effective value"));
    }

    #[test]
    fn an_unknown_key_is_reported_as_inventory_only_not_as_a_winner() {
        let mut r = Resolution::new("/provider/x/name");
        r.status = ResolutionStatus::Unsupported;
        r.support = SupportLevel::InventoryOnly;
        r.observed_value = PublicValue::redacted("inventory_only", ValueKind::String);
        let text = render_explain(&RunOutput::default(), &r, &ctx());
        assert!(text.contains("no merge result is claimed"), "got: {text}");
        assert!(!text.contains("wins among"));
    }

    #[test]
    fn summary_is_a_single_line() {
        let line = render_summary(&resolution());
        assert!(!line.contains('\n'));
        assert!(line.contains("/model"));
        assert!(line.contains("partial"));
    }
}
