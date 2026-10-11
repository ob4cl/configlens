//! Shared helpers for integration tests. Every test uses a synthetic home and a captured
//! environment: no test ever reads the developer's real configuration.
#![allow(dead_code)] // each integration test crate uses a different subset of these helpers
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

use configlens::adapters::{adapter_for, Client};
use configlens::context::{FailOn, Os, ResolutionContext, Trust, VersionContext};
use configlens::filesystem::{FileAccess, MemoryFileAccess, RealFileAccess};
use configlens::parse::Limits;
use configlens::resolve::{Engine, RunOutput};
use configlens::security::{PathMode, PathPolicy};

/// A context with an isolated synthetic home and no captured environment.
pub fn ctx(client: &str, cwd: &str) -> ResolutionContext {
    let mut ctx = ResolutionContext::new(client, cwd, "/home/synthetic", "/home/synthetic/.config");
    ctx.os = Os::Linux;
    ctx.client_version = VersionContext::Claimed(
        match client {
            "claude" => "2.1.296",
            _ => "1.18.35",
        }
        .to_string(),
    );
    ctx.trust = Trust::Unknown;
    ctx.paths = PathMode::Local;
    ctx.limits = Limits::default();
    ctx.fail_on = FailOn::Error;
    ctx
}

/// A path that is absolute on the host platform. `Path::is_absolute` is platform specific:
/// `/etc/secret` is rooted but NOT absolute on Windows, so a synthetic reference written that
/// way would take a different (though still refusing) code path there.
pub fn host_abs(posix: &str) -> String {
    if cfg!(windows) {
        format!("C:{posix}")
    } else {
        posix.to_string()
    }
}

pub fn with_trust(mut ctx: ResolutionContext, trust: Trust) -> ResolutionContext {
    ctx.trust = trust;
    ctx
}

pub fn with_env(mut ctx: ResolutionContext, pairs: &[(&str, &str)]) -> ResolutionContext {
    let mut env = std::collections::BTreeMap::new();
    for (k, v) in pairs {
        env.insert((*k).to_string(), (*v).to_string());
    }
    ctx.env = Some(env);
    ctx
}

pub fn with_args(mut ctx: ResolutionContext, args: &[&str]) -> ResolutionContext {
    ctx.client_arguments = args.iter().map(|a| (*a).to_string()).collect();
    ctx
}

pub fn mem(files: &[(&str, &str)]) -> MemoryFileAccess {
    let mut access = MemoryFileAccess::new();
    for (path, content) in files {
        access = access.with_file(*path, content);
    }
    access
}

pub fn policy_for(ctx: &ResolutionContext) -> PathPolicy {
    let managed: Vec<std::path::PathBuf> = vec![
        std::path::PathBuf::from("/etc/claude-code"),
        std::path::PathBuf::from("/etc/opencode"),
    ];
    PathPolicy::new(
        &ctx.home,
        &ctx.xdg_config,
        ctx.project_root.as_deref(),
        &managed,
        &ctx.approved_roots,
    )
}

pub fn run_with(client: Client, ctx: &ResolutionContext, access: &dyn FileAccess) -> RunOutput {
    let adapter = adapter_for(client);
    let paths = policy_for(ctx);
    Engine::new(adapter.as_ref(), ctx, access, &paths)
        .run()
        .expect("engine run should not fail fatally")
}

/// Run with an in-memory filesystem.
pub fn run(client: Client, ctx: &ResolutionContext, files: &[(&str, &str)]) -> RunOutput {
    run_with(client, ctx, &mem(files))
}

/// Run against a real temporary directory tree.
pub fn run_real(client: Client, ctx: &ResolutionContext) -> RunOutput {
    run_with(client, ctx, &RealFileAccess)
}

/// Write a synthetic project tree, returning the directory (kept alive by the caller).
pub fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    for (rel, content) in files {
        let path = dir.path().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&path, content).expect("write");
    }
    dir
}

/// An isolated home + project for CLI tests. The process environment is cleared and rebuilt from
/// this sandbox, so no test can read the developer's real configuration.
pub struct Sandbox {
    pub dir: tempfile::TempDir,
    pub home: std::path::PathBuf,
    pub project: std::path::PathBuf,
}

impl Sandbox {
    pub fn new(project_files: &[(&str, &str)], home_files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let home = dir.path().join("home");
        let project = dir.path().join("project");
        std::fs::create_dir_all(&home).expect("mkdir home");
        std::fs::create_dir_all(&project).expect("mkdir project");
        for (rel, content) in project_files {
            let path = project.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            std::fs::write(&path, content).expect("write");
        }
        for (rel, content) in home_files {
            let path = home.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            std::fs::write(&path, content).expect("write");
        }
        Self { dir, home, project }
    }

    /// A command with a cleared environment and this sandbox's home.
    pub fn command(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::cargo_bin("configlens").expect("binary");
        cmd.env_clear();
        cmd.env("HOME", &self.home);
        cmd.env("XDG_CONFIG_HOME", self.home.join(".config"));
        cmd.current_dir(&self.project);
        cmd
    }

    pub fn path(&self, rel: &str) -> std::path::PathBuf {
        self.project.join(rel)
    }
}
