//! OpenCode adapter — bounded 0.1 local scope (spec §3, §12 step 5).
//!
//! Implemented: local scalar settings, nested object merges, verified `{env:}`/`{file:}`
//! interpolation, JSON/JSONC sources, managed file layer.
//! Always reported as gaps: remote `.well-known` defaults, `.opencode` directory contents, macOS
//! managed preferences, and `$OPENCODE_CONFIG_DIR` placement (whose position upstream does not pin).

use std::path::{Path, PathBuf};

use crate::adapters::{
    default_applicability, managed_dirs, Applicability, Candidate, Client, ClientAdapter,
    DiscoveryPlan, Registry, SourceClass,
};
use crate::context::ResolutionContext;
use crate::filesystem::FileAccess;
use crate::parse::Syntax;
use crate::security::PathPolicy;

/// The two file names OpenCode documents for every file-based layer.
const CONFIG_FILES: [&str; 2] = ["opencode.json", "opencode.jsonc"];

pub struct OpenCodeAdapter {
    registry: Registry,
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self {
            registry: Registry::load(Client::OpenCode)
                .unwrap_or_else(|_| Registry::unavailable(Client::OpenCode.id())),
        }
    }

    /// Candidate config files inside `dir`, in documented name order.
    fn files_in(dir: &Path) -> Vec<PathBuf> {
        CONFIG_FILES.iter().map(|name| dir.join(name)).collect()
    }
}

impl Default for OpenCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientAdapter for OpenCodeAdapter {
    fn client(&self) -> Client {
        Client::OpenCode
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
        let mut num_sources = 0usize;
        let mut push = |plan: &mut DiscoveryPlan, candidate: Candidate| {
            num_sources += 1;
            if num_sources <= ctx.max_sources {
                plan.candidates.push(candidate);
            }
        };

        // 2. Global config (lowest observable layer).
        for path in Self::files_in(&ctx.xdg_config.join("opencode")) {
            if access.exists_regular(&path) {
                push(
                    &mut plan,
                    Candidate {
                        layer_id: "oc.global".to_string(),
                        class: SourceClass::User,
                        path: Some(path),
                        inline: None,
                        note: None,
                    },
                );
            }
        }

        // 3. Custom path from the allowlisted environment.
        if let Some(custom) = ctx.environment_value("OPENCODE_CONFIG") {
            push(
                &mut plan,
                Candidate {
                    layer_id: "oc.custom_path".to_string(),
                    class: SourceClass::CustomPath,
                    path: Some(PathBuf::from(custom)),
                    inline: None,
                    note: None,
                },
            );
        }

        // 5b. Custom directory: position relative to project/inline is not documented upstream.
        if let Some(dir) = ctx.environment_value("OPENCODE_CONFIG_DIR") {
            push(
                &mut plan,
                Candidate {
                    layer_id: "oc.custom_dir".to_string(),
                    class: SourceClass::CustomDir,
                    path: Some(PathBuf::from(dir)),
                    inline: None,
                    note: Some("custom_dir_placement_unverified".to_string()),
                },
            );
        }

        // 4. Project config: cwd, then upward to the nearest git directory.
        let search_start = ctx.project_root.clone().unwrap_or_else(|| ctx.cwd.clone());
        if let Some(dir) = project_config_dir(ctx, &search_start, access) {
            let found = Self::files_in(&dir)
                .into_iter()
                .filter(|p| access.exists_regular(p))
                .collect::<Vec<_>>();
            let ambiguous = found.len() > 1;
            for path in found {
                push(
                    &mut plan,
                    Candidate {
                        layer_id: "oc.project".to_string(),
                        class: SourceClass::ProjectShared,
                        path: Some(path),
                        inline: None,
                        note: ambiguous.then(|| "jsonc_coexistence_unverified".to_string()),
                    },
                );
            }
        }

        // 6. Inline content from the allowlisted environment.
        if let Some(content) = ctx.environment_value("OPENCODE_CONFIG_CONTENT") {
            push(
                &mut plan,
                Candidate {
                    layer_id: "oc.inline".to_string(),
                    class: SourceClass::Inline,
                    path: None,
                    inline: Some(content.to_string()),
                    note: Some("inline_content_is_sensitive".to_string()),
                },
            );
        }

        // 7. Managed files for this platform.
        for dir in managed_dirs(Client::OpenCode, ctx.os, ctx.program_data.as_deref()) {
            for path in Self::files_in(&dir) {
                if access.exists_regular(&path) {
                    push(
                        &mut plan,
                        Candidate {
                            layer_id: "oc.managed_file".to_string(),
                            class: SourceClass::ManagedFile,
                            path: Some(path),
                            inline: None,
                            note: None,
                        },
                    );
                }
            }
        }

        let _ = roots;
        plan
    }

    fn syntax_for(&self, _layer_id: &str) -> Syntax {
        // OpenCode documents both JSON and JSONC for its config files.
        Syntax::Jsonc
    }

