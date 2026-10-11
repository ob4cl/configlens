//! CLI contract: exit codes, formats, colour, strict mode, snapshot and diff (spec §4).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::Sandbox;
use predicates::prelude::*;

fn opencode_project() -> Sandbox {
    Sandbox::new(
        &[],
        &[(
            ".config/opencode/opencode.json",
            r#"{"model": "global-model"}"#,
        )],
    )
}

#[test]
fn clients_lists_implemented_and_unsupported_clients() {
    let sandbox = opencode_project();
    sandbox
        .command()
        .arg("clients")
        .assert()
        .success()
        .stdout(predicate::str::contains("opencode"))
        .stdout(predicate::str::contains("claude"))
        .stdout(predicate::str::contains("codex"))
        .stdout(predicate::str::contains("unsupported"));
}

#[test]
fn clients_json_is_one_valid_envelope_without_ansi() {
    let sandbox = opencode_project();
    let output = sandbox
        .command()
        .args(["clients", "--format", "json"])
        .output()
        .expect("run");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("utf8");
    assert!(!text.contains('\u{1b}'), "no ANSI in JSON output");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
    assert_eq!(parsed["schema_version"], "1.0");
}

#[test]
fn explain_requires_a_client() {
    let sandbox = opencode_project();
    sandbox
        .command()
        .args(["explain", "/model"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--client"));
}

#[test]
fn explain_unknown_client_is_a_usage_error_and_unimplemented_client_is_unsupported() {
    let sandbox = opencode_project();
    sandbox
        .command()
        .args(["explain", "/model", "--client", "nope"])
        .assert()
        .code(2);
    sandbox
        .command()
        .args(["explain", "/model", "--client", "codex"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("unsupported"));
}

#[test]
fn explain_text_shows_the_observed_winner_and_the_rule() {
    let sandbox = Sandbox::new(
        &[(".claude/settings.json", r#"{"model": "project-model"}"#)],
        &[(".claude/settings.json", r#"{"model": "user-model"}"#)],
    );
    sandbox
        .command()
        .args([
            "explain",
            "/model",
            "--client",
            "claude",
            "--client-version",
            "2.1.296",
            "--cwd",
            ".",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("/model = \"project-model\""))
        .stdout(predicate::str::contains("Observed winner:"))
        .stdout(predicate::str::contains(
            "Rule: claude.scalar.layer-override",
        ))
        .stdout(predicate::str::contains("Coverage: partial"))
        .stdout(predicate::str::contains("effective value withheld"));
}

#[test]
fn explain_json_has_every_required_field_and_no_ansi() {
    let sandbox = opencode_project();
    let output = sandbox
        .command()
        .args([
            "explain", "/model", "--client", "opencode", "--format", "json",
        ])
        .output()
        .expect("run");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("utf8");
    assert!(!text.contains('\u{1b}'));
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
    for field in [
        "schema_version",
        "tool_version",
        "client",
        "adapter_revision",
        "compatibility",
        "context",
        "sources",
        "resolutions",
        "diagnostics",
        "coverage",
        "redaction",
        "errors",
    ] {
        assert!(parsed.get(field).is_some(), "missing {field}");
    }
    let resolution = &parsed["resolutions"].as_array().expect("array")[0];
    assert!(resolution["events"].is_array());
    assert!(resolution["observed_value"].is_object());
}

#[test]
fn explain_an_unknown_pointer_exits_3_but_still_prints_an_envelope() {
    let sandbox = opencode_project();
    let output = sandbox
        .command()
        .args([
            "explain",
            "/definitely/not/a/key",
            "--client",
            "opencode",
            "--format",
            "json",
        ])
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(3));
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).expect("utf8")).expect("json");
    assert_eq!(parsed["resolutions"][0]["status"], "unsupported");
    assert!(parsed["resolutions"][0]["effective_value"].is_null());
}

#[test]
fn strict_without_a_tested_version_exits_3() {
    let sandbox = opencode_project();
    sandbox
        .command()
        .args([
            "explain",
            "/model",
            "--client",
            "opencode",
            "--client-version",
            "9.9.9",
            "--strict",
        ])
        .assert()
        .code(3);
    // Without --strict the same run is an ordinary provisional report.
    sandbox
        .command()
        .args([
            "explain",
            "/model",
            "--client",
            "opencode",
            "--client-version",
            "9.9.9",
        ])
        .assert()
        .success();
}

#[test]
fn doctor_exits_zero_by_default_and_one_for_warning_threshold() {
    let sandbox = Sandbox::new(
        &[(".claude/settings.json", r#"{"model": "m", "model": "n"}"#)],
        &[],
    );
    // A duplicate key is a warning: default --fail-on error stays 0.
    sandbox
        .command()
        .args([
            "doctor",
            "--client",
            "claude",
            "--client-version",
            "2.1.296",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("duplicate_key"));
    sandbox
        .command()
        .args([
            "doctor",
            "--client",
            "claude",
            "--client-version",
            "2.1.296",
            "--fail-on",
            "warning",
        ])
        .assert()
        .code(1);
}

#[test]
fn doctor_never_claims_the_configuration_is_safe() {
    let sandbox = opencode_project();
    sandbox
        .command()
        .args(["doctor", "--client", "opencode"])
        .assert()
        .success()
        .stdout(predicate::str::contains("never certifies"));
}

#[test]
fn snapshot_creates_new_by_default_and_requires_overwrite() {
    let sandbox = opencode_project();
    let target = sandbox.project.join("before.json");
    sandbox
        .command()
        .args([
            "snapshot",
            "--client",
            "opencode",
            "--output",
            target.to_str().expect("path"),
        ])
        .assert()
        .success();
    assert!(target.exists());
    // Second run without --overwrite must refuse.
    sandbox
        .command()
        .args([
            "snapshot",
            "--client",
            "opencode",
            "--output",
            target.to_str().expect("path"),
        ])
        .assert()
        .code(2);
    sandbox
        .command()
        .args([
            "snapshot",
            "--client",
            "opencode",
            "--output",
            target.to_str().expect("path"),
            "--overwrite",
        ])
        .assert()
        .success();
}

#[test]
fn snapshot_is_redacted_and_owner_only_on_unix() {
    let sandbox = Sandbox::new(&[], &[]);
    let secret = "SEEDED-MCP-TOKEN-1234567890";
    let project = Sandbox::new(
        &[(
            ".claude/settings.json",
            &format!(
                r#"{{"mcpServers": {{"jira": {{"env": {{"TOKEN": "{secret}"}}}}}}, "model": "m"}}"#
            ),
        )],
        &[],
    );
    let target = project.project.join("report.json");
    project
        .command()
        .args([
            "snapshot",
            "--client",
            "claude",
            "--client-version",
            "2.1.296",
            "--output",
            target.to_str().expect("path"),
        ])
        .assert()
        .success();
    let text = std::fs::read_to_string(&target).expect("read");
    assert!(!text.contains(secret), "snapshot must be redacted");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(parsed["redaction"]["mode"], "local");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&target)
            .expect("meta")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "snapshot must be owner-only");
    }
    let _ = sandbox;
}

#[cfg(unix)]
#[test]
fn snapshot_refuses_a_symlink_destination() {
    let sandbox = opencode_project();
    let real = sandbox.project.join("real.json");
    std::fs::write(&real, "{}").expect("write");
    let link = sandbox.project.join("link.json");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    sandbox
        .command()
        .args([
            "snapshot",
            "--client",
            "opencode",
            "--output",
            link.to_str().expect("path"),
            "--overwrite",
        ])
        .assert()
        .code(2);
}

#[test]
fn diff_of_identical_snapshots_reports_no_changes() {
    let sandbox = opencode_project();
    let before = sandbox.project.join("b.json");
    let after = sandbox.project.join("a.json");
    for target in [&before, &after] {
        sandbox
            .command()
            .args([
                "snapshot",
                "--client",
                "opencode",
                "--output",
                target.to_str().expect("path"),
            ])
            .assert()
            .success();
    }
    sandbox
        .command()
        .args([
            "diff",
            before.to_str().expect("path"),
            after.to_str().expect("path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("no differences"));
}

#[test]
fn diff_reports_value_changes_and_unknown_redacted_comparisons() {
    let sandbox = opencode_project();
    let before = sandbox.project.join("b.json");
    sandbox
        .command()
        .args([
            "snapshot",
            "--client",
            "opencode",
            "--output",
            before.to_str().expect("path"),
        ])
        .assert()
        .success();

    // Change the value, then snapshot again.
    std::fs::write(
        sandbox.home.join(".config/opencode/opencode.json"),
        r#"{"model": "changed-model", "mcp": {"jira": {"environment": {"TOKEN": "SEEDED"}}}}"#,
    )
    .expect("write");
    let after = sandbox.project.join("a.json");
    sandbox
        .command()
        .args([
            "snapshot",
            "--client",
            "opencode",
            "--output",
            after.to_str().expect("path"),
        ])
        .assert()
        .success();

    let output = sandbox
        .command()
        .args([
            "diff",
            before.to_str().expect("path"),
            after.to_str().expect("path"),
        ])
        .output()
        .expect("run");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("utf8");
    assert!(text.contains("/model"), "got: {text}");
    assert!(text.contains("value"));
    // A value that is redacted on both sides is not provably unchanged.
    assert!(
        text.contains("unknown_redacted"),
        "redacted pairs must compare as unknown: {text}"
    );
    assert!(!text.contains("SEEDED"));
}

#[test]
fn diff_rejects_incompatible_documents() {
    let sandbox = opencode_project();
    let bad = sandbox.project.join("bad.json");
    std::fs::write(&bad, "{\"schema_version\":\"2.0\"}").expect("write");
    sandbox
        .command()
        .args([
            "diff",
            bad.to_str().expect("path"),
            bad.to_str().expect("path"),
        ])
        .assert()
        .code(2);
}

#[test]
fn schema_emits_the_bundled_report_schema() {
    let sandbox = opencode_project();
    let output = sandbox
        .command()
        .args(["schema", "--kind", "report"])
        .output()
        .expect("run");
    assert!(output.status.success());
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).expect("utf8")).expect("json");
    assert_eq!(parsed["properties"]["schema_version"]["const"], "1.0");
}

#[test]
fn no_color_is_honoured_and_json_never_colours() {
    let sandbox = opencode_project();
    let mut cmd = sandbox.command();
    cmd.env("NO_COLOR", "1");
    cmd.args([
        "explain", "/model", "--client", "opencode", "--color", "always",
    ]);
    let output = cmd.output().expect("run");
    let text = String::from_utf8(output.stdout).expect("utf8");
    assert!(!text.contains('\u{1b}'), "NO_COLOR must win");
}

#[test]
fn a_file_over_the_resource_limit_exits_4() {
    let big = format!("{{\"model\": \"{}\"}}", "a".repeat(2 * 1024 * 1024 + 10));
    let sandbox = Sandbox::new(&[], &[(".config/opencode/opencode.json", &big)]);
    sandbox
        .command()
        .args(["scan", "--client", "opencode"])
        .assert()
        .code(4);
}

#[test]
fn a_malformed_high_priority_file_exits_4_and_is_not_ignored() {
    let sandbox = Sandbox::new(&[(".claude/settings.json", "{ \"model\": \"x\", }")], &[]);
    sandbox
        .command()
        .args(["explain", "/model", "--client", "claude"])
        .assert()
        .code(4)
        .stdout(predicate::str::contains("malformed"));
}

#[test]
fn completions_emit_a_script_for_each_shell() {
    let sandbox = opencode_project();
    for (shell, needle) in [
        ("bash", "complete -F _configlens"),
        ("zsh", "#compdef configlens"),
        ("fish", "complete -c configlens"),
    ] {
        sandbox
            .command()
            .args(["completions", "--shell", shell])
            .assert()
            .success()
            .stdout(predicate::str::contains(needle));
    }
    sandbox
        .command()
        .args(["completions", "--shell", "powershell"])
        .assert()
        .code(2);
}

#[test]
fn help_lists_the_support_report_and_completions_commands_with_examples() {
    let sandbox = opencode_project();
    sandbox
        .command()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("support-report"))
        .stdout(predicate::str::contains("completions"))
        .stdout(predicate::str::contains("Examples:"));
}

#[test]
fn support_report_needs_an_explicit_output_or_preview() {
    let sandbox = opencode_project();
    sandbox
        .command()
        .args(["support-report", "--client", "opencode"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--output"));
}

#[test]
fn help_lists_every_documented_command() {
    let sandbox = opencode_project();
    sandbox
        .command()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("clients"))
        .stdout(predicate::str::contains("scan"))
        .stdout(predicate::str::contains("explain"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("snapshot"))
        .stdout(predicate::str::contains("diff"))
        .stdout(predicate::str::contains("schema"));
}

// --- prerelease version identification -------------------------------------------------------
//
// The tag, the archive names, `--version`, the report `tool_version` and the SBOM root component
// must all identify the same release, so a binary of `0.1.0-beta.3` must never introduce itself as
// `0.1.0`.

#[test]
fn version_output_is_exactly_the_package_version() {
    let sandbox = opencode_project();
    let output = sandbox.command().arg("--version").output().expect("run");
    assert!(output.status.success(), "--version must exit 0");
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf8").trim(),
        format!("configlens {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn a_prerelease_suffix_is_reported_and_never_truncated() {
    let version = env!("CARGO_PKG_VERSION");
    let Some((base, suffix)) = version.split_once('-') else {
        // A final release has no suffix to check; nothing to guard here.
        return;
    };
    assert!(!suffix.is_empty(), "unexpected package version {version}");
    let sandbox = opencode_project();
    let stdout = String::from_utf8(
        sandbox
            .command()
            .arg("--version")
            .output()
            .expect("run")
            .stdout,
    )
    .expect("utf8");
    assert_eq!(
        stdout.trim(),
        format!("configlens {version}"),
        "the prerelease suffix must be reported verbatim"
    );
    assert_ne!(
        stdout.trim(),
        format!("configlens {base}"),
        "a prerelease build must not present itself as the bare base version"
    );
}

#[test]
fn reported_versions_agree_and_schema_and_adapter_revisions_stay_independent() {
    let sandbox = opencode_project();
    let output = sandbox
        .command()
        .args([
            "scan",
            "--client",
            "opencode",
            "--client-version",
            "1.18.35",
            "--format",
            "json",
        ])
        .output()
        .expect("run");
    assert!(output.status.success());
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).expect("utf8")).expect("json");
    // Same value as `--version` and the release tag: never a second, drifting copy.
    assert_eq!(parsed["tool_version"], env!("CARGO_PKG_VERSION"));
    // Deliberately independent of the package version.
    assert_eq!(parsed["schema_version"], "1.0");
    assert_eq!(parsed["adapter_revision"], "0.1.0");
}

// --- --quiet -----------------------------------------------------------------------------------

#[test]
fn without_quiet_the_snapshot_note_goes_to_stderr() {
    let sandbox = opencode_project();
    let target = sandbox.path("noted.json");
    let output = sandbox
        .command()
        .args([
            "snapshot",
            "--client",
            "opencode",
            "--client-version",
            "1.18.35",
            "--output",
            target.to_str().expect("path"),
        ])
        .output()
        .expect("run");
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("wrote"),
        "the informational note belongs on stderr"
    );
    assert!(
        output.stdout.is_empty(),
        "snapshot writes nothing to stdout"
    );
}

#[test]
fn quiet_suppresses_only_the_informational_note_and_still_writes_the_snapshot() {
    let sandbox = opencode_project();
    let target = sandbox.path("quiet.json");
    let output = sandbox
        .command()
        .args([
            "snapshot",
            "--client",
            "opencode",
            "--client-version",
            "1.18.35",
            "--output",
            target.to_str().expect("path"),
            "--quiet",
        ])
        .output()
        .expect("run");
    assert!(output.status.success(), "the exit code must not change");
    assert!(
        output.stderr.is_empty(),
        "--quiet must silence the note, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(target.exists(), "the snapshot is still written");
}

#[test]
fn quiet_keeps_the_preview_stdout_machine_readable() {
    let sandbox = opencode_project();
    let output = sandbox
        .command()
        .args([
            "support-report",
            "--client",
            "opencode",
            "--client-version",
            "1.18.35",
            "--preview",
            "--quiet",
        ])
        .output()
        .expect("run");
    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "no note with --quiet");
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).expect("utf8"))
            .expect("--quiet must leave stdout as pure JSON");
    assert_eq!(parsed["command"], "support-report");
}

#[test]
fn quiet_is_a_global_flag_that_never_silences_stdout() {
    let sandbox = opencode_project();
    // Accepted on every subcommand, and the product output still goes to stdout.
    sandbox
        .command()
        .args(["clients", "--quiet"])
        .assert()
        .success()
        .stdout(predicate::str::contains("opencode"));
}

#[test]
fn quiet_never_silences_a_usage_error() {
    let sandbox = opencode_project();
    // Missing --client is a usage error: still exit 2, still explained on stderr.
    sandbox
        .command()
        .args(["explain", "/model", "--quiet"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--client"));
}

#[test]
fn quiet_never_silences_the_support_report_usage_error() {
    let sandbox = opencode_project();
    sandbox
        .command()
        .args(["support-report", "--client", "opencode", "--quiet"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--output"));
}
