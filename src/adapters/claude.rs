//! Claude Code adapter — bounded, verified settings keys (spec §3, §12 step 6).
//!
//! File settings, actual environment, supplied session flags and managed settings stay distinct:
//! environment variables are *not* a precedence level, they are decided per key/pair.
//! Permission lists are reported as provenance only; ConfigLens never claims they authorize
//! anything.

use std::path::{Path, PathBuf};

use crate::adapters::{
    default_applicability, managed_dirs, Applicability, Candidate, Client, ClientAdapter,
    DiscoveryPlan, Registry, SourceClass,
};
use crate::context::ResolutionContext;
use crate::filesystem::FileAccess;
use crate::parse::Syntax;
use crate::security::PathPolicy;

/// The managed file that carries file-based policy.
const MANAGED_FILE: &str = "managed-settings.json";
/// The directory that splits one policy across teams.
const MANAGED_DIR: &str = "managed-settings.d";

pub struct ClaudeAdapter {
    registry: Registry,
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self {
            registry: Registry::load(Client::Claude)
                .unwrap_or_else(|_| Registry::unavailable(Client::Claude.id())),
        }
    }

    /// Where `~/.claude` lives, honouring `CLAUDE_CONFIG_DIR`.
    fn claude_home(&self, ctx: &ResolutionContext) -> PathBuf {
        match ctx.environment_value("CLAUDE_CONFIG_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => ctx.home.join(".claude"),
        }
    }

    /// `managed-settings.json` plus every `*.json` in `managed-settings.d/`, in merge order:
    /// the file first, then the directory entries in alphabetical order (hidden and non-`.json`
    /// entries are ignored, exactly as documented).
    fn managed_files(
        &self,
        dir: &Path,
        access: &dyn FileAccess,
        roots: &[PathBuf],
    ) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        let primary = dir.join(MANAGED_FILE);
        out.push((primary, "managed_settings_file".to_string()));
        let mut listed: Vec<PathBuf> = access
            .list_dir(&dir.join(MANAGED_DIR), roots)
            .into_iter()
            .filter(|p| {
                p.extension().map(|e| e == "json").unwrap_or(false)
                    && p.file_name()
                        .map(|n| !n.to_string_lossy().starts_with('.'))
                        .unwrap_or(false)
            })
            .collect();
        listed.sort();
        for path in listed {
            out.push((path, "managed_settings_directory".to_string()));
        }
        out
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientAdapter for ClaudeAdapter {
    fn client(&self) -> Client {
        Client::Claude
    }

    fn registry(&self) -> &Registry {
        &self.registry
    }

    fn discover(
        &self,
        ctx: &ResolutionContext,
        access: &dyn FileAccess,
        paths: &PathPolicy,
    ) -> DiscoveryPlan {
        let mut plan = DiscoveryPlan::default();
        for (id, why) in self.registry.coverage_gaps() {
            plan.expected_unobservable.push((id.to_string(), why));
        }
        let roots = &paths.approved_roots;
        let claude_home = self.claude_home(ctx);

        // 1. User settings (`$CLAUDE_CONFIG_DIR` wins over `~/.claude`).
        let user = claude_home.join("settings.json");
        if access.exists_regular(&user) {
            plan.candidates.push(Candidate {
                layer_id: "cc.user".to_string(),
                class: SourceClass::User,
                path: Some(user),
                inline: None,
                note: None,
            });
        }

        // 2. Shared project settings: the session's primary working directory.
        plan.candidates.push(Candidate {
            layer_id: "cc.project_shared".to_string(),
            class: SourceClass::ProjectShared,
            path: Some(ctx.cwd.join(".claude/settings.json")),
            inline: None,
            note: None,
        });

        // 3. Project local settings: the repository root from v2.1.211, otherwise the cwd.
        let root = ctx.project_root.clone().unwrap_or_else(|| ctx.cwd.clone());
        let local = root.join(".claude/settings.local.json");
        plan.candidates.push(Candidate {
            layer_id: "cc.project_local".to_string(),
            class: SourceClass::ProjectLocal,
            path: Some(local.clone()),
            inline: None,
            note: ctx
                .project_root
                .is_none()
                .then(|| "project_root_uncertain".to_string()),
        });

        // A file an earlier version (pre v2.1.211) left in the starting directory is still read;
        // the root file's value wins where both set the same key.
        let legacy = ctx.cwd.join(".claude/settings.local.json");
        if legacy != local && access.exists_regular(&legacy) {
            plan.candidates.push(Candidate {
                layer_id: "cc.project_local".to_string(),
                class: SourceClass::ProjectLocal,
                path: Some(legacy),
                inline: None,
                note: Some("legacy_pre_v2_1_211_path".to_string()),
            });
        }

        // 4. Session arguments: only when the user supplied them through the context file.
        if !ctx.client_arguments.is_empty() {
            plan.candidates.push(Candidate {
                layer_id: "cc.session_args".to_string(),
                class: SourceClass::SessionArgs,
                path: None,
                inline: None,
                note: Some("supplied_arguments_simulated".to_string()),
            });
        }

        // 6. Managed file tier for this platform.
        for dir in managed_dirs(Client::Claude, ctx.os, ctx.program_data.as_deref()) {
            for (path, why) in self.managed_files(&dir, access, roots) {
                plan.candidates.push(Candidate {
                    layer_id: "cc.managed.file".to_string(),
                    class: SourceClass::ManagedFile,
                    path: Some(path),
                    inline: None,
                    note: Some(why),
                });
            }
        }

        plan
    }

    fn syntax_for(&self, _layer_id: &str) -> Syntax {
        // Claude Code settings files are strict JSON: a comment or trailing comma is an error.
        Syntax::Strict
    }

    fn applicability(&self, layer_id: &str, ctx: &ResolutionContext) -> Applicability {
        let class = match layer_id {
            "cc.project_shared" => SourceClass::ProjectShared,
            "cc.project_local" => SourceClass::ProjectLocal,
            _ => SourceClass::User,
        };
        default_applicability(class, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{FailOn, Os, Trust, VersionContext};
    use crate::filesystem::MemoryFileAccess;
    use crate::parse::Limits;
    use crate::security::PathMode;

    fn ctx(cwd: &str, home: &str) -> ResolutionContext {
        ResolutionContext {
            client: "claude".into(),
            os: Os::Linux,
            cwd: PathBuf::from(cwd),
            project_root: None,
            home: PathBuf::from(home),
            xdg_config: PathBuf::from("/home/dev/.config"),
            program_data: None,
            approved_roots: Vec::new(),
            client_version: VersionContext::Claimed("2.1.296".into()),
            trust: Trust::Unknown,
            env: None,
            supplied_env: Default::default(),
            client_arguments: Vec::new(),
            supplied_sources: Vec::new(),
            strict: false,
            simulated: false,
            paths: PathMode::Local,
            limits: Limits::default(),
            total_bytes_limit: 16 * 1024 * 1024,
            max_sources: 128,
            fail_on: FailOn::Error,
            color: false,
        }
    }

    fn policy(ctx: &ResolutionContext, root: Option<&str>) -> PathPolicy {
        PathPolicy::new(
            &ctx.home,
            &ctx.xdg_config,
            root.map(Path::new),
            &[PathBuf::from("/etc/claude-code")],
            &[],
        )
    }

    fn paths(plan: &DiscoveryPlan) -> Vec<String> {
        plan.candidates
            .iter()
            .filter_map(|c| c.path.as_ref().map(|p| p.display().to_string()))
            .collect()
    }

    fn layer<'a>(plan: &'a DiscoveryPlan, id: &str) -> Vec<&'a Candidate> {
        plan.candidates
            .iter()
            .filter(|c| c.layer_id == id)
            .collect()
    }

    #[test]
    fn finds_user_settings_in_the_claude_home() {
        let ctx = ctx("/home/dev/work/p", "/home/dev");
        let access = MemoryFileAccess::new().with_file("/home/dev/.claude/settings.json", "{}");
        let plan = ClaudeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(paths(&plan).contains(&"/home/dev/.claude/settings.json".to_string()));
        assert_eq!(layer(&plan, "cc.user").len(), 1);
    }

    #[test]
    fn claude_config_dir_overrides_the_default_home_location() {
        let mut ctx = ctx("/home/dev/work/p", "/home/dev");
        ctx.env = Some(std::collections::BTreeMap::from([(
            "CLAUDE_CONFIG_DIR".to_string(),
            "/srv/claudehome".to_string(),
        )]));
        let access = MemoryFileAccess::new().with_file("/srv/claudehome/settings.json", "{}");
        let plan = ClaudeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(paths(&plan).contains(&"/srv/claudehome/settings.json".to_string()));
        assert!(!paths(&plan).contains(&"/home/dev/.claude/settings.json".to_string()));
    }

    #[test]
    fn project_files_are_read_from_the_working_directory_and_the_repository_root() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git")).expect("mkdir .git");
        let nested = root.join("sub");
        std::fs::create_dir_all(&nested).expect("mkdir sub");
        let ctx = ctx(&nested.display().to_string(), "/home/dev");
        let mut ctx = ctx;
        ctx.project_root = Some(root.to_path_buf());
        let access = MemoryFileAccess::new();
        let plan = ClaudeAdapter::new().discover(
            &ctx,
            &access,
            &policy(&ctx, Some(&root.display().to_string())),
        );

        // Shared project settings come from the session's primary working directory.
        assert_eq!(
            layer(&plan, "cc.project_shared")[0]
                .path
                .as_ref()
                .map(|p| p.to_path_buf()),
            Some(nested.join(".claude/settings.json"))
        );
        // Local settings live at the repository root (documented from v2.1.211).
        assert_eq!(
            layer(&plan, "cc.project_local")[0]
                .path
                .as_ref()
                .map(|p| p.to_path_buf()),
            Some(root.join(".claude/settings.local.json"))
        );
    }

    #[test]
    fn a_legacy_starting_directory_local_file_is_inventoried_separately() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git")).expect("mkdir .git");
        let nested = root.join("sub");
        std::fs::create_dir_all(&nested).expect("mkdir sub");
        let legacy = nested.join(".claude/settings.local.json");
        std::fs::create_dir_all(legacy.parent().expect("parent")).expect("mkdir");
        std::fs::write(&legacy, "{}").expect("write");

        let ctx = ctx(&nested.display().to_string(), "/home/dev");
        let mut ctx = ctx;
        ctx.project_root = Some(root.to_path_buf());
        let plan = ClaudeAdapter::new().discover(
            &ctx,
            &crate::filesystem::RealFileAccess,
            &policy(&ctx, Some(&root.display().to_string())),
        );
        let legacy_candidate = plan
            .candidates
            .iter()
            .find(|c| {
                c.note
                    .as_deref()
                    .unwrap_or("")
                    .contains("legacy_pre_v2_1_211_path")
            })
            .expect("legacy local file must be visible");
        assert_eq!(
            legacy_candidate.path.as_ref().map(|p| p.to_path_buf()),
            Some(legacy)
        );
    }

    #[test]
    fn without_a_repository_root_the_boundary_is_marked_uncertain() {
        let ctx = ctx("/tmp/loose/dir", "/home/dev");
        let plan =
            ClaudeAdapter::new().discover(&ctx, &MemoryFileAccess::new(), &policy(&ctx, None));
        assert!(plan.candidates.iter().any(|c| {
            c.layer_id == "cc.project_local"
                && c.note
                    .as_deref()
                    .unwrap_or("")
                    .contains("project_root_uncertain")
        }));
    }

    #[test]
    fn managed_file_and_directory_entries_are_merged_in_documented_order() {
        let dir = tempfile::tempdir().expect("temp");
        let managed = dir.path().join("claude-code");
        std::fs::create_dir_all(managed.join(MANAGED_DIR)).expect("mkdir");
        std::fs::write(managed.join(MANAGED_FILE), "{}").expect("write");
        std::fs::write(managed.join(MANAGED_DIR).join("20-security.json"), "{}").expect("write");
        std::fs::write(managed.join(MANAGED_DIR).join("10-telemetry.json"), "{}").expect("write");
        std::fs::write(managed.join(MANAGED_DIR).join(".hidden.json"), "{}").expect("write");
        std::fs::write(managed.join(MANAGED_DIR).join("notes.txt"), "x").expect("write");

        let mut ctx = ctx("/home/dev/work/p", "/home/dev");
        ctx.os = Os::Linux;
        let adapter = ClaudeAdapter::new();
        let files = adapter.managed_files(
            &managed,
            &crate::filesystem::RealFileAccess,
            std::slice::from_ref(&managed),
        );
        let names: Vec<String> = files
            .iter()
            .map(|(p, _)| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert_eq!(
            names,
            vec![
                "managed-settings.json",
                "10-telemetry.json",
                "20-security.json"
            ],
            "file first, then alphabetical *.json, hidden and non-json ignored"
        );
    }

    #[test]
    fn managed_layer_uses_the_platform_directory_from_the_registry_contract() {
        let mut ctx = ctx("/home/dev/work/p", "/home/dev");
        let adapter = ClaudeAdapter::new();
        let plan = adapter.discover(&ctx, &MemoryFileAccess::new(), &policy(&ctx, None));
        assert!(paths(&plan).contains(&"/etc/claude-code/managed-settings.json".to_string()));

        ctx.os = Os::Windows;
        let plan = adapter.discover(&ctx, &MemoryFileAccess::new(), &policy(&ctx, None));
        assert!(paths(&plan).iter().any(|p| p
            .replace('\\', "/")
            .ends_with("ClaudeCode/managed-settings.json")));
    }

    #[test]
    fn session_arguments_appear_only_when_they_were_supplied() {
        let ctx = ctx("/home/dev/work/p", "/home/dev");
        let plan =
            ClaudeAdapter::new().discover(&ctx, &MemoryFileAccess::new(), &policy(&ctx, None));
        assert!(layer(&plan, "cc.session_args").is_empty());

        let mut with_args = ctx.clone();
        with_args.client_arguments = vec!["--model".to_string(), "opus".to_string()];
        let plan = ClaudeAdapter::new().discover(
            &with_args,
            &MemoryFileAccess::new(),
            &policy(&with_args, None),
        );
        let args = layer(&plan, "cc.session_args");
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].class, SourceClass::SessionArgs);
        assert_eq!(args[0].path, None, "session flags have no file path");
    }

    #[test]
    fn mdm_server_managed_and_registry_layers_are_always_gaps() {
        let ctx = ctx("/home/dev/work/p", "/home/dev");
        let plan =
            ClaudeAdapter::new().discover(&ctx, &MemoryFileAccess::new(), &policy(&ctx, None));
        let gaps: Vec<&str> = plan
            .expected_unobservable
            .iter()
            .map(|(id, _)| id.as_str())
            .collect();
        assert!(gaps.contains(&"cc.managed.mdm"));
        assert!(gaps.contains(&"cc.managed.server"));
        assert!(gaps.contains(&"cc.managed.hkcu"));
    }

    #[test]
    fn project_files_are_conditional_when_trust_is_unknown() {
        let ctx = ctx("/home/dev/work/p", "/home/dev");
        let adapter = ClaudeAdapter::new();
        assert_eq!(
            adapter.applicability("cc.project_shared", &ctx),
            Applicability::Conditional("trust_unknown".to_string())
        );

        let mut trusted = ctx.clone();
        trusted.trust = Trust::Trusted;
        assert_eq!(
            adapter.applicability("cc.project_shared", &trusted),
            Applicability::Applicable
        );

        let mut untrusted = ctx;
        untrusted.trust = Trust::Untrusted;
        assert_eq!(
            adapter.applicability("cc.project_local", &untrusted),
            Applicability::Skipped("workspace_untrusted".to_string())
        );
    }

    #[test]
    fn claude_settings_files_use_strict_json() {
        assert_eq!(ClaudeAdapter::new().syntax_for("cc.user"), Syntax::Strict);
        assert_eq!(
            ClaudeAdapter::new().syntax_for("cc.managed.file"),
            Syntax::Strict
        );
    }
}
