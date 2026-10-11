//! Resolution benchmarks — `cargo bench`.
//!
//! Dependency-free on purpose: the crate forbids unnecessary dependencies, so this is a plain
//! binary (`harness = false`) that times the engine over synthetic in-memory documents and prints
//! a table. Numbers are indicative, not a statistical benchmark: they exist so that a review can
//! see the shape of the cost (bytes in, layers, output) and so that a regression is visible.
//!
//! Run: `cargo bench` (release profile). Nothing here reads a real configuration file or the
//! process environment, and nothing is written to disk.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use configlens::adapters::{adapter_for, Client};
use configlens::context::{FailOn, Os, ResolutionContext, Trust, VersionContext};
use configlens::filesystem::MemoryFileAccess;
use configlens::parse::Limits;
use configlens::resolve::Engine;
use configlens::security::{PathMode, PathPolicy};

const GLOBAL: &str = "/home/synthetic/.config/opencode/opencode.json";
const PROJECT: &str = "/home/synthetic/work/opencode.json";

fn synthetic_ctx() -> ResolutionContext {
    let mut ctx = ResolutionContext::new(
        "opencode",
        "/home/synthetic/work",
        "/home/synthetic",
        "/home/synthetic/.config",
    );
    ctx.os = Os::Linux;
    ctx.client_version = VersionContext::Claimed("1.18.35".to_string());
    ctx.trust = Trust::Unknown;
    ctx.paths = PathMode::Local;
    ctx.limits = Limits::default();
    ctx.fail_on = FailOn::Error;
    ctx
}

fn policy(ctx: &ResolutionContext) -> PathPolicy {
    PathPolicy::new(
        &ctx.home,
        &ctx.xdg_config,
        ctx.project_root.as_deref(),
        &[std::path::PathBuf::from("/etc/opencode")],
        &ctx.approved_roots,
    )
}

/// A document with `keys` top-level keys plus a nested object, all with plausible values.
fn document(keys: usize) -> String {
    let mut body = String::from("{\"model\": \"synthetic-model\",");
    for index in 0..keys {
        body.push_str(&format!("\"key_{index:05}\": \"value-{index}\","));
    }
    body.push_str("\"mcp\": {");
    for index in 0..16 {
        body.push_str(&format!(
            "\"server_{index}\": {{\"type\": \"local\", \"command\": [\"synthetic\"]}}"
        ));
        if index < 15 {
            body.push(',');
        }
    }
    body.push_str("}}");
    body
}

/// Median wall-clock time of `iterations` engine runs, in milliseconds.
fn time_engine(keys: usize, iterations: usize) -> (f64, usize) {
    let global = document(keys);
    let project = String::from(
        "{\"model\": \"project-model\", \"mcp\": {\"server_0\": {\"enabled\": true}}}",
    );
    let ctx = synthetic_ctx();
    let access = MemoryFileAccess::new()
        .with_file(GLOBAL, &global)
        .with_file(PROJECT, &project);
    let paths = policy(&ctx);
    let adapter = adapter_for(Client::OpenCode);

    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        let output = Engine::new(adapter.as_ref(), &ctx, &access, &paths)
            .run()
            .expect("synthetic run must not fail");
        let resolution = output.explain("/model");
        assert!(
            matches!(
                resolution.status,
                configlens::resolve::ResolutionStatus::Partial
                    | configlens::resolve::ResolutionStatus::Resolved
            ),
            "the synthetic pointer must resolve, got {:?}",
            resolution.status
        );
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    (median.as_secs_f64() * 1_000.0, global.len() + project.len())
}

fn main() {
    println!("ConfigLens benchmarks (release profile)");
    println!(
        "{:>8}  {:>10}  {:>12}  {:>14}",
        "keys", "doc bytes", "median ms/op", "runs/sec (est)"
    );
    for keys in [0usize, 100, 1_000, 5_000] {
        // Warm up once, then take the median of a bounded number of runs.
        let iterations = if keys > 1_000 { 9 } else { 21 };
        let _ = time_engine(keys, 1);
        let (median_ms, bytes) = time_engine(keys, iterations);
        let per_second = if median_ms > 0.0 {
            1_000.0 / median_ms
        } else {
            f64::INFINITY
        };
        println!("{keys:>8}  {bytes:>10}  {median_ms:>12.3}  {per_second:>14.0}");
    }
    let _ = Duration::from_millis(0);
    println!(
        "\nEach operation is one full pass: parse both layers, build the trace, project the value."
    );
}
