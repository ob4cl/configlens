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
fn control_characters_in_key_names_never_reach_a_channel() {
    // A key name is attacker-controlled text that becomes part of a JSON Pointer. The pointer is a
    // display string in every channel (report, text, diff), so a control character in a key must be
    // neutralised before it can reach a terminal (spec §7 "pointer displays").
    let key = "evil\\u001b[31mkey\\u202e\\u200b";
    let sandbox = Sandbox::new(
        &[(
            ".claude/settings.json",
            &format!("{{\"model\": \"m\", \"{key}\": 1}}"),
        )],
        &[(
            ".config/opencode/opencode.json",
            &format!("{{\"model\": \"m\", \"{key}\": 1}}"),
        )],
    );
    let commands: Vec<Vec<&str>> = vec![
        vec!["scan", "--client", "opencode"],
        vec!["scan", "--client", "opencode", "--format", "json"],
        vec!["scan", "--client", "claude"],
        vec!["scan", "--client", "claude", "--format", "json"],
        vec![
            "scan", "--client", "claude", "--paths", "share", "--format", "json",
        ],
        vec![
            "explain", "/model", "--client", "claude", "--format", "json",
        ],
        vec!["doctor", "--client", "opencode"],
        vec!["doctor", "--client", "opencode", "--format", "json"],
    ];
    for args in commands {
        let output = sandbox.command().args(&args).output().expect("run");
        let text = channels(&output);
        assert!(
            !text.contains('\u{1b}') && !text.to_lowercase().contains("\\u001b"),
            "escape sequence survived {args:?}: {text:?}"
        );
        assert!(
            !text.contains('\u{202e}') && !text.contains('\u{200b}'),
            "invisible control survived {args:?}"
        );
    }

    // The diff channel renders pointers from snapshot files too.
    let before = sandbox.path("before.json");
    let after = sandbox.path("after.json");
    for (path, client) in [(&before, "claude"), (&after, "claude")] {
        sandbox
            .command()
            .args([
                "snapshot",
                "--client",
                client,
                "--output",
                path.to_str().expect("utf-8 path"),
            ])
            .output()
            .expect("snapshot");
    }
    for format in [None, Some("json")] {
        let mut args = vec!["diff", before.to_str().unwrap(), after.to_str().unwrap()];
        if let Some(format) = format {
            args.push("--format");
            args.push(format);
        }
        let output = sandbox.command().args(&args).output().expect("diff");
        let text = channels(&output);
        assert!(
            !text.contains('\u{1b}') && !text.to_lowercase().contains("\\u001b"),
            "escape sequence survived in diff {format:?}: {text:?}"
        );
        assert!(!text.contains('\u{202e}'), "bidi control survived in diff");
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
fn share_mode_pseudonymizes_url_hosts_but_local_mode_keeps_them_for_diagnosis() {
    let sandbox = seeded_sandbox();
    // Local mode is the diagnostic view: the private host is shown (spec §7), but never the
    // userinfo, query or fragment.
    let local = sandbox
        .command()
        .args(["explain", "/mcp/jira/url", "--client", "opencode"])
        .output()
        .expect("run");
    let local_text = channels(&local);
    assert!(
        local_text.contains("jira.example"),
        "local shows the host: {local_text}"
    );
    assert!(!local_text.contains("user:pass"));
    assert!(!local_text.contains(SEED_VALUE));

    // Share mode pseudonymizes the private host in *every* output (text and JSON).
    for extra in [Vec::<&str>::new(), vec!["--format", "json"]] {
        let share = sandbox
            .command()
            .args([
                "explain",
                "/mcp/jira/url",
                "--client",
                "opencode",
                "--paths",
                "share",
            ])
            .args(&extra)
            .output()
            .expect("run");
        let text = channels(&share);
        assert!(
            !text.contains("jira.example"),
            "share mode must pseudonymize the URL host: {text}"
        );
        assert!(
            text.contains("host_"),
            "share mode must show a host ID: {text}"
        );
        assert!(!text.contains("user:pass"));
        assert!(!text.contains(SEED_VALUE));
    }
}

#[test]
fn share_mode_aliases_embedded_absolute_paths() {
    let sandbox = Sandbox::new(&[], &[]);
    let home = sandbox.home.to_str().expect("home").to_string();
    let shell = format!("{home}/.local/bin/zsh");
    std::fs::create_dir_all(sandbox.home.join(".config/opencode")).expect("mkdir");
    // Built with serde_json: on Windows the home path contains backslashes, which a
    // hand-written JSON string would turn into invalid escapes and the file would not parse.
    let body = serde_json::json!({ "shell": shell }).to_string();
    std::fs::write(sandbox.home.join(".config/opencode/opencode.json"), body).expect("write");

    let share = sandbox
        .command()
        .args([
            "explain", "/shell", "--client", "opencode", "--paths", "share",
        ])
        .output()
        .expect("run");
    let text = channels(&share);
    assert!(
        !text.contains(&home),
        "share mode must alias the embedded home path: {text}"
    );
    assert!(
        text.contains("$HOME/.local/bin/zsh"),
        "share mode must show the alias: {text}"
    );
}

#[test]
fn share_mode_text_output_pseudonymizes_unknown_key_names_too() {
    let sandbox = seeded_sandbox();
    let key = "totally_unknown";
    let share = sandbox
        .command()
        .args([
            "explain",
            &format!("/{key}/nested/deep"),
            "--client",
            "opencode",
            "--paths",
            "share",
        ])
        .output()
        .expect("run");
    let text = channels(&share);
    assert!(
        !text.contains(key),
        "share-mode text output must not expose an unknown key name: {text}"
    );
}

#[test]
fn an_unrecognised_session_argument_is_never_echoed() {
    // A client argument can itself be a credential (`--api-key=…`); it must never be echoed into a
    // diagnostic. Spec §7 forbids exposing command arguments in reports.
    let sandbox = Sandbox::new(
        &[],
        &[(".claude/settings.json", r#"{"model": "user-model"}"#)],
    );
    let secret_arg = "SEEDED-ARG-3f7a2b";
    let context = format!(
        r#"{{"schema_version":"1.0","client":"claude","client_arguments":["--api-key","{secret_arg}"]}}"#
    );
    let context_path = sandbox.project.join("ctx.json");
    std::fs::write(&context_path, context).expect("write");
    for format in [Vec::<&str>::new(), vec!["--format", "json"]] {
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
            ])
            .args(&format)
            .output()
            .expect("run");
        let text = channels(&output);
        assert!(
            !text.contains(secret_arg),
            "a session argument must never be echoed: {text}"
        );
        if !format.is_empty() {
            assert!(
                text.contains("unrecognised_session_flag"),
                "the gap must still be reported: {text}"
            );
        }
    }
}

#[test]
fn a_url_value_cannot_smuggle_credentials_past_the_sanitizer() {
    // Regression (fuzz target `hostile_inputs`): the value's own `/` ends the authority early, so
    // the `:pw@host` tail lands in the path and a delimiter-only parse republished it. No channel
    // may carry the userinfo marker, in local or share mode.
    let body = serde_json::json!({
        "model": "m",
        "mcp": {
            "srv": {
                "type": "remote",
                "url": "https://ZUINFOuser:passtool_version.ex{/b:pw@host.example/mcp?token=ZUINFO#ZUINFO"
            }
        }
    })
    .to_string();
    let sandbox = Sandbox::new(&[], &[(".config/opencode/opencode.json", &body)]);
    for mode in [Vec::<&str>::new(), vec!["--paths", "share"]] {
        for format in [Vec::<&str>::new(), vec!["--format", "json"]] {
            for command in ["scan", "explain"] {
                let mut args = vec![command];
                if command == "explain" {
                    args.push("/mcp/srv/url");
                }
                args.push("--client");
                args.push("opencode");
                let output = sandbox
                    .command()
                    .args(&args)
                    .args(&mode)
                    .args(&format)
                    .output()
                    .expect("run");
                let text = channels(&output);
                assert!(
                    !text.contains("ZUINFO") && !text.contains("user:pass"),
                    "URL userinfo leaked for {command} {mode:?} {format:?}: {text}"
                );
            }
        }
    }
}

#[test]
fn the_saved_hostile_inputs_crash_artifact_is_neutralised() {
    // Regression (fuzz target `hostile_inputs`, session 2): this exact fuzz unit tripped
    // `hostile_inputs.rs` with "URL userinfo/query/fragment leaked into the Local JSON report".
    // The value's own `/` ended the URL authority early, so `user:pw@host` landed in the path and a
    // delimiter-only parse republished it. The unit is kept as committed evidence under
    // `fuzz/results/` (crash artifacts in `fuzz/artifacts/` are gitignored) and replayed here
    // through the real CLI for both clients, both path modes and both formats, so it stays covered
    // after the corpus is pruned.
    let artifact = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fuzz/results/hostile_inputs-crash-6ac24e86f9105163905201b031b3bc1ae4ddb070");
    let bytes =
        std::fs::read(&artifact).unwrap_or_else(|e| panic!("read {}: {e}", artifact.display()));
    let payload = String::from_utf8_lossy(&bytes);
    // Mirrors the template in `fuzz/fuzz_targets/hostile_inputs.rs`: the marker sits in the userinfo,
    // the query and the fragment, wrapping the arbitrary value.
    let url = format!(
        "https://ZUINFO{payload}:pw@host.example/mcp?token=ZUINFO{payload}#ZUINFO{payload}"
    );
    let opencode = serde_json::json!({
        "model": "m",
        "mcp": { "srv": { "type": "remote", "url": url } }
    })
    .to_string();
    let claude = serde_json::json!({
        "model": "m",
        "mcpServers": { "srv": { "url": url } }
    })
    .to_string();
    let sandbox = Sandbox::new(
        &[(".claude/settings.json", &claude)],
        &[(".config/opencode/opencode.json", &opencode)],
    );
    for client in ["claude", "opencode"] {
        for mode in [Vec::<&str>::new(), vec!["--paths", "share"]] {
            for format in [Vec::<&str>::new(), vec!["--format", "json"]] {
                let output = sandbox
                    .command()
                    .args(["scan", "--client", client])
                    .args(&mode)
                    .args(&format)
                    .output()
                    .expect("run");
                let report = channels(&output);
                for needle in ["ZUINFO", "user:pass", "passtool_version"] {
                    assert!(
                        !report.contains(needle),
                        "the saved crash artifact leaked {needle:?} for {client} {mode:?} {format:?}: {report}"
                    );
                }
            }
        }
    }
}

#[test]
fn support_report_is_share_sanitized_and_preview_writes_nothing() {
    let sandbox = seeded_sandbox();
    // Without an explicit output or preview, support-report is a usage error.
    sandbox
        .command()
        .args(["support-report", "--client", "opencode"])
        .assert()
        .code(2);

    // Preview: sanitized JSON on stdout, nothing written anywhere.
    let before_entries = std::fs::read_dir(&sandbox.project)
        .expect("read dir")
        .count();
    let preview = sandbox
        .command()
        .args(["support-report", "--client", "opencode", "--preview"])
        .output()
        .expect("run");
    assert!(preview.status.success());
    let text = channels(&preview);
    assert!(!text.contains(SEED_VALUE));
    assert!(!text.contains("user:pass"));
    assert!(
        !text.contains("jira.example"),
        "preview must be share-sanitized"
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(preview.stdout).expect("utf8")).expect("json");
    assert_eq!(parsed["redaction"]["mode"], "share");
    assert_eq!(
        std::fs::read_dir(&sandbox.project)
            .expect("read dir")
            .count(),
        before_entries,
        "preview must not create files in the project"
    );

    // Explicit output: the file is written and is share-sanitized.
    let target = sandbox.project.join("support.json");
    sandbox
        .command()
        .args([
            "support-report",
            "--client",
            "opencode",
            "--output",
            target.to_str().expect("path"),
        ])
        .assert()
        .success();
    let written = std::fs::read_to_string(&target).expect("read");
    assert!(!written.contains(SEED_VALUE));
    assert!(!written.contains("jira.example"));
    let parsed: serde_json::Value = serde_json::from_str(&written).expect("json");
    assert_eq!(parsed["redaction"]["mode"], "share");
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
