//! The ten acceptance examples from the specification (§9), exercised through the CLI.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::Sandbox;
use predicates::prelude::*;

#[test]
fn acceptance_example_1_two_scalar_definitions_keep_both_locations() {
    let sandbox = Sandbox::new(
        &[
            (".claude/settings.json", r#"{"model": "project-model"}"#),
            (".claude/settings.local.json", r#"{"model": "local-model"}"#),
        ],
        &[(".claude/settings.json", r#"{"model": "user-model"}"#)],
    );
    let output = sandbox
        .command()
        .args([
            "explain",
            "/model",
            "--client",
            "claude",
            "--client-version",
            "2.1.296",
        ])
        .output()
        .expect("run");
    let text = String::from_utf8(output.stdout).expect("utf8");
    assert!(
        text.contains("settings.local.json"),
        "winner location missing: {text}"
    );
    assert!(
        text.contains("settings.json"),
        "shadowed location missing: {text}"
    );
    assert!(text.contains("replaced_by_later_layer"));
}

#[test]
fn acceptance_example_2_list_contributions_are_explained_without_authorization_claims() {
    let sandbox = Sandbox::new(
        &[(
            ".claude/settings.json",
            r#"{"permissions": {"allow": ["Bash(npm run test *)"]}}"#,
        )],
        &[(
            ".claude/settings.json",
            r#"{"permissions": {"allow": ["Read(./.env)"]}}"#,
        )],
    );
    // The shared project list waits for workspace trust, so declare the folder trusted.
    let context_path = sandbox.project.join("trusted.json");
    std::fs::write(
        &context_path,
        r#"{"schema_version":"1.0","client":"claude","project_trust":"trusted"}"#,
    )
    .expect("write");

    let output = sandbox
        .command()
        .args([
            "explain",
            "/permissions/allow",
            "--client",
            "claude",
            "--client-version",
            "2.1.296",
            "--input-context",
            context_path.to_str().expect("path"),
        ])
        .output()
        .expect("run");
    let text = String::from_utf8(output.stdout).expect("utf8");
    assert!(
        text.contains("Bash(npm run test *)"),
        "each element must be visible: {text}"
    );
    assert!(text.contains("Read(./.env)"));
    assert!(
        text.contains("not an authorization decision"),
        "the caveat must be printed: {text}"
    );
}

#[test]
fn acceptance_example_3_unknown_managed_context_shows_observed_winner_and_strict_exits_3() {
    let sandbox = Sandbox::new(
        &[],
        &[(".config/opencode/opencode.json", r#"{"model": "m"}"#)],
    );
    sandbox
        .command()
        .args([
            "explain",
            "/model",
            "--client",
            "opencode",
            "--client-version",
            "1.18.35",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("observed"))
        .stdout(predicate::str::contains("effective value withheld"));
    sandbox
        .command()
        .args([
            "explain",
            "/model",
            "--client",
            "opencode",
            "--client-version",
            "1.18.35",
            "--strict",
        ])
        .assert()
        .code(3);
}

#[test]
fn acceptance_example_4_unsupported_version_keeps_the_inventory_only() {
    let sandbox = Sandbox::new(
        &[],
        &[(
            ".config/opencode/opencode.json",
            r#"{"model": "m", "snapshot": false}"#,
        )],
    );
    let output = sandbox
        .command()
        .args([
            "explain",
            "/model",
            "--client",
            "opencode",
            "--client-version",
            "0.0.1",
            "--format",
            "json",
        ])
        .output()
        .expect("run");
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).expect("utf8")).expect("json");
    assert_eq!(parsed["resolutions"][0]["status"], "unsupported");
    assert!(parsed["resolutions"][0]["effective_value"].is_null());
    assert!(parsed["context"]["client_version"] == "0.0.1");
    assert!(parsed["compatibility"]["provisional"] == true);
}

#[test]
fn acceptance_example_5_untrusted_project_layer_is_skipped_and_explained() {
    let context = r#"{"schema_version":"1.0","client":"claude","project_trust":"untrusted"}"#;
    let sandbox = Sandbox::new(
        &[(
            ".claude/settings.json",
            r#"{"permissions": {"allow": ["Bash(ls)"]}}"#,
        )],
        &[],
    );
    let context_path = sandbox.project.join("context.json");
    std::fs::write(&context_path, context).expect("write");
    let output = sandbox
        .command()
        .args([
            "explain",
            "/permissions/allow",
            "--client",
            "claude",
            "--client-version",
            "2.1.296",
            "--input-context",
            context_path.to_str().expect("path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run");
    assert!(output.status.success());
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).expect("utf8")).expect("json");
    assert_eq!(parsed["context"]["trust"], "untrusted");
    let resolution = parsed["resolutions"]
        .as_array()
        .expect("resolutions")
        .iter()
        .find(|r| r["pointer"] == "/permissions/allow")
        .expect("the queried pointer must be present");
    assert_eq!(resolution["status"], "partial");
    assert!(resolution["effective_value"].is_null());
    let reasons: Vec<String> = resolution["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter_map(|e| e["reason_code"].as_str().map(str::to_string))
        .collect();
    assert!(
        reasons.iter().any(|r| r == "workspace_untrusted"),
        "the skipped project layer must be explained: {reasons:?}"
    );
}

#[test]
fn acceptance_example_9_no_config_files_is_an_empty_inventory_not_an_error() {
    let sandbox = Sandbox::new(&[], &[]);
    sandbox
        .command()
        .args(["scan", "--client", "claude", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"sources\": []"))
        .stdout(predicate::str::contains("cc.managed.server"));
}

#[test]
fn acceptance_example_10_runtime_arguments_change_verified_keys_and_flag_unknowns() {
    let sandbox = Sandbox::new(
        &[],
        &[(".claude/settings.json", r#"{"model": "user-model"}"#)],
    );
    let context = r#"{"schema_version":"1.0","client":"claude","client_arguments":["--model","flag-model","--totally-unknown","x"]}"#;
    let context_path = sandbox.project.join("ctx.json");
    std::fs::write(&context_path, context).expect("write");
    let output = sandbox
        .command()
        .args([
            "explain",
            "/model",
            "--client",
            "claude",
            "--client-version",
            "2.1.296",
            "--input-context",
            context_path.to_str().expect("path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run");
    assert!(output.status.success());
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).expect("utf8")).expect("json");
    assert_eq!(
        parsed["resolutions"][0]["observed_value"]["value"],
        "flag-model"
    );
    assert_eq!(parsed["context"]["simulated"], true);
    let codes: Vec<&str> = parsed["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert!(
        codes.contains(&"unrecognised_session_flag"),
        "unknown flags must introduce a gap: {codes:?}"
    );
}

#[test]
fn acceptance_example_5b_trusted_project_applies_normally() {
    let sandbox = Sandbox::new(
        &[(
            ".claude/settings.json",
            r#"{"permissions": {"allow": ["Bash(ls)"]}}"#,
        )],
        &[],
    );
    let context_path = sandbox.project.join("ctx.json");
    std::fs::write(
        &context_path,
        r#"{"schema_version":"1.0","client":"claude","project_trust":"trusted"}"#,
    )
    .expect("write");
    let output = sandbox
        .command()
        .args([
            "explain",
            "/permissions/allow",
            "--client",
            "claude",
            "--client-version",
            "2.1.296",
            "--input-context",
            context_path.to_str().expect("path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run");
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).expect("utf8")).expect("json");
    let resolution = parsed["resolutions"]
        .as_array()
        .expect("resolutions")
        .iter()
        .find(|r| r["pointer"] == "/permissions/allow")
        .expect("the queried pointer must be present");
    let reasons: Vec<&str> = resolution["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter_map(|e| e["reason_code"].as_str())
        .collect();
    assert!(!reasons.contains(&"trust_unknown"), "{reasons:?}");
    assert_eq!(resolution["observed_value"]["value"][0], "Bash(ls)");
}
