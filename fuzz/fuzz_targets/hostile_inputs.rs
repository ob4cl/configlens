//! Fuzz target: potentially hostile inputs, and the promise that nothing sensitive leaks.
//!
//! Run: `cargo +nightly fuzz run hostile_inputs`
//!
//! The product promise is "reports are safe to paste into an issue". That promise is only worth
//! anything if it holds for input written by an adversary, so this target builds configuration
//! documents out of arbitrary attacker-controlled bytes and asserts the report contract for every
//! channel it can render:
//!
//! 1. **No value leak.** Text injected under a sensitive field (`env` values, `token`, `secret`,
//!    `apiKey`, MCP headers, `password`) or under a value position that no adapter allowlists must
//!    never appear in the JSON report — in *either* path mode — as raw text or as its JSON escaping.
//! 2. **No credential leak from URLs.** URL userinfo, query and fragment must not survive into any
//!    channel; only the sanitized URL may appear.
//! 3. **No unknown-key leak in share mode.** `--paths share` replaces unknown object key names with
//!    opaque IDs, so an attacker-chosen key name must not appear in a share-mode report. (Local mode
//!    deliberately shows key names for diagnosis, so it is not asserted there.)
//! 4. **No terminal control survives.** Neither the report JSON nor the rendered text may contain
//!    ESC (terminal escape/OSC), bidi override, or zero-width characters — spec §7 requires them to
//!    be stripped from every user-derived string, including key names and pointer displays. The
//!    JSON is inspected *decoded*: a serialized ESC (`\u001b`) and an input that literally carried
//!    the text `\u001b` are indistinguishable as substrings, and bidi/zero-width characters are
//!    written as raw UTF-8 rather than escaped, so the escaped form is not a sound witness.
//! 5. **No panic**, for any byte sequence, in either client's pipeline.

#![no_main]

use std::path::PathBuf;

use libfuzzer_sys::fuzz_target;
use serde_json::json;

use configlens::adapters::{adapter_for, Client};
use configlens::context::{FailOn, Os, ResolutionContext, Trust, VersionContext};
use configlens::filesystem::MemoryFileAccess;
use configlens::model::report::{build, to_json, Compatibility, ReportInput};
use configlens::parse::Limits;
use configlens::render::text::render_scan;
use configlens::resolve::Engine;
use configlens::security::{PathMode, PathPolicy};

/// Markers are uppercase/high-entropy on purpose: an allowlisted model name is lower-case only, and
/// a plain string cannot contain `=` or a space, so if one of these appears the value was published
/// by a bug rather than by policy.
const MARKER_VALUE: &str = "ZSENTINELV";
/// Injected into URL userinfo/query/fragment, which must never be reproduced.
const MARKER_USERINFO: &str = "ZUINFO";
/// Injected as an object *key name*; must be opaque in share mode.
const MARKER_KEY: &str = "ZKEY";
/// Injected inside a Claude permission rule. Local mode deliberately shows the rule (it is the
/// subject of the question), so only share mode, which maps rules to `rule_N`, may not leak it.
const MARKER_RULE: &str = "ZRULE";

const CLAUDE_PROJECT: &str = "/home/fuzz/work/.claude/settings.json";
const OC_GLOBAL: &str = "/home/fuzz/.config/opencode/opencode.json";

const HOSTILE_CHARS: &[&str] = &[
    "\u{1b}[31m",                          // ANSI CSI colour
    "\u{1b}]0;t\u{7}",                     // OSC window title (BEL-terminated)
    "\u{1b}]8;;https://e.example\u{1b}\\", // OSC 8 hyperlink
    "\u{202e}",                            // right-to-left override
    "\u{200b}",                            // zero-width space
    "\u{feff}",                            // BOM / zero-width no-break space
    "\n",                                  // untrusted newline
    "=",                                   // breaks "plain string" shape
];

fn claude_document(text: &str, hostile: &str) -> String {
    let marked = format!("{MARKER_VALUE}{text}");
    json!({
        // `/model` and `/fallbackModel` are the only model-name pointers: an upper-case token must
        // not be promoted to a public "verified model name".
        "model": marked.clone(),
        "fallbackModel": marked.clone(),
        // Credential-looking names, in and out of the redacted `env` subtree.
        "apiKey": marked.clone(),
        "token": marked.clone(),
        "secret": marked.clone(),
        "password": marked.clone(),
        "env": { "JOB_TOKEN": marked.clone() },
        // `/mcpServers/**` is a redacted subtree in the Claude registry.
        "mcpServers": {
            "srv": {
                "env": { "AUTHORIZATION": marked.clone() },
                "headers": { "Authorization": marked.clone() },
                "url": format!("https://{MARKER_USERINFO}{text}:pw@host.example/mcp?token={MARKER_USERINFO}{text}#{MARKER_USERINFO}{text}")
            }
        },
        "permissions": { "allow": [format!("Bash({MARKER_RULE}{text})")] },
        // An attacker-chosen *key name*, including hostile control characters.
        format!("{MARKER_KEY}{hostile}{text}"): "value-under-an-overly-clever-key"
    })
    .to_string()
}

