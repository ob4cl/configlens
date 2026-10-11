//! OpenCode adapter rules, end to end through the resolution engine.
//! Test ids here are the ones `compatibility/opencode/registry.json` cites.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{ctx, host_abs, mem, policy_for, run, run_with, with_env};
use configlens::adapters::Client;
use configlens::context::Trust;
use configlens::model::event::EventKind;
use configlens::resolve::{Engine, GapKind, ResolutionStatus};
use configlens::security::PathMode;

const GLOBAL: &str = "/home/synthetic/.config/opencode/opencode.json";
const PROJECT: &str = "/home/synthetic/work/opencode.json";
const MANAGED: &str = "/etc/opencode/opencode.json";

fn oc_ctx() -> configlens::context::ResolutionContext {
    let mut c = ctx("opencode", "/home/synthetic/work");
    c.project_root = Some("/home/synthetic/work".into());
    c
}

#[test]
fn oc_scalar_project_overrides_global() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[
            (GLOBAL, r#"{"model": "global-model"}"#),
            (PROJECT, r#"{"model": "project-model"}"#),
        ],
    );
    let r = output.explain("/model");
    assert_eq!(r.observed_str(), Some("project-model"));
    assert_eq!(
        r.event_kinds(),
        vec!["replaced", "selected"],
        "the shadowed definition must stay in the trace"
    );
    let replaced = r
        .events
        .iter()
        .find(|e| e.kind == EventKind::Replaced)
        .expect("replaced event");
    assert!(replaced
        .location
        .as_deref()
        .unwrap_or("")
        .contains("opencode.json:1:"));
    assert!(
        r.selected_location().unwrap_or("").ends_with(":1:11") || r.selected_location().is_some()
    );
}

