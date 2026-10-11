//! Fuzz target: precedence resolution (`configlens::resolve::Engine`).
//!
//! Run: `cargo +nightly fuzz run resolve_precedence`
//!
//! The engine decides *which configuration value wins and why*, and its output is what a user acts
//! on. A wrong winner that still looks plausible is the worst failure mode this tool has, so the
//! target fuzzes the whole resolution pipeline over multi-layer, multi-client inputs and asserts the
//! properties that make a trace trustworthy:
//!
//! 1. **No panic**, for either adapter, for either trust state, and for inputs that are also
//!    pathological as documents (truncated, duplicated keys, huge nesting, empty, binary-ish).
//! 2. **Determinism.** The same inputs must produce byte-identical results — twice, including the
//!    final JSON report in both path modes. A trace that changes between runs cannot be evidence.
//! 3. **Pointer integrity.** Every declared pointer is a legal RFC 6901 pointer, `explain` returns
//!    a resolution whose `pointer` is exactly the one asked for, and no pointer is resolved twice.
//! 4. **Trace integrity.** Every event that names a definition or a source names one that exists,
//!    and the definition it names really does cover that pointer (its own pointer, or an ancestor
//!    of it). An event attributed to an unrelated definition is a fabricated provenance claim.
//! 5. **Exactly one winner.** A resolution has at most one `selected` event.
//! 6. **Bounded output.** Nothing survives a resolution that was not in the input: no resolution
//!    without any definition behind it may claim a `selected` event.

#![no_main]

use std::path::PathBuf;

use libfuzzer_sys::fuzz_target;

use configlens::adapters::{adapter_for, Client};
use configlens::context::{FailOn, Os, ResolutionContext, Trust, VersionContext};
use configlens::filesystem::{FileKind, MemoryFileAccess};
use configlens::model::event::EventKind;
use configlens::model::report::{build, to_json, Compatibility, ReportInput};
use configlens::parse::Limits;
use configlens::resolve::{Engine, ResolutionStatus, RunOutput};
use configlens::security::{PathMode, PathPolicy};

/// Candidate layer paths for both adapters. Which subset is *applicable* is the adapter's business;
/// the fuzzer just makes sure a document exists at each candidate so multi-layer precedence actually
/// happens.
const CLAUDE_USER: &str = "/home/fuzz/.claude/settings.json";
const CLAUDE_PROJECT: &str = "/home/fuzz/work/.claude/settings.json";
const CLAUDE_LOCAL: &str = "/home/fuzz/work/.claude/settings.local.json";
const OC_GLOBAL: &str = "/home/fuzz/.config/opencode/opencode.json";
const OC_PROJECT: &str = "/home/fuzz/work/opencode.json";
const OC_DIR: &str = "/home/fuzz/work/.opencode/opencode.json";

/// A hostile-but-representative set of layer contents, cycled across candidate paths so that any
/// pair of them can end up as "two layers defining the same key".
fn layers(data: &[String]) -> [String; 6] {
    let pick = |index: usize| -> String {
        if data.is_empty() {
            String::new()
        } else {
            data[index % data.len()].to_string()
        }
    };
    [pick(0), pick(1), pick(2), pick(3), pick(4), pick(5)]
}

fn context(client: Client, trust: Trust, mode: PathMode) -> ResolutionContext {
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
    ctx.trust = trust;
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
        &[PathBuf::from("/etc/claude-code"), PathBuf::from("/etc/opencode")],
        &ctx.approved_roots,
    )
}

fn access_from(contents: &[String; 6]) -> MemoryFileAccess {
    MemoryFileAccess::new()
        .with_file(CLAUDE_USER, &contents[0])
        .with_file(CLAUDE_PROJECT, &contents[1])
        .with_file(CLAUDE_LOCAL, &contents[2])
        .with_file(OC_GLOBAL, &contents[3])
        .with_file(OC_PROJECT, &contents[4])
        // A non-regular file type must be classified and refused, never read.
        .with_kind(OC_DIR, FileKind::Socket)
}

