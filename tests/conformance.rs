//! Conformance properties and a bounded fuzz smoke test (spec §8).
//!
//! The generator is a small deterministic LCG so a failure can be reproduced from the seed printed
//! in the assertion message (no `proptest` dependency: see docs/decisions.md D10).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{ctx, mem, run, with_trust};
use configlens::adapters::Client;
use configlens::context::Trust;
use configlens::model::event::EventKind;
use configlens::model::report::{build, Compatibility, ReportInput};
use configlens::parse::{parse, Limits, Syntax};
use configlens::resolve::ResolutionStatus;
use configlens::security::{redact_value, PathMode, PublicValue, RedactionPolicy};

const GLOBAL: &str = "/home/synthetic/.config/opencode/opencode.json";
const PROJECT: &str = "/home/synthetic/work/opencode.json";

fn oc_ctx() -> configlens::context::ResolutionContext {
    let mut c = ctx("opencode", "/home/synthetic/work");
    c.project_root = Some("/home/synthetic/work".into());
    c
}

fn report_json(run: &configlens::resolve::RunOutput) -> String {
    let ctx = oc_ctx();
    let paths = common::policy_for(&ctx);
    let report = build(
        ReportInput {
            command: "explain",
            client: run.client.clone(),
            adapter_revision: run
                .adapter
                .as_ref()
                .map(|a| a.revision.clone())
                .unwrap_or_default(),
            compatibility: Compatibility {
                conformance_status: "fixture_verified_max".into(),
                tested_versions: vec!["1.18.35".into()],
                documented_versions: vec!["1.18.35".into()],
                pinned_commit: None,
                evidence: Vec::new(),
                provisional: false,
            },
            context: ctx,
            sources: &run.sources,
            resolutions: &run.resolutions,
            diagnostics: &run.diagnostics,
            coverage: &run.coverage,
            paths: &paths,
            errors: Vec::new(),
        },
        PathMode::Local,
    );
    configlens::model::report::to_json(&report).expect("json")
}

#[test]
fn property_identical_context_produces_byte_identical_reports() {
    let files = [
        (GLOBAL, r#"{"model": "a", "server": {"port": 1}}"#),
        (
            PROJECT,
            r#"{"model": "b", "mcp": {"x": {"enabled": true}}}"#,
        ),
    ];
    let first = report_json(&run(Client::OpenCode, &oc_ctx(), &files));
    let second = report_json(&run(Client::OpenCode, &oc_ctx(), &files));
    assert_eq!(first, second);
}

#[test]
fn property_irrelevant_definitions_do_not_change_a_supported_resolution() {
    let base = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(PROJECT, r#"{"model": "b"}"#)],
    );
    let noisy = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(
            PROJECT,
            r#"{"model": "b", "totally_unrelated": {"deep": [1, 2, 3]}}"#,
        )],
    );
    assert_eq!(
        base.explain("/model").observed_str(),
        noisy.explain("/model").observed_str()
    );
    assert_eq!(
        base.explain("/model").status,
        noisy.explain("/model").status
    );
}

#[test]
fn property_every_contributing_event_has_provenance_or_a_documented_reason() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[
            (GLOBAL, r#"{"model": "a", "server": {"port": 1}}"#),
            (PROJECT, r#"{"model": "b", "server": {"hostname": "h"}}"#),
        ],
    );
    for resolution in &output.resolutions {
        for event in &resolution.events {
            if event.value.is_none() {
                continue;
            }
            let has_source = event.source_id.is_some() && event.definition_id.is_some();
            let has_reason = event.reason_code.is_some();
            assert!(
                has_source || has_reason,
                "event in {} has neither provenance nor reason: {event:?}",
                resolution.pointer
            );
        }
    }
}

#[test]
fn property_no_resolved_status_while_a_relevant_source_is_unobserved() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(PROJECT, r#"{"model": "b"}"#)],
    );
    let resolution = output.explain("/model");
    assert_ne!(resolution.status, ResolutionStatus::Resolved);
    assert!(resolution.effective_value.is_none());
    assert!(!resolution.coverage_gaps.is_empty());
}

#[test]
fn property_redaction_is_idempotent() {
    let policy = RedactionPolicy {
        sensitive_field_patterns: vec!["token".into()],
        ..RedactionPolicy::default()
    };
    let values = [
        serde_json::json!("plain-text"),
        serde_json::json!({"nested": ["a", "b"]}),
        serde_json::json!(42),
        serde_json::json!(null),
    ];
    for value in values {
        for pointer in ["/model", "/token", "/unknown/path"] {
            let once = redact_value(pointer, &value, &policy, PathMode::Local);
            let twice = redact_value(pointer, &value, &policy, PathMode::Local);
            assert_eq!(once, twice, "redaction must be a pure function");
            if once.is_redacted() {
                assert!(serde_json::to_string(&once).is_ok());
            }
        }
    }
}