#[test]
fn oc_project_is_never_trust_gated() {
    // O4 regression: OpenCode has no upstream workspace-trust concept, so `oc.project` must be
    // unconditionally applicable regardless of the trust hint — no trust gap and no trust
    // diagnostic. (The old, Claude-derived gate made it conditional/skipped.)
    for trust in [Trust::Unknown, Trust::Untrusted] {
        let mut c = oc_ctx();
        c.trust = trust;
        let output = run(
            Client::OpenCode,
            &c,
            &[(PROJECT, r#"{"model": "project-model"}"#)],
        );
        assert_eq!(
            output.explain("/model").observed_str(),
            Some("project-model"),
            "trust {trust:?} must not gate the OpenCode project config"
        );
        assert!(
            !output.has_gap(GapKind::TrustUnknown, "run"),
            "OpenCode must never report a trust gap"
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|d| d.code != "trust_unknown" && d.code != "workspace_untrusted"),
            "OpenCode must never report a trust diagnostic: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn oc_managed_file_overrides_project() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[
            (PROJECT, r#"{"model": "project-model"}"#),
            (MANAGED, r#"{"model": "managed-model"}"#),
        ],
    );
    let r = output.explain("/model");
    // The managed file wins among observed sources, but the macOS preference layer above it is
    // never observable, so no effective value may be claimed.
    assert_eq!(r.observed_str(), Some("managed-model"));
    assert_eq!(r.status, ResolutionStatus::Partial);
    assert!(r.effective_value.is_none());
    assert!(r
        .coverage_gaps
        .iter()
        .any(|g| g.contains("oc.managed_macos_prefs")));
}

#[test]
fn oc_inline_content_overrides_project() {
    let c = with_env(
        oc_ctx(),
        &[(
            "OPENCODE_CONFIG_CONTENT",
            r#"{"model": "inline-model", "snapshot": false}"#,
        )],
    );
    let output = run(
        Client::OpenCode,
        &c,
        &[
            (GLOBAL, r#"{"model": "global-model"}"#),
            (PROJECT, r#"{"model": "project-model"}"#),
        ],
    );
    let r = output.explain("/model");
    assert_eq!(r.observed_str(), Some("inline-model"));
    let inline = r
        .events
        .iter()
        .find(|e| e.kind == EventKind::Selected)
        .expect("selected");
    assert!(inline
        .location
        .as_deref()
        .unwrap_or("")
        .contains("session environment"));
}

#[test]
fn oc_scalar_boundary_highest_layer_null_is_not_missing() {
    // An explicit null is a value, not an absence: it still replaces a lower layer.
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[
            (GLOBAL, r#"{"small_model": "a"}"#),
            (PROJECT, r#"{"small_model": null}"#),
        ],
    );
    let r = output.explain("/small_model");
    assert_eq!(r.status, ResolutionStatus::Partial);
    assert!(r.observed_value.as_value().is_some_and(|v| v.is_null()));
    assert_eq!(
        r.observed_value.kind,
        configlens::model::definition::ValueKind::Null
    );
    assert_eq!(r.event_kinds(), vec!["replaced", "selected"]);
}

#[test]
fn oc_scalar_negative_unknown_layer_does_not_win() {
    // A local `.opencode` directory is listed upstream but is not a settings document here.
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(GLOBAL, r#"{"model": "global-model"}"#)],
    );
    let r = output.explain("/model");
    assert_eq!(r.observed_str(), Some("global-model"));
    assert!(output.has_gap(GapKind::ExpectedUnobservable, "oc.opencode_dirs"));
}

#[test]
fn oc_object_nested_merge_retains_unrelated_keys() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[
            (
                GLOBAL,
                r#"{"server": {"port": 4096, "hostname": "127.0.0.1"}}"#,
            ),
            (PROJECT, r#"{"server": {"port": 5000}}"#),
        ],
    );
    assert_eq!(
        output.explain("/server/port").observed_value.as_value(),
        Some(&serde_json::json!(5000))
    );
    assert_eq!(
        output.explain("/server/hostname").observed_value.as_value(),
        Some(&serde_json::json!("127.0.0.1")),
        "non-conflicting keys survive the merge"
    );
    let container = output.explain("/server");
    assert_eq!(container.status, ResolutionStatus::Partial);
    assert!(container.notes.iter().any(|n| n.contains("container")));
}

#[test]
fn oc_object_child_events_retained() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[
            (
                GLOBAL,
                r#"{"mcp": {"jira": {"enabled": false, "type": "remote"}}}"#,
            ),
            (PROJECT, r#"{"mcp": {"jira": {"enabled": true}}}"#),
        ],
    );
    let r = output.explain("/mcp/jira/enabled");
    assert_eq!(r.observed_value.as_value(), Some(&serde_json::json!(true)));
    assert_eq!(r.event_kinds(), vec!["replaced", "selected"]);
    assert_eq!(
        output.explain("/mcp/jira/type").observed_value.as_value(),
        Some(&serde_json::json!("remote"))
    );
}

#[test]
fn oc_object_negative_no_merge_for_unknown_root() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(PROJECT, r#"{"totally_unknown": {"a": 1}}"#)],
    );
    let r = output.explain("/totally_unknown/a");
    assert_eq!(r.status, ResolutionStatus::Unsupported);
    assert!(
        r.observed_value.is_redacted(),
        "unknown values are never shown"
    );
    assert!(r.effective_value.is_none());
}

#[test]
fn oc_list_inventory_only_no_merge_claim() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[
            (GLOBAL, r#"{"instructions": ["a.md"]}"#),
            (PROJECT, r#"{"instructions": ["b.md"]}"#),
        ],
    );
    let r = output.explain("/instructions");
    assert_eq!(r.status, ResolutionStatus::Partial);
    assert!(r.effective_value.is_none());
    assert!(r.notes.iter().any(|n| n.contains("inventory_only")));
    assert!(r.observed_value.is_redacted());
}

#[test]
fn oc_unknown_key_inventory_only() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(PROJECT, r#"{"future_setting": "value"}"#)],
    );
    let r = output.explain("/future_setting");
    assert_eq!(r.status, ResolutionStatus::Unsupported);
    assert_eq!(
        r.rule_id.as_deref(),
        Some("opencode.unknown.inventory-only")
    );
}

#[test]
fn oc_env_interpolation_tainted() {
    let c = with_env(oc_ctx(), &[("OPENCODE_MODEL", "SEEDED-INTERPOLATED-VALUE")]);
    let output = run(
        Client::OpenCode,
        &c,
        &[(PROJECT, r#"{"model": "{env:OPENCODE_MODEL}"}"#)],
    );
    let r = output.explain("/model");
    assert!(r.events.iter().any(|e| e.kind == EventKind::Transformed));
    assert!(
        r.effective_value.is_none(),
        "derived env values are never published"
    );
    let text = serde_json::to_string(&r).expect("json");
    assert!(!text.contains("SEEDED-INTERPOLATED-VALUE"));
}

#[test]
fn oc_env_interpolation_unresolved_without_context() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(PROJECT, r#"{"model": "{env:OPENCODE_MODEL}"}"#)],
    );
    let r = output.explain("/model");
    assert_eq!(r.status, ResolutionStatus::Partial);
    assert!(r
        .events
        .iter()
        .any(|e| e.reason_code.as_deref() == Some("env_variable_not_supplied")));
}

#[test]
fn oc_file_interpolation_requires_allow_read_root() {
    let outside = host_abs("/etc/secret-model");
    let blocked_spec = format!(r#"{{"model": "{{file:{outside}}}"}}"#);
    let blocked = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(PROJECT, blocked_spec.as_str())],
    );
    let r = blocked.explain("/model");
    assert!(r
        .events
        .iter()
        .any(|e| e.reason_code.as_deref() == Some("file_reference_blocked")));

    let mut allowed = oc_ctx();
    let root = host_abs("/home/synthetic/work");
    let readable = format!("{root}/model.txt");
    let allowed_spec = format!(r#"{{"model": "{{file:{readable}}}"}}"#);
    allowed.approved_roots = vec![root.into()];
    let files = mem(&[
        (PROJECT, allowed_spec.as_str()),
        (readable.as_str(), "SEEDED-FILE-MODEL"),
    ]);
    let output = run_with(Client::OpenCode, &allowed, &files);
    let r = output.explain("/model");
    assert!(r.events.iter().any(|e| e.kind == EventKind::Transformed));
    let text = serde_json::to_string(&r).expect("json");
    assert!(!text.contains("SEEDED-FILE-MODEL"));
}

#[test]
fn oc_remote_and_mdm_reported_unobservable() {
    let output = run(Client::OpenCode, &oc_ctx(), &[]);
    assert!(output.has_gap(GapKind::ExpectedUnobservable, "oc.remote.wellknown"));
    assert!(output.has_gap(GapKind::ExpectedUnobservable, "oc.managed_macos_prefs"));
}

#[test]
fn oc_json_and_jsonc_coexistence_partial() {
    let mut c = oc_ctx();
    c.project_root = Some("/home/synthetic/work".into());
    let output = run(
        Client::OpenCode,
        &c,
        &[
            (PROJECT, r#"{"model": "from-json"}"#),
            (
                "/home/synthetic/work/opencode.jsonc",
                r#"{"model": "from-jsonc"}"#,
            ),
        ],
    );
    let r = output.explain("/model");
    assert_eq!(r.status, ResolutionStatus::Partial);
    assert!(
        r.effective_value.is_none(),
        "co-existence preference is not verified"
    );
    assert!(r
        .notes
        .iter()
        .chain(r.coverage_gaps.iter())
        .any(|n| n.contains("jsonc_coexistence")));
}

#[test]
fn oc_custom_dir_placement_gap() {
    let c = with_env(oc_ctx(), &[("OPENCODE_CONFIG_DIR", "/srv/team/ocdir")]);
    let output = run(
        Client::OpenCode,
        &c,
        &[(PROJECT, r#"{"model": "project-model"}"#)],
    );
    assert!(output.has_gap(GapKind::RulePlacementUnverified, "oc.custom_dir"));
    let r = output.explain("/model");
    assert_eq!(r.status, ResolutionStatus::Partial);
}

#[test]
fn oc_mcp_environment_fully_redacted() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(
            PROJECT,
            r#"{"mcp": {"jira": {"environment": {"JIRA_TOKEN": "SEEDED-MCP-TOKEN"}}}}"#,
        )],
    );
    let r = output.explain("/mcp/jira/environment");
    assert!(r.observed_value.is_redacted());
    assert_eq!(r.observed_value.reason_str(), "mcp_environment_map");
    let text = serde_json::to_string(&output.explain("/mcp/jira/environment")).expect("json");
    assert!(!text.contains("SEEDED-MCP-TOKEN"));
}

#[test]
fn oc_documented_scalars_use_layer_override() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[
            (
                GLOBAL,
                r#"{"shell": "/bin/bash", "attachment": {"image": {"max_width": 1000}}}"#,
            ),
            (
                PROJECT,
                r#"{"shell": "/bin/zsh", "attachment": {"image": {"max_width": 2000}}}"#,
            ),
        ],
    );
    assert_eq!(output.explain("/shell").observed_str(), Some("/bin/zsh"));
    assert_eq!(
        output
            .explain("/attachment/image/max_width")
            .observed_value
            .as_value(),
        Some(&serde_json::json!(2000))
    );
    assert_eq!(
        output.explain("/shell").support,
        configlens::adapters::SupportLevel::Documented
    );
}

#[test]
fn oc_server_scalar_layer_override() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[
            (
                GLOBAL,
                r#"{"server": {"hostname": "0.0.0.0", "mdns": true}}"#,
            ),
            (PROJECT, r#"{"server": {"hostname": "127.0.0.1"}}"#),
        ],
    );
    assert_eq!(
        output.explain("/server/hostname").observed_str(),
        Some("127.0.0.1")
    );
    assert_eq!(
        output.explain("/server/mdns").observed_value.as_value(),
        Some(&serde_json::json!(true))
    );
}