fn opencode_document(text: &str, hostile: &str) -> String {
    let marked = format!("{MARKER_VALUE}{text}");
    json!({
        "model": marked.clone(),
        "small_model": marked.clone(),
        // `/mcp/*/url` is the allowlisted public-URL pointer: sanitize_url must drop userinfo,
        // query and fragment in both modes.
        "mcp": {
            "srv": {
                "type": "remote",
                "url": format!("https://{MARKER_USERINFO}{text}:pw@host.example/mcp?token={MARKER_USERINFO}{text}#{MARKER_USERINFO}{text}"),
                "headers": { "Authorization": marked.clone() },
                "environment": { "TOKEN": marked.clone() }
            }
        },
        format!("{MARKER_KEY}{hostile}{text}"): marked.clone()
    })
    .to_string()
}

fn context(client: Client, mode: PathMode) -> ResolutionContext {
    let mut ctx = ResolutionContext::new(
        client.id(),
        "/home/fuzz/work",
        "/home/fuzz",
        "/home/fuzz/.config",
    );
    ctx.os = Os::Linux;
    ctx.project_root = Some(PathBuf::from("/home/fuzz/work"));
    ctx.client_version = match client {
        Client::OpenCode => VersionContext::Claimed("1.18.35".to_string()),
        Client::Claude => VersionContext::Claimed("2.1.296".to_string()),
    };
    ctx.trust = Trust::Unknown;
    ctx.paths = mode;
    ctx.limits = Limits::default();
    ctx.fail_on = FailOn::Error;
    ctx.approved_roots = vec![PathBuf::from("/home/fuzz")];
    ctx
}

fn policy(ctx: &ResolutionContext) -> PathPolicy {
    PathPolicy::new(
        &ctx.home,
        &ctx.xdg_config,
        ctx.project_root.as_deref(),
        &[
            PathBuf::from("/etc/claude-code"),
            PathBuf::from("/etc/opencode"),
        ],
        &ctx.approved_roots,
    )
}

fn report_json(
    output: &configlens::resolve::RunOutput,
    ctx: &ResolutionContext,
    policy: &PathPolicy,
    mode: PathMode,
) -> String {
    let report = build(
        ReportInput {
            command: "scan",
            client: output.client.clone(),
            adapter_revision: "0.1.0".to_string(),
            compatibility: Compatibility {
                conformance_status: "fixture_verified".to_string(),
                tested_versions: Vec::new(),
                documented_versions: Vec::new(),
                pinned_commit: None,
                evidence: Vec::new(),
                provisional: true,
            },
            context: ctx.clone(),
            sources: &output.sources,
            resolutions: &output.resolutions,
            diagnostics: &output.diagnostics,
            coverage: &output.coverage,
            paths: policy,
            errors: Vec::new(),
        },
        mode,
    );
    to_json(&report).expect("a report must serialize")
}

/// Every occurrence of `needle` in `haystack`, including its JSON-escaped form.
fn escaped_forms(needle: &str) -> Vec<String> {
    let quoted = serde_json::to_string(needle).expect("a string must serialize");
    vec![needle.to_string(), quoted.trim_matches('"').to_string()]
}

fn assert_absent(channel: &str, haystack: &str, needle: &str, mode: PathMode, what: &str) {
    for form in escaped_forms(needle) {
        assert!(
            !haystack.contains(&form),
            "{what} leaked into the {mode:?} {channel}: the {form:?} form of the injected text is present"
        );
    }
}

/// Remove any sentinel token the arbitrary input might itself carry.
///
/// `text` is placed verbatim into contexts the report is *meant* to show in Local mode (unknown key
/// names, permission rules). If the input happens to spell a marker, the absence checks below would
/// fire on a report that is perfectly safe, so the sentinels are stripped out of the input first: a
/// marker can then only appear in a channel by being published by the code under test.
fn strip_sentinels(mut text: String) -> String {
    for marker in [MARKER_VALUE, MARKER_USERINFO, MARKER_KEY, MARKER_RULE] {
        if text.contains(marker) {
            text = text.replace(marker, "");
        }
    }
    text
}

