//! Narrow CLI audit regressions, using isolated synthetic homes only.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::Sandbox;

#[test]
fn programdata_capture_respects_no_env() {
    use clap::Parser;
    use configlens::adapters::Client;
    use configlens::cli::{prepare, Cli, Command};

    const CHILD: &str = "CONFIGLENS_AUDIT_PROGRAMDATA_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let root = std::env::var("HOME").unwrap();
        for no_env in [true, false] {
            let mut argv = vec![
                "configlens",
                "scan",
                "--client",
                "opencode",
                "--cwd",
                &root,
                "--project-root",
                &root,
            ];
            if no_env {
                argv.push("--no-env");
            }
            let cli = Cli::parse_from(argv);
            let Command::Scan(common) = cli.command else {
                panic!("expected scan");
            };
            let context = prepare(Client::OpenCode, &common).unwrap().context;
            assert_eq!(
                context.program_data.is_some(),
                !no_env,
                "PROGRAMDATA capture must honor the environment policy"
            );
            if !no_env {
                assert!(context.program_data.as_deref() == Some(std::path::Path::new(&root)));
            }
        }
        return;
    }
    let temp = tempfile::TempDir::new().unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "programdata_capture_respects_no_env"])
        .env_clear()
        .env(CHILD, "1")
        .env("HOME", temp.path())
        .env("USERPROFILE", temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join(".config"))
        .env("PROGRAMDATA", temp.path())
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "isolated PROGRAMDATA policy regression failed"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn ordinary_stdout_failures_are_file_problems_not_panics() {
    use std::process::{Command, Stdio};
    let sandbox = Sandbox::new(&[], &[]);
    let output = Command::new(env!("CARGO_BIN_EXE_configlens"))
        .env_clear()
        .env("HOME", &sandbox.home)
        .env("XDG_CONFIG_HOME", sandbox.home.join(".config"))
        .current_dir(&sandbox.project)
        .arg("schema")
        .stdout(Stdio::from(
            std::fs::File::options()
                .write(true)
                .open("/dev/full")
                .expect("full device"),
        ))
        .stderr(Stdio::piped())
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("output_unwritable"));
    assert!(!stderr.contains("internal error"));
}

#[test]
fn explicit_input_boundary_and_error_cases_preserve_the_exit_contract() {
    let sandbox = Sandbox::new(&[], &[]);
    let limit = configlens::parse::Limits::default().max_bytes;
    std::fs::write(
        sandbox.path("exact.json"),
        format!("{}{{}}", " ".repeat(limit - 2)),
    )
    .expect("write");
    sandbox
        .command()
        .args([
            "scan",
            "--client",
            "claude",
            "--cwd",
            ".",
            "--input-context",
            "exact.json",
        ])
        .assert()
        .success();
    std::fs::write(sandbox.path("binary.json"), [0xff]).expect("write");
    for (name, code) in [
        ("missing.json", "report_unreadable"),
        (".", "not_a_regular_file"),
        ("binary.json", "not_utf8"),
    ] {
        let output = sandbox
            .command()
            .args(["diff", name, name])
            .output()
            .expect("run");
        assert_eq!(output.status.code(), Some(4));
        assert!(String::from_utf8_lossy(&output.stderr).contains(code));
    }
    std::fs::write(sandbox.dir.path().join("context.json"), "{}").expect("write");
    sandbox
        .command()
        .args([
            "scan",
            "--client",
            "claude",
            "--cwd",
            ".",
            "--input-context",
            "../context.json",
        ])
        .assert()
        .success();
}

#[cfg(unix)]
#[test]
fn explicit_inputs_refuse_sockets_and_escaping_links_but_allow_local_links() {
    let sandbox = Sandbox::new(&[], &[]);
    let _socket = std::os::unix::net::UnixListener::bind(sandbox.path("socket")).expect("socket");
    std::fs::write(sandbox.dir.path().join("outside.json"), "{}").expect("write");
    std::os::unix::fs::symlink(
        sandbox.dir.path().join("outside.json"),
        sandbox.path("escape.json"),
    )
    .expect("link");
    std::fs::write(sandbox.path("context.json"), "{}").expect("write");
    std::os::unix::fs::symlink(sandbox.path("context.json"), sandbox.path("local.json"))
        .expect("link");
    for (name, code) in [
        ("socket", "not_a_regular_file"),
        ("escape.json", "symlink_escape"),
    ] {
        for args in [
            vec!["diff", name, name],
            vec![
                "scan",
                "--client",
                "claude",
                "--cwd",
                ".",
                "--input-context",
                name,
            ],
        ] {
            let output = sandbox.command().args(args).output().expect("run");
            assert_eq!(output.status.code(), Some(4));
            assert!(String::from_utf8_lossy(&output.stderr).contains(code));
        }
    }
    sandbox
        .command()
        .args([
            "scan",
            "--client",
            "claude",
            "--cwd",
            ".",
            "--input-context",
            "local.json",
        ])
        .assert()
        .success();
}

#[test]
fn argument_parser_errors_do_not_echo_untrusted_values() {
    let sandbox = Sandbox::new(&[], &[]);
    let seed = "SYNTHETIC_ARG_SECRET_91";
    for args in [
        vec!["clients", "--format", seed],
        vec![seed],
        vec!["scan", "--client", "claude", "--unexpected", seed],
    ] {
        let output = sandbox.command().args(args).output().expect("run");
        assert_eq!(output.status.code(), Some(2));
        let channels = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !channels.contains(seed),
            "argument value leaked: {channels}"
        );
    }
}

