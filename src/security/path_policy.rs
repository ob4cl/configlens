//! Path policy: display aliases and the allowed-read-root check (spec §4 `--allow-read-root`, §7).
//!
//! ConfigLens never accepts an arbitrary path to read. Every candidate must sit under a root that
//! the current resolution context authorises: the home directory, the project root, the platform
//! managed-config directories, or a root the user passed explicitly with `--allow-read-root`.

use std::path::{Path, PathBuf};

use crate::security::sanitize_bounded;

/// One alias root, e.g. `$HOME` for `/home/user`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    pub root: PathBuf,
    pub name: &'static str,
}

/// Display + read policy derived from the resolution context.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathPolicy {
    pub aliases: Vec<Alias>,
    pub approved_roots: Vec<PathBuf>,
}

/// Does `candidate` sit inside `root`, comparing path components (no filesystem access)?
pub fn is_within(root: &Path, candidate: &Path) -> bool {
    let r: Vec<_> = root.components().collect();
    let c: Vec<_> = candidate.components().collect();
    c.len() >= r.len() && r.iter().zip(c.iter()).all(|(a, b)| a == b)
}

impl PathPolicy {
    /// Build from the context. `extra_approved` comes from `--allow-read-root`.
    pub fn new(
        home: &Path,
        xdg_config: &Path,
        project_root: Option<&Path>,
        managed_roots: &[PathBuf],
        extra_approved: &[PathBuf],
    ) -> Self {
        let mut aliases = vec![Alias {
            root: home.to_path_buf(),
            name: "$HOME",
        }];
        if !xdg_config.as_os_str().is_empty() {
            aliases.push(Alias {
                root: xdg_config.to_path_buf(),
                name: "$XDG_CONFIG",
            });
        }
        if let Some(root) = project_root {
            aliases.push(Alias {
                root: root.to_path_buf(),
                name: "$PROJECT",
            });
        }
        for root in managed_roots {
            aliases.push(Alias {
                root: root.clone(),
                name: "$MANAGED",
            });
        }
        let mut approved_roots: Vec<PathBuf> = Vec::new();
        for alias in &aliases {
            if !approved_roots.contains(&alias.root) {
                approved_roots.push(alias.root.clone());
            }
        }
        for root in extra_approved {
            if !approved_roots.contains(root) {
                approved_roots.push(root.clone());
            }
        }
        Self {
            aliases,
            approved_roots,
        }
    }