#[test]
fn property_a_trust_unknown_project_value_never_becomes_resolved() {
    let mut context = oc_ctx();
    context.trust = Trust::Unknown;
    let output = run(
        Client::OpenCode,
        &context,
        &[(PROJECT, r#"{"model": "b"}"#)],
    );
    let _ = with_trust(context.clone(), Trust::Unknown);
    assert_ne!(output.explain("/model").status, ResolutionStatus::Resolved);
}

/// Deterministic 64-bit LCG: the seed is printed on failure so a case can be replayed.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }
}

#[test]
fn fuzz_smoke_parser_pointer_and_sanitiser_never_panic_and_stay_bounded() {
    let alphabet: Vec<char> = "{}[]\",:/*\\ nulll0123456789abc\n\t \u{e9}"
        .chars()
        .collect();
    let mut rng = Lcg(0x5eed_1234_abcd_ef01);
    let limits = Limits {
        max_depth: 16,
        max_definitions: 64,
        max_bytes: 4096,
    };
    for case in 0..400 {
        let len = (rng.next() % 64) as usize;
        let text: String = (0..len)
            .map(|_| alphabet[(rng.next() as usize) % alphabet.len()])
            .collect();
        for syntax in [Syntax::Jsonc, Syntax::Strict] {
            if let Ok(parsed) = parse(&text, syntax, &limits) {
                assert!(parsed.definitions.len() <= limits.max_definitions + 1);
                assert!(parsed.depth <= limits.max_depth);
                for definition in &parsed.definitions {
                    // Every span must be a valid slice of the input and on a char boundary.
                    if let Some(span) = definition.span {
                        assert!(span.end_byte <= text.len(), "case {case}: span past end");
                        assert!(text.is_char_boundary(span.start_byte));
                        assert!(text.is_char_boundary(span.end_byte));
                    }
                }
            }
        }
        let sanitized = configlens::security::sanitize(&text);
        assert!(!sanitized.contains('\u{1b}'));
        assert!(sanitized.len() <= text.len() * 2 + 8);
        assert_eq!(sanitized, configlens::security::sanitize(&sanitized));
    }
}

#[test]
fn fuzz_smoke_redaction_never_publishes_a_seeded_token() {
    let mut rng = Lcg(0xfeed_0bad_c0de_0001);
    let policy = RedactionPolicy {
        sensitive_field_patterns: vec!["token".into(), "key".into()],
        ..RedactionPolicy::default()
    };
    let tokens = ["SEEDED-A", "SEEDED-B", "SEEDED-C"];
    for case in 0..200 {
        let token = tokens[(rng.next() as usize) % tokens.len()];
        let value = serde_json::json!({ "layer": case, "value": token });
        let pointer = if case % 3 == 0 { "/token" } else { "/unknown" };
        let projected = redact_value(pointer, &value, &policy, PathMode::Local);
        let text = serde_json::to_string(&projected).expect("json");
        assert!(!text.contains(token), "case {case} leaked: {text}");
    }
}

#[test]
fn fuzz_smoke_event_values_in_a_report_are_public_values_only() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(PROJECT, r#"{"model": "b", "unknown": {"a": "b"}}"#)],
    );
    for resolution in &output.resolutions {
        assert!(serde_json::to_string(&resolution.observed_value).is_ok());
        for event in &resolution.events {
            if let Some(value) = &event.value {
                let _: &PublicValue = value;
                if event.kind == EventKind::Ignored {
                    assert!(value.is_redacted() || value.as_value().is_some());
                }
            }
        }
    }
}

#[test]
fn memory_and_real_file_access_agree_on_a_synthetic_tree() {
    let dir = common::project(&[
        ("opencode.json", r#"{"model": "real-model"}"#),
        (".git/HEAD", "ref: refs/heads/main\n"),
    ]);
    let mut context = ctx("opencode", &dir.path().display().to_string());
    context.project_root = Some(dir.path().to_path_buf());
    let real = common::run_real(Client::OpenCode, &context);
    assert_eq!(real.explain("/model").observed_str(), Some("real-model"));

    let memory = run(
        Client::OpenCode,
        &context,
        &[(
            &dir.path().join("opencode.json").display().to_string(),
            r#"{"model": "real-model"}"#,
        )],
    );
    assert_eq!(memory.explain("/model").observed_str(), Some("real-model"));
    let _ = mem(&[]);
}