    fn applicability(&self, layer_id: &str, ctx: &ResolutionContext) -> Applicability {
        let class = match layer_id {
            "oc.project" => SourceClass::ProjectShared,
            _ => SourceClass::User,
        };
        default_applicability(class, ctx)
    }
}

/// Directory that holds the project config.
///
/// Boundary rule (spec §6): upward discovery ends at the verified root or adapter boundary and never
/// recurses through unrelated repositories. The boundary is the explicitly supplied `--project-root`,
/// otherwise the nearest git directory. With no boundary we look at the working directory only and
/// the caller records that the boundary is uncertain.
fn project_config_dir(
    ctx: &ResolutionContext,
    start: &Path,
    access: &dyn FileAccess,
) -> Option<PathBuf> {
    let boundary = match &ctx.project_root {
        Some(root) => Some(root.clone()),
        None => find_git_root(access, start, 64),
    };
    if boundary.is_none() {
        return CONFIG_FILES
            .iter()
            .any(|name| access.exists_regular(&start.join(name)))
            .then(|| start.to_path_buf());
    }
    let mut current = Some(start.to_path_buf());
    while let Some(dir) = current {
        if CONFIG_FILES
            .iter()
            .any(|name| access.exists_regular(&dir.join(name)))
        {
            return Some(dir);
        }
        if boundary.as_deref() == Some(dir.as_path()) {
            return None;
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Nearest ancestor containing a `.git` directory or a worktree `.git` file.
fn find_git_root(access: &dyn FileAccess, start: &Path, max_depth: usize) -> Option<PathBuf> {
    let mut current = Some(start.to_path_buf());
    let mut steps = 0usize;
    while let Some(dir) = current {
        if steps > max_depth {
            return None;
        }
        let marker = dir.join(".git");
        if access.exists_regular(&marker)
            || access.classify(&marker, &[]) == crate::filesystem::FileKind::Directory
        {
            return Some(dir);
        }
        current = dir.parent().map(|p| p.to_path_buf());
        steps += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{Os, ResolutionContext, Trust, VersionContext};
    use crate::filesystem::MemoryFileAccess;
    use crate::parse::Limits;
    use crate::security::PathMode;

    fn ctx(cwd: &str, xdg: &str, home: &str) -> ResolutionContext {
        ResolutionContext {
            client: "opencode".into(),
            os: Os::Linux,
            cwd: PathBuf::from(cwd),
            project_root: None,
            home: PathBuf::from(home),
            xdg_config: PathBuf::from(xdg),
            program_data: None,
            approved_roots: Vec::new(),
            client_version: VersionContext::Claimed("1.18.35".into()),
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
            fail_on: crate::context::FailOn::Error,
            color: false,
        }
    }

    fn policy(ctx: &ResolutionContext, root: Option<&str>) -> PathPolicy {
        PathPolicy::new(
            &ctx.home,
            &ctx.xdg_config,
            root.map(Path::new),
            &[PathBuf::from("/etc/opencode")],
            &[],
        )
    }

    fn paths(plan: &DiscoveryPlan) -> Vec<String> {
        plan.candidates
            .iter()
            .filter_map(|c| c.path.as_ref().map(|p| p.display().to_string()))
            .collect()
    }

    #[test]
    fn finds_the_global_config_in_the_xdg_config_home() {
        let ctx = ctx("/home/dev/work/p", "/home/dev/.config", "/home/dev");
        let access =
            MemoryFileAccess::new().with_file("/home/dev/.config/opencode/opencode.json", "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(paths(&plan).contains(&"/home/dev/.config/opencode/opencode.json".to_string()));
        let candidate = plan
            .candidates
            .iter()
            .find(|c| c.layer_id == "oc.global")
            .expect("global layer present");
        assert_eq!(candidate.class, SourceClass::User);
    }

    #[test]
    fn finds_a_project_config_in_the_cwd() {
        let ctx = ctx("/home/dev/work/p", "/home/dev/.config", "/home/dev");
        let access = MemoryFileAccess::new().with_file("/home/dev/work/p/opencode.json", "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(paths(&plan).contains(&"/home/dev/work/p/opencode.json".to_string()));
    }

    #[test]
    fn project_config_is_searched_up_to_the_nearest_git_directory() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git")).expect("mkdir");
        let nested = root.join("a/b");
        std::fs::create_dir_all(&nested).expect("mkdir");
        std::fs::write(root.join("opencode.json"), "{}").expect("write");

        let ctx = ctx(
            &nested.display().to_string(),
            "/home/dev/.config",
            "/home/dev",
        );
        let plan = OpenCodeAdapter::new().discover(
            &ctx,
            &crate::filesystem::RealFileAccess,
            &policy(&ctx, Some(&root.display().to_string())),
        );
        let project = plan
            .candidates
            .iter()
            .find(|c| c.layer_id == "oc.project" && c.path.is_some())
            .expect("project config found by walking up");
        assert_eq!(
            project.path.as_ref().map(|p| p.to_path_buf()),
            Some(root.join("opencode.json"))
        );
    }

    #[test]
    fn project_search_does_not_escape_the_filesystem_root() {
        let ctx = ctx("/tmp/definitely/not/here", "/home/dev/.config", "/home/dev");
        let plan =
            OpenCodeAdapter::new().discover(&ctx, &MemoryFileAccess::new(), &policy(&ctx, None));
        assert!(plan
            .candidates
            .iter()
            .all(|c| c.layer_id != "oc.project" || c.path.is_none()));
    }

    #[test]
    fn both_json_and_jsonc_in_one_directory_are_discovered() {
        let ctx = ctx("/home/dev/work/p", "/home/dev/.config", "/home/dev");
        let access = MemoryFileAccess::new()
            .with_file("/home/dev/work/p/opencode.json", "{}")
            .with_file("/home/dev/work/p/opencode.jsonc", "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        let project: Vec<&Candidate> = plan
            .candidates
            .iter()
            .filter(|c| c.layer_id == "oc.project" && c.path.is_some())
            .collect();
        assert_eq!(project.len(), 2, "both dialects must stay visible");
        assert!(project.iter().all(|c| c
            .note
            .as_deref()
            .unwrap_or("")
            .contains("jsonc_coexistence")));
    }

    #[test]
    fn custom_path_and_inline_come_from_the_allowlisted_environment() {
        let mut ctx = ctx("/home/dev/work/p", "/home/dev/.config", "/home/dev");
        ctx.env = Some(std::collections::BTreeMap::from([
            (
                "OPENCODE_CONFIG".to_string(),
                "/srv/team/oc.json".to_string(),
            ),
            (
                "OPENCODE_CONFIG_CONTENT".to_string(),
                "{\"model\":\"inline-model\"}".to_string(),
            ),
            (
                "OPENCODE_CONFIG_DIR".to_string(),
                "/srv/team/ocdir".to_string(),
            ),
        ]));
        let plan =
            OpenCodeAdapter::new().discover(&ctx, &MemoryFileAccess::new(), &policy(&ctx, None));
        assert!(paths(&plan).contains(&"/srv/team/oc.json".to_string()));
        let inline = plan
            .candidates
            .iter()
            .find(|c| c.layer_id == "oc.inline")
            .expect("inline candidate");
        assert_eq!(inline.class, SourceClass::Inline);
        assert_eq!(
            inline.inline.as_deref(),
            Some("{\"model\":\"inline-model\"}")
        );
        let custom_dir = plan
            .candidates
            .iter()
            .find(|c| c.layer_id == "oc.custom_dir")
            .expect("custom dir candidate");
        assert!(custom_dir
            .note
            .as_deref()
            .unwrap_or("")
            .contains("custom_dir_placement_unverified"));
    }

    #[test]
    fn no_env_disables_every_environment_derived_source() {
        let mut ctx = ctx("/home/dev/work/p", "/home/dev/.config", "/home/dev");
        ctx.env = None;
        let plan =
            OpenCodeAdapter::new().discover(&ctx, &MemoryFileAccess::new(), &policy(&ctx, None));
        assert!(plan.candidates.iter().all(|c| c.layer_id != "oc.inline"
            && c.layer_id != "oc.custom_path"
            && c.layer_id != "oc.custom_dir"));
    }

    #[test]
    fn managed_files_use_the_platform_directory() {
        let mut ctx = ctx("/home/dev/work/p", "/home/dev/.config", "/home/dev");
        let access = MemoryFileAccess::new().with_file("/etc/opencode/opencode.json", "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(paths(&plan).contains(&"/etc/opencode/opencode.json".to_string()));

        ctx.os = Os::MacOs;
        let mac_dir = PathBuf::from("/Library/Application Support/opencode");
        let access = MemoryFileAccess::new().with_file(mac_dir.join("opencode.json"), "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(paths(&plan)
            .contains(&"/Library/Application Support/opencode/opencode.json".to_string()));
        assert!(
            !paths(&plan).contains(&"/etc/opencode/opencode.json".to_string()),
            "the Linux managed directory must not be used on macOS"
        );
    }

    #[test]
    fn remote_directories_and_macos_preferences_are_always_reported_as_gaps() {
        let ctx = ctx("/home/dev/work/p", "/home/dev/.config", "/home/dev");
        let plan =
            OpenCodeAdapter::new().discover(&ctx, &MemoryFileAccess::new(), &policy(&ctx, None));
        let gaps: Vec<&str> = plan
            .expected_unobservable
            .iter()
            .map(|(id, _)| id.as_str())
            .collect();
        assert!(gaps.contains(&"oc.remote.wellknown"));
        assert!(gaps.contains(&"oc.opencode_dirs"));
        assert!(gaps.contains(&"oc.managed_macos_prefs"));
    }

    #[test]
    fn opencode_settings_files_accept_jsonc() {
        assert_eq!(
            OpenCodeAdapter::new().syntax_for("oc.global"),
            Syntax::Jsonc
        );
        assert_eq!(
            OpenCodeAdapter::new().syntax_for("oc.project"),
            Syntax::Jsonc
        );
    }
}
