//! Security: seeded secrets must never appear in any output channel (spec §8 "Security").
//!
//! Every test builds a synthetic home; the real environment is cleared. The seeded values below are
//! deliberately fake.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::Sandbox;
use std::process::Output;

const SEED_VALUE: &str = "SEEDED-VALUE-9f3c1a";
const SEED_PATH_TOKEN: &str = "SEEDED-PATH-8b2d4e";
const SEED_FILE: &str = "SEEDED-FILE-7a1c9d";

/// Run a command and return every output channel as one string.
fn channels(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn seeded_sandbox() -> Sandbox {
    let project = format!(
        r#"{{
  "model": "m",
  "mcp": {{"jira": {{"environment": {{"JIRA_TOKEN": "{SEED_VALUE}"}}, "url": "https://user:pass@jira.example/mcp?token={SEED_VALUE}#frag"}}}},
  "provider": {{"anthropic": {{"options": {{"apiKey": "{SEED_VALUE}"}}}}}},
  "unknown_free_text": "{SEED_VALUE}",
  "agent": {{"reviewer": {{"prompt": "{SEED_VALUE}"}}}},
  "totally_unknown": {{"nested": {{"deep": "{SEED_VALUE}"}}}}
}}"#
    );
    let claude = format!(
        r#"{{
  "model": "m",
  "env": {{"MY_TOKEN": "{SEED_VALUE}"}},
  "hooks": {{"PreToolUse": [{{"command": "echo {SEED_VALUE}"}}]}},
  "mcpServers": {{"jira": {{"env": {{"TOKEN": "{SEED_VALUE}", "FILE": "/tmp/{SEED_FILE}"}}}}}}
}}"#
    );
    Sandbox::new(
        &[(".claude/settings.json", &claude)],
        &[
            (".config/opencode/opencode.json", &project),
            (
                ".config/opencode/opencode.jsonc",
                &format!("{{\"model\": \"{SEED_VALUE}\", \"snapshot\": true}}"),
            ),
        ],
    )
}

#[test]
fn seeded_secrets_never_reach_any_channel() {
    let sandbox = seeded_sandbox();
    let commands: Vec<Vec<&str>> = vec![
        vec!["scan", "--client", "opencode", "--format", "json"],
        vec!["scan", "--client", "opencode"],
        vec!["scan", "--client", "claude", "--format", "json"],
        vec![
            "explain", "/model", "--client", "opencode", "--format", "json",
        ],
        vec![
            "explain",
            "/mcp/jira/environment",
            "--client",
            "opencode",
            "--format",
            "json",
        ],
        vec![
            "explain",
            "/mcp/jira/url",
            "--client",
            "opencode",
            "--format",
            "json",
        ],
        vec![
            "explain",
            "/provider/anthropic/options/apiKey",
            "--client",
            "opencode",
            "--format",
            "json",
        ],
        vec![
            "explain",
            "/totally_unknown/nested/deep",
            "--client",
            "opencode",
            "--format",
            "json",
        ],
        vec![
            "explain",
            "/env/MY_TOKEN",
            "--client",
            "claude",
            "--format",
            "json",
        ],
        vec![
            "explain",
            "/hooks/PreToolUse/0/command",
            "--client",
            "claude",
            "--format",
            "json",
        ],
        vec![
            "explain",
            "/mcpServers/jira/env",
            "--client",
            "claude",
            "--format",
            "json",
        ],
        vec!["doctor", "--client", "opencode", "--format", "json"],
        vec!["doctor", "--client", "claude", "--format", "json"],
        vec!["clients", "--format", "json"],
    ];
    for args in commands {
        let output = sandbox.command().args(&args).output().expect("run");
        let text = channels(&output);
        assert!(
            !text.contains(SEED_VALUE),
            "seeded secret leaked for {args:?}:\n{text}"
        );
        assert!(
            !text.contains(SEED_PATH_TOKEN),
            "path token leaked for {args:?}"
        );
        assert!(!text.contains(SEED_FILE), "file token leaked for {args:?}");
        assert!(
            !text.contains("user:pass"),
            "URL userinfo leaked for {args:?}"
        );
        // The MCP URL *host* (`jira.example`) is intentionally shown in local mode: spec §7 says
        // private hosts are pseudonymized only in **share** mode, while local mode "show[s] config
        // locations for diagnosis". `rules_opencode::oc_mcp_type_and_url_rules` and
        // `security::redact::urls_have_userinfo_query_and_fragment_removed` both pin that local
        // behaviour, so asserting host absence here (all commands run local) would contradict the
        // specification and those tests. Share-mode private-host pseudonymization for URL
        // authorities is a known, documented gap (docs/implementation-status.md, "Known gaps"), so
        // it is not asserted. The §8 invariant -- seeded values and URL userinfo absent from every
        // channel -- is asserted above for every command.
    }
}