    /// Render `path` for a report: aliased in local mode, further reduced in share mode.
    pub fn display(&self, path: &Path, mode: crate::security::PathMode) -> String {
        let mut best: Option<&Alias> = None;
        for alias in &self.aliases {
            if is_within(&alias.root, path) {
                let deeper = best
                    .map(|b| alias.root.components().count() > b.root.components().count())
                    .unwrap_or(true);
                if deeper {
                    best = Some(alias);
                }
            }
        }
        if let Some(alias) = best {
            let rest = path.strip_prefix(&alias.root).unwrap_or(Path::new(""));
            let rest = rest.to_string_lossy().replace('\\', "/");
            let joined = if rest.is_empty() {
                alias.name.to_string()
            } else {
                format!("{}/{}", alias.name, rest)
            };
            return sanitize_bounded(&joined, 256);
        }
        match mode {
            crate::security::PathMode::Local => sanitize_bounded(&path.display().to_string(), 256),
            crate::security::PathMode::Share => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                sanitize_bounded(&format!("$EXTERNAL/{name}"), 256)
            }
        }
    }

    /// May this path be read? `Err(code)` is a blocked-read reason, never a path leak.
    pub fn check_read(&self, path: &Path) -> Result<(), &'static str> {
        if self.approved_roots.iter().any(|root| is_within(root, path)) {
            Ok(())
        } else {
            Err("outside_approved_roots")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::PathMode;

    fn policy() -> PathPolicy {
        PathPolicy::new(
            Path::new("/home/dev"),
            Path::new("/home/dev/.config"),
            Some(Path::new("/home/dev/work/project")),
            &[PathBuf::from("/etc/claude-code")],
            &[],
        )
    }

    #[test]
    fn is_within_compares_components_not_prefix_strings() {
        assert!(is_within(Path::new("/a/b"), Path::new("/a/b/c")));
        assert!(is_within(Path::new("/a/b"), Path::new("/a/b")));
        assert!(!is_within(Path::new("/a/b"), Path::new("/a/bc")));
        assert!(!is_within(Path::new("/a/b"), Path::new("/a")));
        assert!(is_within(Path::new("/"), Path::new("/anything")));
    }

    #[test]
    fn display_aliases_the_known_roots() {
        let p = policy();
        assert_eq!(
            p.display(
                Path::new("/home/dev/.claude/settings.json"),
                PathMode::Local
            ),
            "$HOME/.claude/settings.json"
        );
        assert_eq!(
            p.display(
                Path::new("/home/dev/work/project/.claude/settings.json"),
                PathMode::Local
            ),
            "$PROJECT/.claude/settings.json"
        );
        assert_eq!(
            p.display(
                Path::new("/home/dev/.config/opencode/opencode.json"),
                PathMode::Local
            ),
            "$XDG_CONFIG/opencode/opencode.json"
        );
        assert_eq!(
            p.display(
                Path::new("/etc/claude-code/managed-settings.json"),
                PathMode::Local
            ),
            "$MANAGED/managed-settings.json"
        );
    }

    #[test]
    fn project_alias_wins_over_home_when_nested() {
        let p = policy();
        assert_eq!(
            p.display(
                Path::new("/home/dev/work/project/opencode.json"),
                PathMode::Local
            ),
            "$PROJECT/opencode.json"
        );
    }

    #[test]
    fn unknown_paths_keep_their_location_locally() {
        let p = policy();
        assert_eq!(
            p.display(Path::new("/opt/other/opencode.json"), PathMode::Local),
            "/opt/other/opencode.json"
        );
    }

    #[test]
    fn share_mode_drops_directory_components_of_external_paths() {
        let p = policy();
        assert_eq!(
            p.display(Path::new("/opt/other/opencode.json"), PathMode::Share),
            "$EXTERNAL/opencode.json"
        );
        // Alias roots already hide the machine-specific part.
        assert_eq!(
            p.display(
                Path::new("/home/dev/.claude/settings.json"),
                PathMode::Share
            ),
            "$HOME/.claude/settings.json"
        );
    }

    #[test]
    fn display_sanitizes_control_characters_in_names() {
        let p = policy();
        let shown = p.display(
            Path::new("/home/dev/.claude/evil\u{1b}]8;;http://x\u{7}.json"),
            PathMode::Local,
        );
        assert_eq!(shown, "$HOME/.claude/evil.json");
    }

    #[test]
    fn reads_are_allowed_inside_authorized_roots() {
        let p = policy();
        assert!(p
            .check_read(Path::new("/home/dev/.claude/settings.json"))
            .is_ok());
        assert!(p
            .check_read(Path::new("/etc/claude-code/managed-settings.json"))
            .is_ok());
        assert!(p
            .check_read(Path::new("/home/dev/work/project/opencode.json"))
            .is_ok());
    }

    #[test]
    fn reads_outside_authorized_roots_are_blocked() {
        let p = policy();
        assert_eq!(
            p.check_read(Path::new("/etc/shadow")).unwrap_err(),
            "outside_approved_roots"
        );
    }

    #[test]
    fn extra_approved_roots_open_exactly_one_subtree() {
        let p = PathPolicy::new(
            Path::new("/home/dev"),
            Path::new("/home/dev/.config"),
            None,
            &[],
            &[PathBuf::from("/srv/shared")],
        );
        assert!(p.check_read(Path::new("/srv/shared/team.json")).is_ok());
        assert!(p.check_read(Path::new("/srv/other/team.json")).is_err());
    }
}
