//! Claude Code adapter rules, end to end through the resolution engine.
//! Test ids here are the ones `compatibility/claude/registry.json` cites.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{ctx, run, run_with, with_args, with_env, with_trust};
use configlens::adapters::{Client, SupportLevel};
use configlens::context::{ResolutionContext, Trust};
use configlens::model::event::EventKind;
use configlens::resolve::ResolutionStatus;

const USER: &str = "/home/synthetic/.claude/settings.json";
const PROJECT: &str = "/home/synthetic/work/.claude/settings.json";
const LOCAL: &str = "/home/synthetic/work/.claude/settings.local.json";
const MANAGED: &str = "/etc/claude-code/managed-settings.json";

fn cc_ctx() -> ResolutionContext {
    let mut c = ctx("claude", "/home/synthetic/work");
    c.project_root = Some("/home/synthetic/work".into());
    with_trust(c, Trust::Trusted)
}

#[test]
fn cc_local_overrides_project_overrides_user() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[
            (USER, r#"{"model": "user-model"}"#),
            (PROJECT, r#"{"model": "project-model"}"#),
            (LOCAL, r#"{"model": "local-model"}"#),
        ],
    );
    let r = output.explain("/model");
    assert_eq!(r.observed_str(), Some("local-model"));
    assert_eq!(r.event_kinds(), vec!["replaced", "replaced", "selected"]);
    let locations: Vec<&str> = r
        .events
        .iter()
        .filter_map(|e| e.location.as_deref())
        .collect();
    assert!(locations[0].contains("settings.json"));
    assert!(locations[2].contains("settings.local.json"));
}

#[test]
fn cc_managed_overrides_user_files() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[
            (USER, r#"{"model": "user-model", "cleanupPeriodDays": 5}"#),
            (MANAGED, r#"{"model": "managed-model"}"#),
        ],
    );
    let r = output.explain("/model");
    assert_eq!(r.observed_str(), Some("managed-model"));
    // MDM, server-managed settings and the HKCU registry sit above the managed file and are not
    // observable here, so nothing may be called effective.
    assert_eq!(r.status, ResolutionStatus::Partial);
    assert!(r.effective_value.is_none());
    assert!(output.has_gap(
        configlens::resolve::GapKind::ExpectedUnobservable,
        "cc.managed.server"
    ));
}

#[test]
fn cc_session_settings_flag_overrides_files() {
    let c = with_args(cc_ctx(), &["--settings", r#"{"model": "session-model"}"#]);
    let output = run(
        Client::Claude,
        &c,
        &[
            (USER, r#"{"model": "user-model"}"#),
            (LOCAL, r#"{"model": "local-model"}"#),
        ],
    );
    let r = output.explain("/model");
    assert_eq!(r.observed_str(), Some("session-model"));
    assert!(r
        .notes
        .iter()
        .any(|n| n.contains("simulated") || n.contains("supplied")));
}

#[test]
fn cc_model_env_pair_overrides_file() {
    let c = with_env(cc_ctx(), &[("ANTHROPIC_MODEL", "env-model")]);
    let output = run(Client::Claude, &c, &[(USER, r#"{"model": "user-model"}"#)]);
    let r = output.explain("/model");
    assert_eq!(r.observed_str(), Some("env-model"));
    assert!(r
        .events
        .iter()
        .any(|e| e.reason_code.as_deref() == Some("env_pair_overrides_files")));
    assert_eq!(
        output.explain("/model").rule_id.as_deref(),
        Some("claude.scalar.layer-override")
    );
}

#[test]
fn cc_default_model_env_only_when_no_file_sets_key() {
    let c = with_env(cc_ctx(), &[("ANTHROPIC_DEFAULT_MODEL", "default-model")]);
    let with_file = run(Client::Claude, &c, &[(USER, r#"{"model": "user-model"}"#)]);
    assert_eq!(
        with_file.explain("/model").observed_str(),
        Some("user-model")
    );

    let without_file = run(Client::Claude, &c, &[]);
    let r = without_file.explain("/model");
    assert_eq!(r.observed_str(), Some("default-model"));
    assert!(r
        .events
        .iter()
        .any(|e| e.reason_code.as_deref() == Some("env_pair_default")));
}

#[test]
fn cc_permissions_allow_lists_concat_and_dedupe() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[
            (
                USER,
                r#"{"permissions": {"allow": ["Bash(npm run test *)", "Read(./.env)"]}}"#,
            ),
            (
                PROJECT,
                r#"{"permissions": {"allow": ["Read(./.env)", "Bash(npm run lint)"]}}"#,
            ),
        ],
    );
    let r = output.explain("/permissions/allow");
    let list = r
        .observed_value
        .as_value()
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        list.len(),
        3,
        "duplicates are removed, order is stable: {list:?}"
    );
    assert_eq!(list[0], serde_json::json!("Bash(npm run test *)"));
    assert_eq!(list[1], serde_json::json!("Read(./.env)"));
    assert_eq!(list[2], serde_json::json!("Bash(npm run lint)"));
    assert_eq!(
        r.events
            .iter()
            .filter(|e| e.kind == EventKind::Contributed)
            .count(),
        2
    );
}

#[test]
fn cc_permissions_list_concat_negative_no_authorization_claim() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[(
            USER,
            r#"{"permissions": {"allow": ["Bash(npm run test *)"]}}"#,
        )],
    );
    let r = output.explain("/permissions/allow");
    assert!(
        r.notes.iter().any(|n| n.contains("not an authorization")),
        "got notes: {:?}",
        r.notes
    );
    assert!(r.authorization_note().is_some());
}

#[test]
fn cc_default_mode_scope_exclusion_ignores_project_value() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[
            (USER, r#"{"permissions": {"defaultMode": "plan"}}"#),
            (
                PROJECT,
                r#"{"permissions": {"defaultMode": "bypassPermissions"}}"#,
            ),
        ],
    );
    let r = output.explain("/permissions/defaultMode");
    assert_eq!(r.observed_str(), Some("plan"));
    assert!(r.ignored_reasons().contains(&"scope_excluded_value"));
    assert!(r
        .notes
        .iter()
        .any(|n| n.contains("does not take effect from project")));
}

#[test]
fn cc_fallback_model_ordered_whole_replace() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[
            (USER, r#"{"fallbackModel": ["a", "b"]}"#),
            (PROJECT, r#"{"fallbackModel": ["c"]}"#),
        ],
    );
    let r = output.explain("/fallbackModel");
    let list = r
        .observed_value
        .as_value()
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        list,
        vec![serde_json::json!("c")],
        "the whole chain is replaced"
    );
    assert_eq!(r.event_kinds(), vec!["replaced", "selected"]);
}

#[test]
fn cc_available_models_locks_model_choice() {
    let allowed = run(
        Client::Claude,
        &cc_ctx(),
        &[
            (USER, r#"{"model": "claude-opus-5-5"}"#),
            (MANAGED, r#"{"availableModels": ["claude-opus-5-5"]}"#),
        ],
    );
    let r = allowed.explain("/model");
    assert!(r
        .events
        .iter()
        .any(|e| e.kind == EventKind::Constrained && e.rule_id.is_some()));
    assert_eq!(r.ignored_reasons(), Vec::<&str>::new());

    let blocked = run(
        Client::Claude,
        &cc_ctx(),
        &[
            (USER, r#"{"model": "claude-sonnet-5"}"#),
            (MANAGED, r#"{"availableModels": ["claude-opus-5-5"]}"#),
        ],
    );
    let r = blocked.explain("/model");
    assert_eq!(r.status, ResolutionStatus::Partial);
    assert!(r.effective_value.is_none());
    assert!(r
        .notes
        .iter()
        .any(|n| n.contains("not in the managed allowlist")));
}

#[test]
fn cc_allow_from_project_conditional_when_trust_unknown() {
    let unknown = with_trust(cc_ctx(), Trust::Unknown);
    let output = run(
        Client::Claude,
        &unknown,
        &[(PROJECT, r#"{"permissions": {"allow": ["Bash(ls)"]}}"#)],
    );
    let r = output.explain("/permissions/allow");
    assert_eq!(r.status, ResolutionStatus::Partial);
    assert!(r.effective_value.is_none());
    assert!(output.has_gap(configlens::resolve::GapKind::TrustUnknown, "run"));
    assert!(!r.ignored_reasons().is_empty());

    let trusted = run(
        Client::Claude,
        &cc_ctx(),
        &[(PROJECT, r#"{"permissions": {"allow": ["Bash(ls)"]}}"#)],
    );
    assert_eq!(
        trusted
            .explain("/permissions/allow")
            .ignored_reasons()
            .len(),
        0
    );
}

#[test]
fn cc_untrusted_shared_project_gates_only_allow_deny_still_applies() {
    // C4 regression: trust gating is key-granular, not file-granular. From an untrusted *shared*
    // project file the `allow` rule waits for trust, but `deny` applies immediately — the whole
    // file must not be dropped (the pre-fix behaviour).
    let untrusted = with_trust(cc_ctx(), Trust::Untrusted);
    let output = run(
        Client::Claude,
        &untrusted,
        &[(
            PROJECT,
            r#"{"permissions": {"allow": ["Bash(ls)"], "deny": ["Read(./.env)"]}}"#,
        )],
    );

    let allow = output.explain("/permissions/allow");
    assert_eq!(allow.status, ResolutionStatus::Partial);
    assert!(
        allow.ignored_reasons().contains(&"workspace_untrusted"),
        "shared allow must be gated: {:?}",
        allow.ignored_reasons()
    );

    let deny = output.explain("/permissions/deny");
    assert!(
        deny.ignored_reasons().is_empty(),
        "deny rules apply right away, even in an untrusted project: {:?}",
        deny.ignored_reasons()
    );
    let list = deny
        .observed_value
        .as_value()
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(list, vec![serde_json::json!("Read(./.env)")]);
}

#[test]
fn cc_untrusted_local_allow_applies_without_trust() {
    // C4 regression: an *untracked* local file's allow rules apply without the trust step.
    let untrusted = with_trust(cc_ctx(), Trust::Untrusted);
    let output = run(
        Client::Claude,
        &untrusted,
        &[(
            LOCAL,
            r#"{"permissions": {"allow": ["Bash(npm run test *)"]}}"#,
        )],
    );
    let allow = output.explain("/permissions/allow");
    assert!(
        allow.ignored_reasons().is_empty(),
        "untracked local allow applies without trust: {:?}",
        allow.ignored_reasons()
    );
    let list = allow
        .observed_value
        .as_value()
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(list, vec![serde_json::json!("Bash(npm run test *)")]);
}

#[test]
fn cc_untrusted_project_non_permission_keys_still_apply() {
    // C4 regression: only the trust-gated keys wait for trust; ordinary keys (e.g. `model`) from a
    // project file apply even when the workspace is untrusted.
    let untrusted = with_trust(cc_ctx(), Trust::Untrusted);
    let output = run(
        Client::Claude,
        &untrusted,
        &[
            (USER, r#"{"model": "user-model"}"#),
            (PROJECT, r#"{"model": "project-model"}"#),
        ],
    );
    assert_eq!(
        output.explain("/model").observed_str(),
        Some("project-model"),
        "a non-permission project key is not trust-gated"
    );
}

#[test]
fn cc_env_values_from_project_files_are_redacted_and_trust_gated() {
    let output = run(
        Client::Claude,
        &with_trust(cc_ctx(), Trust::Unknown),
        &[(PROJECT, r#"{"env": {"MY_SECRET": "SEEDED-ENV"}}"#)],
    );
    let r = output.explain("/env/MY_SECRET");
    assert!(r.observed_value.is_redacted());
    assert!(r.effective_value.is_none());
    let text = serde_json::to_string(&r).expect("json");
    assert!(!text.contains("SEEDED-ENV"));
}

#[test]
fn cc_object_nested_merge_key_by_key() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[
            (
                USER,
                r#"{"sandbox": {"enabled": true, "network": {"allowedDomains": ["a.example"]}}}"#,
            ),
            (
                PROJECT,
                r#"{"sandbox": {"network": {"allowedDomains": ["b.example"]}}}"#,
            ),
        ],
    );
    assert_eq!(
        output.explain("/sandbox/enabled").observed_value.as_value(),
        Some(&serde_json::json!(true)),
        "unrelated keys survive"
    );
    let domains = output
        .explain("/sandbox/network/allowedDomains")
        .observed_value
        .as_value()
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        domains,
        vec![
            serde_json::json!("a.example"),
            serde_json::json!("b.example")
        ],
        "documented list merging inside nested blocks"
    );
}

#[test]
fn cc_unknown_key_inventory_only() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[(USER, r#"{"someFutureKey": "value"}"#)],
    );
    let r = output.explain("/someFutureKey");
    assert_eq!(r.status, ResolutionStatus::Unsupported);
    assert_eq!(r.support, SupportLevel::InventoryOnly);
    assert_eq!(r.rule_id.as_deref(), Some("claude.unknown.inventory-only"));
}

#[test]
fn cc_hooks_are_inventoried_and_never_read() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[(
            USER,
            r#"{"hooks": {"PreToolUse": [{"command": "SEEDED-HOOK-COMMAND"}]}}"#,
        )],
    );
    let r = output.explain("/hooks/PreToolUse/0/command");
    assert!(r.observed_value.is_redacted());
    let text = serde_json::to_string(&output.resolutions).expect("json");
    assert!(!text.contains("SEEDED-HOOK-COMMAND"));
}

#[test]
fn cc_managed_missing_reported_unobservable() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[(USER, r#"{"model": "user-model"}"#)],
    );
    assert!(output.has_gap(
        configlens::resolve::GapKind::ManagedStateUnknown,
        "cc.managed.file"
    ));
    let r = output.explain("/model");
    assert_eq!(r.status, ResolutionStatus::Partial);
    assert!(r.effective_value.is_none());
}

#[test]
fn cc_unknown_version_provisional() {
    let mut c = cc_ctx();
    c.client_version = configlens::context::VersionContext::Claimed("9.9.9".into());
    let output = run(Client::Claude, &c, &[(USER, r#"{"model": "user-model"}"#)]);
    let r = output.explain("/model");
    assert_eq!(r.status, ResolutionStatus::Unsupported);
    assert_eq!(r.support, SupportLevel::Unsupported);
    assert!(r.effective_value.is_none());
    assert!(output.has_gap(configlens::resolve::GapKind::VersionUntested, "run"));
}

#[test]
fn cc_documented_scalars_use_layer_override() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[
            (
                USER,
                r#"{"theme": "dark", "verbose": false, "cleanupPeriodDays": 7}"#,
            ),
            (PROJECT, r#"{"theme": "light"}"#),
        ],
    );
    assert_eq!(output.explain("/theme").observed_str(), Some("light"));
    assert_eq!(
        output.explain("/verbose").observed_value.as_value(),
        Some(&serde_json::json!(false))
    );
    assert_eq!(
        output
            .explain("/cleanupPeriodDays")
            .observed_value
            .as_value(),
        Some(&serde_json::json!(7))
    );
    assert_eq!(output.explain("/theme").support, SupportLevel::Documented);
}

#[test]
fn cc_project_local_file_from_the_repository_root_wins_over_the_starting_directory() {
    let dir = common::project(&[
        (".git/HEAD", "ref: refs/heads/main\n"),
        (".claude/settings.local.json", r#"{"model": "root-local"}"#),
        (
            "sub/.claude/settings.local.json",
            r#"{"model": "legacy-local"}"#,
        ),
    ]);
    let root = dir.path();
    let mut c = ctx("claude", &root.join("sub").display().to_string());
    c.project_root = Some(root.to_path_buf());
    c.home = root.join("home");
    let output = run_with(Client::Claude, &c, &configlens::filesystem::RealFileAccess);
    let r = output.explain("/model");
    assert_eq!(r.observed_str(), Some("root-local"));
    assert!(r.ignored_reasons().contains(&"legacy_pre_v2_1_211_path"));
}

#[test]
fn cc_malformed_project_file_is_not_silently_ignored() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[
            (LOCAL, r#"{"model": "local-model",}"#),
            (USER, r#"{"model": "user-model"}"#),
        ],
    );
    let r = output.explain("/model");
    assert_eq!(r.status, ResolutionStatus::Invalid);
    assert!(r.effective_value.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|d| d.code == "malformed" && d.severity == configlens::resolve::Severity::Error));
    // The lower-precedence value may be observed, but it is never called the runtime value.
    assert_eq!(r.observed_str(), Some("user-model"));
}

#[test]
fn cc_comment_in_a_settings_file_is_a_syntax_error() {
    let output = run(
        Client::Claude,
        &cc_ctx(),
        &[(
            USER,
            "{\n  // comments are not allowed\n  \"model\": \"user-model\"\n}",
        )],
    );
    assert!(output
        .diagnostics
        .iter()
        .any(|d| d.code == "malformed" && d.line == Some(2)));
    assert!(output.explain("/model").effective_value.is_none());
}

#[test]
fn cc_no_config_files_is_an_empty_inventory_not_an_error() {
    let output = run(Client::Claude, &cc_ctx(), &[]);
    assert!(output.explain("/model").status == ResolutionStatus::Absent);
    // Regression: a planned location whose file is absent is not a source. The empty inventory is
    // represented by coverage gaps (managed state, boundary), never by `sources[].status = missing`.
    assert!(
        output.sources.is_empty(),
        "no config files must yield an empty inventory: {:?}",
        output.sources
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|d| d.severity == configlens::resolve::Severity::Error)
            .count(),
        0
    );
    assert!(output.has_gap(
        configlens::resolve::GapKind::ExpectedUnobservable,
        "cc.managed.mdm"
    ));
}