#[cfg(unix)]
#[test]
fn closed_stdout_is_a_successful_pipe_termination_not_a_panic() {
    use std::os::fd::OwnedFd;
    use std::process::{Command, Stdio};
    let sandbox = Sandbox::new(&[], &[]);
    for args in [
        vec!["schema"],
        vec!["clients"],
        vec!["clients", "--format", "json"],
        vec!["completions", "--shell", "bash"],
        vec!["--help"],
        vec!["--version"],
        vec!["scan", "--client", "claude", "--cwd", "."],
        vec!["doctor", "--client", "claude", "--cwd", "."],
        vec!["explain", "/model", "--client", "claude", "--cwd", "."],
        vec![
            "support-report",
            "--client",
            "claude",
            "--cwd",
            ".",
            "--preview",
            "--quiet",
        ],
    ] {
        let (writer, reader) = std::os::unix::net::UnixStream::pair().expect("socket pair");
        drop(reader);
        let output = Command::new(env!("CARGO_BIN_EXE_configlens"))
            .env_clear()
            .env("HOME", &sandbox.home)
            .env("XDG_CONFIG_HOME", sandbox.home.join(".config"))
            .current_dir(&sandbox.project)
            .args(&args)
            .stdout(Stdio::from(OwnedFd::from(writer)))
            .stderr(Stdio::piped())
            .output()
            .expect("run");
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert!(
            output.stderr.is_empty(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn context_validation_errors_never_echo_sensitive_input() {
    let sandbox = Sandbox::new(&[], &[]);
    let seed = "SYNTHETIC_CONTEXT_SECRET_83";
    for input in [
        format!(r#"{{"project_trust":42,"schema_version":"{seed}"}}"#),
        format!(r#"{{"schema_version":"{seed}"}}"#),
        format!(r#"{{"project_trust":"{seed}"}}"#),
        format!(r#"{{"sources":[{{"role":"managed","path":"{seed}","content":false}}]}}"#),
        format!(r#"{{"environment":{{"ANY":"{seed}"}},"client_arguments":"{seed}"}}"#),
    ] {
        std::fs::write(sandbox.path("context.json"), input).expect("write");
        let output = sandbox
            .command()
            .args([
                "scan",
                "--client",
                "claude",
                "--cwd",
                ".",
                "--input-context",
                "context.json",
            ])
            .output()
            .expect("run");
        assert_eq!(output.status.code(), Some(2));
        let channels = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !channels.contains(seed),
            "context content leaked: {channels}"
        );
    }
}

#[test]
fn snapshot_with_2500_keys_larger_than_context_limit_can_be_diffed() {
    let settings: serde_json::Map<String, serde_json::Value> = (0..2500)
        .map(|i| {
            (
                format!("synthetic_setting_with_long_name_{i:04}"),
                serde_json::json!(i),
            )
        })
        .collect();
    let config = serde_json::to_string(&settings).expect("serialize settings");
    assert!(config.len() < configlens::parse::Limits::default().max_bytes);
    let sandbox = Sandbox::new(&[], &[(".claude/settings.json", &config)]);
    sandbox
        .command()
        .args([
            "snapshot",
            "--client",
            "claude",
            "--client-version",
            "2.1.296",
            "--cwd",
            ".",
            "--output",
            "snapshot.json",
        ])
        .assert()
        .success();
    let bytes = std::fs::metadata(sandbox.path("snapshot.json"))
        .expect("metadata")
        .len();
    assert!(
        bytes > configlens::parse::Limits::default().max_bytes as u64,
        "snapshot: {bytes} bytes"
    );
    sandbox
        .command()
        .args(["diff", "snapshot.json", "snapshot.json", "--format", "json"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"identical\": true"));
}

#[test]
fn diff_accepts_a_report_at_the_exact_serialized_limit() {
    let sandbox = Sandbox::new(&[], &[]);
    let envelope = serde_json::json!({
        "schema_version": configlens::REPORT_SCHEMA_VERSION,
        "client": "claude", "resolutions": []
    })
    .to_string();
    let content = format!(
        "{}{}",
        envelope,
        " ".repeat(configlens::commands::MAX_REPORT_BYTES - envelope.len())
    );
    configlens::commands::write_snapshot(&sandbox.path("exact-report.json"), &content, false)
        .expect("write exact-limit snapshot");
    sandbox
        .command()
        .args([
            "diff",
            "exact-report.json",
            "exact-report.json",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"identical\": true"));
}

#[test]
fn explicit_input_files_are_bounded_before_parsing() {
    let sandbox = Sandbox::new(&[], &[]);
    let limit = configlens::parse::Limits::default().max_bytes;
    assert_eq!(limit, 2 * 1024 * 1024, "contexts retain the 2 MiB cap");
    let padded_json = format!("{}{{}}", " ".repeat(limit - 1));
    std::fs::write(sandbox.path("large.json"), padded_json).expect("write");
    let report = std::fs::File::create(sandbox.path("large-report.json")).expect("create");
    report
        .set_len(configlens::commands::MAX_REPORT_BYTES as u64 + 1)
        .expect("size");
    for args in [
        vec!["diff", "large-report.json", "large-report.json"],
        vec![
            "scan",
            "--client",
            "claude",
            "--cwd",
            ".",
            "--input-context",
            "large.json",
        ],
    ] {
        let output = sandbox.command().args(args).output().expect("run");
        assert_eq!(output.status.code(), Some(4));
        assert!(String::from_utf8_lossy(&output.stderr).contains("resource_limit_exceeded"));
    }
}