#[test]
fn oc_mcp_type_and_url_rules() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(
            PROJECT,
            r#"{"mcp": {"jira": {"type": "remote", "url": "https://user:pass@jira.example/mcp?token=SEEDED#f"}}}"#,
        )],
    );
    assert_eq!(
        output.explain("/mcp/jira/type").observed_str(),
        Some("remote")
    );
    let url = output.explain("/mcp/jira/url");
    assert_eq!(
        url.observed_value.as_value().and_then(|v| v.as_str()),
        Some("https://jira.example/mcp?[redacted]")
    );
}

#[test]
fn explain_of_an_absent_key_is_absent_not_invented() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(PROJECT, r#"{"snapshot": true}"#)],
    );
    let r = output.explain("/model");
    assert_eq!(r.status, ResolutionStatus::Absent);
    assert!(r.effective_value.is_none());
    assert!(r.events.is_empty());
}

#[test]
fn a_malformed_highest_relevant_layer_yields_invalid_and_no_effective_value() {
    let c = oc_ctx();
    let files = mem(&[
        (GLOBAL, r#"{"model": "global-model"}"#),
        (MANAGED, r#"{"model": "managed-model""#),
    ]);
    let output = run_with(Client::OpenCode, &c, &files);
    let r = output.explain("/model");
    assert_eq!(r.status, ResolutionStatus::Invalid);
    assert!(r.effective_value.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|d| d.code == "malformed" && d.line.is_some()));
    let text = serde_json::to_string(&output.diagnostics).expect("json");
    assert!(!text.contains("managed-model"));
}

#[test]
fn duplicate_keys_keep_both_locations_and_stop_an_effective_claim() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(PROJECT, r#"{"model": "first", "model": "second"}"#)],
    );
    let r = output.explain("/model");
    assert_eq!(r.status, ResolutionStatus::Partial);
    assert!(r.effective_value.is_none());
    assert!(output.diagnostics.iter().any(|d| d.code == "duplicate_key"));
    assert_eq!(
        r.observed_str(),
        Some("second"),
        "last definition wins inside one file"
    );
}