/// Assert that no *decoded* string in `json_report` carries a terminal control character.
///
/// The serialized JSON is not a sound witness on its own: `serde_json` escapes a real ESC as the six
/// characters `\u001b`, which a substring search cannot tell apart from input that literally
/// contained `\u001b` (written `\\u001b`), while bidi and zero-width characters are written as raw
/// UTF-8 and never match an escaped-literal search at all. Parsing first distinguishes a decoded
/// control (a genuine leak) from an escaped literal (harmless input text).
fn assert_no_control_chars(json_report: &str, mode: PathMode) {
    const CONTROLS: [(&str, char); 4] = [
        ("ESC", '\u{1b}'),
        ("bidi override", '\u{202e}'),
        ("zero-width space", '\u{200b}'),
        ("no-break BOM", '\u{feff}'),
    ];
    let parsed: serde_json::Value =
        serde_json::from_str(json_report).expect("the JSON report must parse");
    let mut stack: Vec<&serde_json::Value> = vec![&parsed];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::String(s) => {
                for (name, control) in CONTROLS {
                    assert!(
                        !s.contains(control),
                        "an unsanitized {name} reached the {mode:?} JSON report"
                    );
                }
            }
            serde_json::Value::Array(items) => stack.extend(items.iter()),
            serde_json::Value::Object(map) => {
                for (key, item) in map {
                    for (name, control) in CONTROLS {
                        assert!(
                            !key.contains(control),
                            "an unsanitized {name} reached the {mode:?} JSON report (object key)"
                        );
                    }
                    stack.push(item);
                }
            }
            _ => {}
        }
    }
}

fn check_document(client: Client, document: &str) {
    for mode in [PathMode::Local, PathMode::Share] {
        let ctx = context(client, mode);
        let policy = policy(&ctx);
        let access = match client {
            Client::Claude => MemoryFileAccess::new().with_file(CLAUDE_PROJECT, document),
            Client::OpenCode => MemoryFileAccess::new()
                .with_file(OC_GLOBAL, document)
                .with_file("/home/fuzz/work/opencode.json", document),
        };
        let adapter = adapter_for(client);
        let Ok(output) = Engine::new(adapter.as_ref(), &ctx, &access, &policy).run() else {
            continue;
        };

        let json_report = report_json(&output, &ctx, &policy, mode);
        assert_absent(
            "JSON report",
            &json_report,
            MARKER_VALUE,
            mode,
            "a sensitive value",
        );
        assert_absent(
            "JSON report",
            &json_report,
            MARKER_USERINFO,
            mode,
            "URL userinfo/query/fragment",
        );

        let text_report = render_scan(&output);
        assert_absent(
            "text report",
            &text_report,
            MARKER_VALUE,
            mode,
            "a sensitive value",
        );
        assert_absent(
            "text report",
            &text_report,
            MARKER_USERINFO,
            mode,
            "URL userinfo/query/fragment",
        );

        // Share mode additionally pseudonymizes unknown key names, and maps permission rules to
        // opaque `rule_N` IDs.
        if mode == PathMode::Share {
            assert_absent(
                "JSON report",
                &json_report,
                MARKER_KEY,
                mode,
                "an unknown object key name",
            );
            assert_absent(
                "text report",
                &text_report,
                MARKER_KEY,
                mode,
                "an unknown object key name",
            );
            assert_absent(
                "JSON report",
                &json_report,
                MARKER_RULE,
                mode,
                "a local-mode permission rule",
            );
            assert_absent(
                "text report",
                &text_report,
                MARKER_RULE,
                mode,
                "a local-mode permission rule",
            );
            assert!(
                !json_report.contains("/home/fuzz"),
                "share mode published an absolute home path"
            );
        }

        // Terminal control characters must not survive into any channel, in any mode. The JSON is
        // checked after parsing (see `assert_no_control_chars`); the text report is checked for the
        // decoded characters directly.
        assert_no_control_chars(&json_report, mode);
        assert!(
            !text_report.contains('\u{1b}')
                && !text_report.contains('\u{202e}')
                && !text_report.contains('\u{200b}')
                && !text_report.contains('\u{feff}'),
            "an unsanitized control character reached the {mode:?} text report"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    // The read layer refuses non-UTF-8 files, so the attacker-controlled string here is the lossy
    // form of arbitrary bytes: valid UTF-8 plus U+FFFD, which is what a hostile writer could ship.
    let text = String::from_utf8_lossy(data);
    let hostile = HOSTILE_CHARS[data.len() % HOSTILE_CHARS.len()];

    // Long inputs are truncated to keep a single run's cost bounded; the parser limit is exercised
    // by `parse_documents`, and here the interesting axis is *content*, not size. Sentinels the
    // input happens to spell are removed so a marker can only arrive by being published.
    let text: String = strip_sentinels(text.chars().take(512).collect());

    check_document(Client::Claude, &claude_document(&text, hostile));
    check_document(Client::OpenCode, &opencode_document(&text, hostile));
});