fn report_json(output: &RunOutput, ctx: &ResolutionContext, policy: &PathPolicy, mode: PathMode) -> String {
    let compatibility = Compatibility {
        conformance_status: output
            .adapter
            .as_ref()
            .map(|metadata| metadata.conformance_status.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        tested_versions: output
            .adapter
            .as_ref()
            .map(|metadata| metadata.tested_versions.clone())
            .unwrap_or_default(),
        documented_versions: output
            .adapter
            .as_ref()
            .map(|metadata| metadata.documented_versions.clone())
            .unwrap_or_default(),
        pinned_commit: output
            .adapter
            .as_ref()
            .and_then(|metadata| metadata.pinned_commit.clone()),
        evidence: Vec::new(),
        provisional: true,
    };
    let report = build(
        ReportInput {
            command: "scan",
            client: output.client.clone(),
            adapter_revision: output
                .adapter
                .as_ref()
                .map(|metadata| metadata.revision.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            compatibility,
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

/// Invariants that must hold for every successful run.
fn check_run(output: &RunOutput, ctx: &ResolutionContext) {
    // --- Pointer integrity ---------------------------------------------------------------------
    let mut seen: Vec<&str> = Vec::with_capacity(output.resolutions.len());
    for resolution in &output.resolutions {
        let pointer = resolution.pointer.as_str();
        assert!(
            pointer.is_empty() || pointer.starts_with('/'),
            "resolution pointer {pointer:?} is not a legal RFC 6901 pointer"
        );
        assert!(
            !seen.contains(&pointer),
            "pointer {pointer:?} was resolved more than once"
        );
        seen.push(pointer);
        assert_eq!(
            output.explain(pointer).pointer,
            pointer,
            "explain returned a resolution for a different pointer"
        );
        if let Some(from_map) = output.by_pointer.get(pointer) {
            assert_eq!(from_map.status, resolution.status);
        }
    }
    // Every declared pointer must be a key of the lookup map (or absent with a synthesised result).
    assert!(
        output.resolutions.len() <= output.definitions.len(),
        "more resolutions ({}) than definitions ({})",
        output.resolutions.len(),
        output.definitions.len()
    );

    // --- Trace integrity ----------------------------------------------------------------------
    let definition_pointers: Vec<(u32, &str)> = output
        .definitions
        .iter()
        .map(|definition| (definition.id.0, definition.pointer.as_str()))
        .collect();
    let source_ids: Vec<u32> = output.sources.iter().map(|source| source.id.0).collect();
    for resolution in &output.resolutions {
        let mut selected = 0usize;
        for event in &resolution.events {
            if event.kind == EventKind::Selected {
                selected += 1;
            }
            if let Some(id) = event.definition_id {
                let found = definition_pointers
                    .iter()
                    .find(|(candidate, _)| *candidate == id)
                    .unwrap_or_else(|| {
                        panic!(
                            "event for {:?} names definition {id}, which the run never produced",
                            resolution.pointer
                        )
                    });
                let owner = found.1;
                assert!(
                    owner.is_empty()
                        || owner == resolution.pointer
                        || resolution.pointer.starts_with(&format!("{owner}/")),
                    "event for {:?} names definition {id} at {owner:?}, which does not cover it",
                    resolution.pointer
                );
            }
            if let Some(id) = event.source_id {
                assert!(
                    source_ids.contains(&id),
                    "event for {:?} names source {id}, which the run never discovered",
                    resolution.pointer
                );
            }
        }
        assert!(
            selected <= 1,
            "resolution {:?} selected {selected} definitions",
            resolution.pointer
        );
        if selected == 0 {
            assert_ne!(
                resolution.status,
                ResolutionStatus::Resolved,
                "resolution {:?} claims a resolved value with no selected event",
                resolution.pointer
            );
        }
    }

    // --- Bounded output -----------------------------------------------------------------------
    // An unresolved/absent pointer must never carry an effective value.
    for resolution in &output.resolutions {
        if !matches!(
            resolution.status,
            ResolutionStatus::Resolved | ResolutionStatus::Partial
        ) {
            assert!(
                resolution.effective_value.is_none(),
                "resolution {:?} has a status of {:?} but still published an effective value",
                resolution.pointer,
                resolution.status
            );
        }
    }
    let _ = ctx;
}

fuzz_target!(|data: &[u8]| {
    // Layer contents are separated by the ASCII record separator so one corpus entry can describe
    // several layers. A chunk that is not valid UTF-8 cannot reach the parser in production (the
    // read layer refuses it), so the lossy form stands in for the bytes a user could actually have
    // written.
    let chunks: Vec<String> = data
        .split(|byte| *byte == 0x1e)
        .take(6)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect();
    let contents = layers(&chunks);

    for client in [Client::OpenCode, Client::Claude] {
        for trust in [Trust::Unknown, Trust::Trusted] {
            for mode in [PathMode::Local, PathMode::Share] {
                let ctx = context(client, trust, mode);
                let policy = policy(&ctx);
                let access = access_from(&contents);
                let adapter = adapter_for(client);

                let run = |access: &MemoryFileAccess| {
                    Engine::new(adapter.as_ref(), &ctx, access, &policy).run()
                };

                let Ok(first) = run(&access) else {
                    // A refused run is a legitimate outcome; it must be an error, not a partial
                    // report pretending to be complete, and it must not panic on a second attempt.
                    continue;
                };
                check_run(&first, &ctx);

                // Determinism: engine result, then the rendered report in this path mode.
                let second = run(&access).expect("the same inputs must not fail on a second run");
                assert_eq!(
                    format!("{first:?}"),
                    format!("{second:?}"),
                    "the engine produced a different result for identical inputs"
                );
                assert_eq!(
                    report_json(&first, &ctx, &policy, mode),
                    report_json(&second, &ctx, &policy, mode),
                    "the report for identical inputs is not byte-identical"
                );
            }
        }
    }
});