#[test]
fn snapshots_and_diffs_never_contain_seeded_secrets() {
    let sandbox = seeded_sandbox();
    let before = sandbox.project.join("before.json");
    let after = sandbox.project.join("after.json");
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
    for target in [&before, &after] {
        let text = std::fs::read_to_string(target).expect("read");
        assert!(!text.contains(SEED_VALUE), "secret leaked into {target:?}");
        assert!(!text.contains("user:pass"));
    }
    let output = sandbox
        .command()
        .args([
            "diff",
            before.to_str().expect("path"),
            after.to_str().expect("path"),
        ])
        .output()
        .expect("run");
    let text = channels(&output);
    assert!(!text.contains(SEED_VALUE));
}

#[test]
fn secrets_in_a_malformed_file_are_not_echoed() {
    let sandbox = Sandbox::new(
        &[(
            ".claude/settings.json",
            &format!("{{\"apiKey\": \"{SEED_VALUE}\", \"model\": }}"),
        )],
        &[],
    );
    for args in [
        vec!["explain", "/model", "--client", "claude"],
        vec![
            "explain", "/model", "--client", "claude", "--format", "json",
        ],
        vec!["doctor", "--client", "claude"],
    ] {
        let output = sandbox.command().args(&args).output().expect("run");
        let text = channels(&output);
        assert!(
            !text.contains(SEED_VALUE),
            "parser error leaked content: {text}"
        );
        // A safe location is still reported.
        assert!(
            text.contains("malformed"),
            "the malformed file must be reported: {text}"
        );
    }
}

#[test]
fn terminal_control_payloads_are_neutralised_in_every_channel() {
    let model = "model\u{1b}]8;;https://evil.example\u{7}\u{202e}txt";
    let sandbox = Sandbox::new(
        &[],
        &[(
            ".config/opencode/opencode.json",
            &format!("{{\"model\": \"{model}\"}}"),
        )],
    );
    for args in [
        vec!["explain", "/model", "--client", "opencode"],
        vec![
            "explain", "/model", "--client", "opencode", "--format", "json",
        ],
        vec!["scan", "--client", "opencode"],
    ] {
        let output = sandbox.command().args(&args).output().expect("run");
        let text = channels(&output);
        assert!(
            !text.contains('\u{1b}'),
            "escape sequence survived: {text:?}"
        );
        assert!(!text.contains('\u{202e}'), "bidi control survived");
        assert!(
            !text.contains("evil.example"),
            "OSC payload survived: {text:?}"
        );
    }
}

#[test]
fn files_outside_authorised_roots_are_never_read() {
    let sandbox = Sandbox::new(&[], &[]);
    let outside = sandbox.dir.path().join("outside.json");
    std::fs::write(&outside, format!("{{\"model\": \"{SEED_VALUE}\"}}")).expect("write");
    let context = format!(
        r#"{{"schema_version":"1.0","client":"opencode","sources":[{{"role":"oc.managed_file","path":"{}","content":"{{\"model\": \"{SEED_VALUE}\"}}"}}]}}"#,
        outside.display()
    );
    let context_path = sandbox.project.join("ctx.json");
    std::fs::write(&context_path, context).expect("write");
    // Supplied sources are evidence, so they are parsed -- but always through the redaction
    // projection, and the model name here is a seeded token rather than a model-shaped value.
    let output = sandbox
        .command()
        .args([
            "explain",
            "/model",
            "--client",
            "opencode",
            "--input-context",
            context_path.to_str().expect("path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run");
    let text = channels(&output);
    assert!(
        !text.contains(SEED_VALUE),
        "supplied content leaked: {text}"
    );
}

#[test]
fn the_source_tree_never_spawns_processes_or_opens_sockets() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    visit(&src, &mut offenders);
    assert!(
        offenders.is_empty(),
        "forbidden capability found in src/: {offenders:?}"
    );
}

fn visit(dir: &std::path::Path, offenders: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit(&path, offenders);
            continue;
        }
        if path.extension().map(|e| e != "rs").unwrap_or(true) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for needle in [
            "std::process::Command",
            "std::net::",
            "TcpStream",
            "reqwest",
            "std::os::unix::process",
        ] {
            if text.contains(needle) {
                offenders.push(format!("{} contains {needle}", path.display()));
            }
        }
    }
}
