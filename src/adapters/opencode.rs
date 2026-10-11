//! OpenCode adapter — bounded 0.1 local scope (spec §3, §12 step 5).
//!
//! Implemented: local scalar settings, nested object merges, verified `{env:}`/`{file:}`
//! interpolation, JSON/JSONC sources, `$OPENCODE_CONFIG` (a file), `$OPENCODE_CONFIG_DIR` (a
//! directory holding `opencode.json`/`opencode.jsonc`), managed file layer.
//! Always reported as gaps: remote `.well-known` defaults, `.opencode` directory contents, macOS
//! managed preferences.

use std::path::{Path, PathBuf};

use crate::adapters::{
    managed_dirs, Applicability, Candidate, Client, ClientAdapter, DiscoveryPlan, Registry,
    SourceClass,
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

        // 5b. Custom directory (`$OPENCODE_CONFIG_DIR`).
        //
        // Upstream treats this value as a *directory*, never as a configuration file: the pinned
        // loader appends it to the list of directories it scans (`config/paths.ts`:
        // `directories()` returns `unique([... , ...(Flag.OPENCODE_CONFIG_DIR ? [Flag.OPENCODE_CONFIG_DIR] : [])])`)
        // and then loads `opencode.json` followed by `opencode.jsonc` from every directory that
        // ends with `.opencode` **or equals `OPENCODE_CONFIG_DIR`** (`config/config.ts`). So the
        // sources here are `<dir>/opencode.json` and `<dir>/opencode.jsonc`, and a value that
        // points at a file yields nothing (upstream would look for `<file>/opencode.json`, which
        // does not exist). Placement is fixed by the same file: the directories loop runs after the
        // project files and before `$OPENCODE_CONFIG_CONTENT`, and `OPENCODE_CONFIG_DIR` is
        // appended last inside the loop.
        if let Some(dir) = ctx.environment_value("OPENCODE_CONFIG_DIR") {
            let dir = PathBuf::from(dir);
            for path in Self::files_in(&dir) {
                if access.exists_regular(&path) {
                    push(
                        &mut plan,
                        Candidate {
                            layer_id: "oc.custom_dir".to_string(),
                            class: SourceClass::CustomDir,
                            path: Some(path),
                            inline: None,
                            note: None,
                        },
                    );
                }
            }
        }

        // 4. Project config: every `opencode.json(c)` from `ctx.cwd` up to the boundary, outermost
        // first.
        //
        // Upstream (`config/paths.ts::files` → `FSUtil.up({targets: [opencode.jsonc,
        // opencode.json], start: cwd, stop: worktree})`) pushes a match at EVERY level from the
        // working directory up to the repository root, then `config.ts` merges that list outermost
        // first, so the nearest file wins and every ancestor contributes (pinned
        // `53d1eabb61e21162157817bf677da0a4ad3332e3`, evidence `OC-PROJECT-SOURCE-20261011`).
        // ConfigLens emits the same ordered list — boundary first, `ctx.cwd` last — so the engine's
        // last-definition-wins tie-break (`resolve`) selects the nearest file. The boundary is the
        // explicit `--project-root`, otherwise the nearest git directory; with no boundary the
        // working directory is the only level and the run records that the boundary is uncertain.
        let project_files = project_config_files(ctx, access);
        let boundary_uncertain = project_boundary(ctx, access).is_none();
        for (index, path) in project_files.into_iter().enumerate() {
            let mut signals: Vec<&str> = Vec::new();
            // The boundary-uncertain signal is a run-level fact; emit it once so the coverage gap is
            // not duplicated across a directory's two dialect files.
            if boundary_uncertain && index == 0 {
                signals.push("project_root_uncertain");
            }
            // A directory holding both dialects has an unverified preference between them; the note
            // follows the file.
            if sibling_config(&path, access) {
                signals.push("jsonc_coexistence_unverified");
            }
            push(
                &mut plan,
                Candidate {
                    layer_id: "oc.project".to_string(),
                    class: SourceClass::ProjectShared,
                    path: Some(path),
                    inline: None,
                    note: (!signals.is_empty()).then(|| signals.join(";")),
                },
            );
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

    fn applicability(&self, _layer_id: &str, _ctx: &ResolutionContext) -> Applicability {
        // OpenCode has no upstream workspace-trust concept (audit O4): a project config is always
        // applicable, and `$OPENCODE_CONFIG*` sources never depend on a trust decision.
        Applicability::Applicable
    }

    fn trust_applicable(&self) -> bool {
        // Trust gating is a Claude-Code-only concept; OpenCode never reports a trust gap.
        false
    }
}

/// The project boundary: the explicit `--project-root`, otherwise the nearest git directory.
///
/// Boundary rule (spec §6): upward discovery ends at the verified root or adapter boundary and never
/// recurses through unrelated repositories.
fn project_boundary(ctx: &ResolutionContext, access: &dyn FileAccess) -> Option<PathBuf> {
    ctx.project_root
        .clone()
        .or_else(|| find_git_root(access, &ctx.cwd, 64))
}

/// Every project config file from `ctx.cwd` up to the boundary, outermost first.
///
/// Upstream collects a match at every level from the working directory up to the repository root and
/// merges them outermost-first, so the nearest file wins. This returns the same order (boundary
/// first, `ctx.cwd` last), with `opencode.json` before `opencode.jsonc` at each level, so the
/// nearest file is last in definition order. The walk **starts at `ctx.cwd`** — the boundary is
/// never the search start — includes the boundary itself, and stops at the filesystem root when the
/// boundary is not an ancestor of `ctx.cwd`. With no boundary, only `ctx.cwd` is inspected and the
/// caller keeps the boundary-uncertain signal.
fn project_config_files(ctx: &ResolutionContext, access: &dyn FileAccess) -> Vec<PathBuf> {
    let Some(boundary) = project_boundary(ctx, access) else {
        return present_config_files(&ctx.cwd, access);
    };
    // Collect every level from cwd up to (and including) the boundary, then reverse to
    // outermost-first order.
    let mut levels: Vec<PathBuf> = Vec::new();
    let mut current = Some(ctx.cwd.clone());
    while let Some(dir) = current {
        let at_boundary = dir == boundary;
        let parent = dir.parent().map(|p| p.to_path_buf());
        levels.push(dir);
        if at_boundary {
            break;
        }
        current = parent;
    }
    levels
        .iter()
        .rev()
        .flat_map(|dir| present_config_files(dir, access))
        .collect()
}

/// The config files present as regular files inside one directory, in documented name order
/// (`opencode.json` then `opencode.jsonc`).
fn present_config_files(dir: &Path, access: &dyn FileAccess) -> Vec<PathBuf> {
    CONFIG_FILES
        .iter()
        .map(|name| dir.join(name))
        .filter(|path| access.exists_regular(path))
        .collect()
}

/// Does `path`'s directory also hold the other documented dialect?
fn sibling_config(path: &Path, access: &dyn FileAccess) -> bool {
    let Some(dir) = path.parent() else {
        return false;
    };
    CONFIG_FILES
        .iter()
        .any(|name| access.exists_regular(&dir.join(name)) && dir.join(name) != path)
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

    /// A host-native path built from `parts`, mirroring how the adapters push components.
    /// Windows joins with `\`, so a `/`-separated literal would never compare equal there.
    fn host_path(parts: &[&str]) -> String {
        let mut path = PathBuf::new();
        for part in parts {
            path.push(part);
        }
        path.display().to_string()
    }

    #[test]
    fn finds_the_global_config_in_the_xdg_config_home() {
        let ctx = ctx("/home/dev/work/p", "/home/dev/.config", "/home/dev");
        let global = host_path(&["/home/dev/.config", "opencode", "opencode.json"]);
        let access = MemoryFileAccess::new().with_file(global.clone(), "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(paths(&plan).contains(&global));
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
        let project = host_path(&["/home/dev/work/p", "opencode.json"]);
        let access = MemoryFileAccess::new().with_file(project.clone(), "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(paths(&plan).contains(&project));
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
            .with_file(host_path(&["/home/dev/work/p", "opencode.json"]), "{}")
            .with_file(host_path(&["/home/dev/work/p", "opencode.jsonc"]), "{}");
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
        let access = MemoryFileAccess::new()
            .with_file(host_path(&["/srv/team/ocdir", "opencode.json"]), "{}")
            .with_file(host_path(&["/srv/team/ocdir", "opencode.jsonc"]), "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
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
            .filter(|c| c.layer_id == "oc.custom_dir")
            .collect::<Vec<_>>();
        assert_eq!(
            custom_dir.len(),
            2,
            "both documented file names inside the directory are candidates"
        );
        assert_eq!(
            custom_dir
                .iter()
                .map(|c| c.path.clone().expect("path"))
                .collect::<Vec<_>>(),
            vec![
                PathBuf::from(host_path(&["/srv/team/ocdir", "opencode.json"])),
                PathBuf::from(host_path(&["/srv/team/ocdir", "opencode.jsonc"])),
            ],
            "the directory's config files are the sources, in upstream merge order"
        );
        assert!(custom_dir.iter().all(|c| c.note.is_none()));
    }

    #[test]
    fn custom_dir_never_treats_the_environment_value_as_a_config_file() {
        // `$OPENCODE_CONFIG_DIR` names a directory; a file-valued variable contributes nothing
        // because upstream looks for `<value>/opencode.json`. Regression for a real Windows beta
        // observation (a file value was read, a directory value was refused).
        let mut ctx = ctx("/home/dev/work/p", "/home/dev/.config", "/home/dev");
        ctx.env = Some(std::collections::BTreeMap::from([(
            "OPENCODE_CONFIG_DIR".to_string(),
            "/srv/team/ocdir/opencode.json".to_string(),
        )]));
        let access = MemoryFileAccess::new()
            .with_file(host_path(&["/srv/team/ocdir", "opencode.json"]), "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(
            plan.candidates
                .iter()
                .all(|c| c.layer_id != "oc.custom_dir"),
            "a file-valued variable must not become an oc.custom_dir source"
        );
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
        let linux_managed = host_path(&["/etc/opencode", "opencode.json"]);
        let access = MemoryFileAccess::new().with_file(linux_managed.clone(), "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(paths(&plan).contains(&linux_managed));

        ctx.os = Os::MacOs;
        let mac_dir = PathBuf::from("/Library/Application Support/opencode");
        let mac_managed = mac_dir.join("opencode.json").display().to_string();
        let access = MemoryFileAccess::new().with_file(mac_dir.join("opencode.json"), "{}");
        let plan = OpenCodeAdapter::new().discover(&ctx, &access, &policy(&ctx, None));
        assert!(paths(&plan).contains(&mac_managed));
        assert!(
            !paths(&plan).contains(&linux_managed),
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