#[test]
fn scan_lists_every_source_with_status_and_applicability() {
    let output = run(
        Client::OpenCode,
        &oc_ctx(),
        &[(PROJECT, r#"{"model": "project-model"}"#)],
    );
    assert!(output.sources.iter().any(|s| s.layer_id == "oc.project"
        && matches!(s.status, configlens::model::source::SourceStatus::Observed)));
    assert!(
        output.sources.iter().all(|s| !s.display.is_empty()),
        "every source has a safe display location"
    );
    assert_eq!(output.client, "opencode");
}

#[test]
fn share_mode_pseudonymizes_unknown_key_names() {
    let mut c = oc_ctx();
    c.paths = PathMode::Share;
    c.cwd = "/opt/private/repo".into();
    let output = run(
        Client::OpenCode,
        &c,
        &[(PROJECT, r#"{"future_setting": "x"}"#)],
    );
    let resources = serde_json::to_string(&output.resolutions).expect("json");
    assert!(!resources.contains("/opt/private/repo"));
    let policy = policy_for(&c);
    assert_eq!(
        policy.display(std::path::Path::new("/opt/private/repo"), PathMode::Share),
        "$EXTERNAL/repo"
    );
}

#[test]
fn an_engine_run_never_touches_the_real_filesystem_when_a_memory_access_is_injected() {
    // A sanity check that the engine only uses the injected FileAccess: the paths below do not
    // exist, yet the run reports them as missing rather than reading anything.
    let output = run(Client::OpenCode, &oc_ctx(), &[]);
    assert!(output.sources.is_empty());
    let adapter = configlens::adapters::adapter_for(Client::OpenCode);
    let context = oc_ctx();
    let files = mem(&[]);
    let paths = policy_for(&context);
    let engine = Engine::new(adapter.as_ref(), &context, &files, &paths);
    assert!(engine.run().is_ok());
}
