//! The only module allowed to touch the filesystem (spec §7 "Resource and file safety").
//!
//! Properties enforced here:
//! * only regular files are read; directories, sockets, FIFOs and device nodes are classified and
//!   refused;
//! * a file is read through a **bounded** handle (limit + 1 bytes) so a "size check then unbounded
//!   read" race cannot happen;
//! * a symlink is followed only when the resolved file stays inside an authorised root;
//! * limits are reported as their own kind, never as silent truncation.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What a candidate path turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Missing,
    Regular,
    Directory,
    SymlinkEscape,
    Socket,
    Fifo,
    Device,
    OverLimit,
    Unreadable,
    NotUtf8,
}

impl FileKind {
    /// Diagnostic code used when this kind prevents a read.
    pub fn diagnostic_code(self) -> &'static str {
        match self {
            FileKind::Missing => "missing",
            FileKind::Regular => "ok",
            FileKind::Directory => "not_a_regular_file",
            FileKind::SymlinkEscape => "symlink_escape",
            FileKind::Socket => "not_a_regular_file",
            FileKind::Fifo => "not_a_regular_file",
            FileKind::Device => "not_a_regular_file",
            FileKind::OverLimit => "resource_limit_exceeded",
            FileKind::Unreadable => "unreadable",
            FileKind::NotUtf8 => "not_utf8",
        }
    }
}

/// Result of one bounded read attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadOutcome {
    pub kind: FileKind,
    pub content: Option<String>,
    pub bytes: u64,
}

impl ReadOutcome {
    pub fn missing() -> Self {
        Self {
            kind: FileKind::Missing,
            content: None,
            bytes: 0,
        }
    }

    pub fn regular(content: String) -> Self {
        let bytes = content.len() as u64;
        Self {
            kind: FileKind::Regular,
            content: Some(content),
            bytes,
        }
    }

    pub fn refused(kind: FileKind) -> Self {
        Self {
            kind,
            content: None,
            bytes: 0,
        }
    }

    pub fn is_readable(&self) -> bool {
        self.kind == FileKind::Regular && self.content.is_some()
    }
}

/// Bounded, root-aware file reads. Injected so reducers never touch the real filesystem.
pub trait FileAccess {
    /// Read at most `max_bytes` from `path`. `authorized_roots` bounds symlink following.
    fn read_capped(&self, path: &Path, max_bytes: u64, authorized_roots: &[PathBuf])
        -> ReadOutcome;

    /// Does `path` exist as a regular file (used for inventory without reading)?
    fn exists_regular(&self, path: &Path) -> bool;

    /// List the entries of one directory (used for `managed-settings.d/`). Implementations must not
    /// recurse and must not follow a symlink out of an authorised root.
    fn list_dir(&self, dir: &Path, authorized_roots: &[PathBuf]) -> Vec<PathBuf> {
        let _ = (dir, authorized_roots);
        Vec::new()
    }

    /// Fallible listing for callers that must distinguish an absent directory from an
    /// unobservable one. The default preserves existing injected implementations.
    fn list_dir_checked(
        &self,
        dir: &Path,
        authorized_roots: &[PathBuf],
    ) -> Result<Vec<PathBuf>, FileKind> {
        Ok(self.list_dir(dir, authorized_roots))
    }

    /// Classify without reading (used for inventory and doctor output).
    fn classify(&self, path: &Path, authorized_roots: &[PathBuf]) -> FileKind;
}

/// Tracks the run-wide resource budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    pub max_total_bytes: u64,
    pub max_sources: usize,
    pub used_bytes: u64,
    pub used_sources: usize,
}

impl Budget {
    pub fn new(max_total_bytes: u64, max_sources: usize) -> Self {
        Self {
            max_total_bytes,
            max_sources,
            used_bytes: 0,
            used_sources: 0,
        }
    }

    /// Charge one source. `Err(code)` means the run must report a limit diagnostic.
    pub fn charge_source(&mut self) -> Result<(), &'static str> {
        if self.used_sources + 1 > self.max_sources {
            return Err("source_limit_exceeded");
        }
        self.used_sources += 1;
        Ok(())
    }

    /// Charge bytes read. `Err(code)` means the run must report a limit diagnostic.
    pub fn charge_bytes(&mut self, bytes: u64) -> Result<(), &'static str> {
        if self.used_bytes.saturating_add(bytes) > self.max_total_bytes {
            return Err("total_bytes_exceeded");
        }
        self.used_bytes += bytes;
        Ok(())
    }
}

/// Real filesystem access.
#[derive(Debug, Default)]
pub struct RealFileAccess;

impl FileAccess for RealFileAccess {
    fn read_capped(
        &self,
        path: &Path,
        max_bytes: u64,
        authorized_roots: &[PathBuf],
    ) -> ReadOutcome {
        use std::io::Read;

        if authorized_roots.is_empty() {
            return ReadOutcome::refused(FileKind::SymlinkEscape);
        }
        let kind = self.classify(path, authorized_roots);
        if kind != FileKind::Regular {
            return ReadOutcome::refused(kind);
        }
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return ReadOutcome::refused(FileKind::Unreadable),
        };
        // Read at most max_bytes + 1 to detect "over limit" without an unbounded read.
        let mut buf = Vec::new();
        let mut handle = file.take(max_bytes + 1);
        if handle.read_to_end(&mut buf).is_err() {
            return ReadOutcome::refused(FileKind::Unreadable);
        }
        if buf.len() as u64 > max_bytes {
            return ReadOutcome::refused(FileKind::OverLimit);
        }
        match String::from_utf8(buf) {
            Ok(text) => ReadOutcome::regular(text),
            Err(_) => ReadOutcome::refused(FileKind::NotUtf8),
        }
    }

    fn exists_regular(&self, path: &Path) -> bool {
        std::fs::metadata(path)
            .map(|m| m.is_file())
            .unwrap_or(false)
    }

    fn list_dir(&self, dir: &Path, authorized_roots: &[PathBuf]) -> Vec<PathBuf> {
        self.list_dir_checked(dir, authorized_roots)
            .unwrap_or_default()
    }

    fn list_dir_checked(
        &self,
        dir: &Path,
        authorized_roots: &[PathBuf],
    ) -> Result<Vec<PathBuf>, FileKind> {
        if authorized_roots.is_empty() {
            return Err(FileKind::SymlinkEscape);
        }
        match self.classify(dir, authorized_roots) {
            FileKind::Missing => return Ok(Vec::new()),
            FileKind::Directory => {}
            other => return Err(other),
        }
        let entries = std::fs::read_dir(dir).map_err(|_| FileKind::Unreadable)?;
        // Keep refused entries: discovery must report them rather than silently
        // treating an unreadable/escaping managed fragment as absent policy.
        entries
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|_| FileKind::Unreadable)
            })
            .collect()
    }

    fn classify(&self, path: &Path, authorized_roots: &[PathBuf]) -> FileKind {
        if path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return FileKind::SymlinkEscape;
        }
        let link_meta = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return FileKind::Missing,
            Err(_) => return FileKind::Unreadable,
        };
        // Resolve the entire path, not just a symlink in its final component:
        // any parent directory can redirect an otherwise ordinary file.
        // Empty roots remain usable for metadata-only discovery of Git markers;
        // they never authorize following a final symlink.
        if !authorized_roots.is_empty() || link_meta.file_type().is_symlink() {
            let target = match std::fs::canonicalize(path) {
                Ok(t) => t,
                Err(_) => return FileKind::Unreadable,
            };
            let inside = authorized_roots.iter().any(|root| {
                !root.as_os_str().is_empty()
                    && !root
                        .components()
                        .any(|component| matches!(component, std::path::Component::ParentDir))
                    && std::fs::canonicalize(root)
                        .map(|r| target.starts_with(&r))
                        .unwrap_or(false)
            });
            if !inside {
                return FileKind::SymlinkEscape;
            }
        }
        let md = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return FileKind::Unreadable,
        };
        let ft = md.file_type();
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if ft.is_fifo() {
                return FileKind::Fifo;
            }
            if ft.is_socket() {
                return FileKind::Socket;
            }
            if ft.is_block_device() || ft.is_char_device() {
                return FileKind::Device;
            }
        }
        if ft.is_dir() {
            return FileKind::Directory;
        }
        if ft.is_file() {
            return FileKind::Regular;
        }
        FileKind::Unreadable
    }
}

/// In-memory access for tests: no test ever touches the developer's real files.
#[derive(Debug, Default, Clone)]
pub struct MemoryFileAccess {
    pub files: BTreeMap<PathBuf, (String, FileKind)>,
}

impl MemoryFileAccess {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_file(mut self, path: impl Into<PathBuf>, content: &str) -> Self {
        self.files
            .insert(path.into(), (content.to_string(), FileKind::Regular));
        self
    }

    pub fn with_kind(mut self, path: impl Into<PathBuf>, kind: FileKind) -> Self {
        self.files.insert(path.into(), (String::new(), kind));
        self
    }
}

impl FileAccess for MemoryFileAccess {
    fn read_capped(&self, path: &Path, max_bytes: u64, _roots: &[PathBuf]) -> ReadOutcome {
        match self.files.get(path) {
            None => ReadOutcome::missing(),
            Some((_, kind)) if *kind != FileKind::Regular => ReadOutcome::refused(*kind),
            Some((content, _)) => {
                if content.len() as u64 > max_bytes {
                    ReadOutcome::refused(FileKind::OverLimit)
                } else {
                    ReadOutcome::regular(content.clone())
                }
            }
        }
    }

    fn exists_regular(&self, path: &Path) -> bool {
        matches!(self.files.get(path), Some((_, FileKind::Regular)))
    }

    fn classify(&self, path: &Path, _roots: &[PathBuf]) -> FileKind {
        self.files
            .get(path)
            .map(|(_, kind)| *kind)
            .unwrap_or(FileKind::Missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    #[test]
    fn reads_a_regular_file_and_counts_bytes() {
        let dir = temp();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{\"a\": 1}").expect("write");
        let access = RealFileAccess;
        let out = access.read_capped(&path, 1024, &[dir.path().to_path_buf()]);
        assert_eq!(out.kind, FileKind::Regular);
        assert_eq!(out.content.as_deref(), Some("{\"a\": 1}"));
        assert_eq!(out.bytes, 8);
    }

    #[cfg(unix)]
    #[test]
    fn parent_directory_symlink_escape_is_refused() {
        let inside = temp();
        let outside = temp();
        std::fs::write(outside.path().join("settings.json"), "{}").expect("write");
        std::os::unix::fs::symlink(outside.path(), inside.path().join("config")).expect("symlink");
        let path = inside.path().join("config/settings.json");
        let out = RealFileAccess.read_capped(&path, 1024, &[inside.path().to_path_buf()]);
        assert_eq!(out.kind, FileKind::SymlinkEscape);
        assert!(out.content.is_none());
    }

    #[test]
    fn missing_file_is_missing_not_an_error() {
        let dir = temp();
        let path = dir.path().join("nope.json");
        let out = RealFileAccess.read_capped(&path, 1024, &[dir.path().to_path_buf()]);
        assert_eq!(out.kind, FileKind::Missing);
        assert_eq!(out.kind.diagnostic_code(), "missing");
    }

    #[test]
    fn directories_are_refused() {
        let dir = temp();
        let out = RealFileAccess.read_capped(dir.path(), 1024, &[dir.path().to_path_buf()]);
        assert_eq!(out.kind, FileKind::Directory);
        assert_eq!(out.kind.diagnostic_code(), "not_a_regular_file");
    }

    #[test]
    fn files_over_the_limit_are_refused_not_truncated() {
        let dir = temp();
        let path = dir.path().join("big.json");
        std::fs::write(&path, "0123456789").expect("write");
        let out = RealFileAccess.read_capped(&path, 4, &[dir.path().to_path_buf()]);
        assert_eq!(out.kind, FileKind::OverLimit);
        assert!(out.content.is_none(), "no silent truncation");
        assert_eq!(out.kind.diagnostic_code(), "resource_limit_exceeded");
    }

    #[test]
    fn a_file_exactly_at_the_limit_is_read() {
        let dir = temp();
        let path = dir.path().join("exact.json");
        std::fs::write(&path, "0123456789").expect("write");
        let out = RealFileAccess.read_capped(&path, 10, &[dir.path().to_path_buf()]);
        assert_eq!(out.kind, FileKind::Regular);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_inside_an_authorized_root_is_followed() {
        let dir = temp();
        let target = dir.path().join("real.json");
        let link = dir.path().join("link.json");
        std::fs::write(&target, "{}").expect("write");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        let out = RealFileAccess.read_capped(&link, 1024, &[dir.path().to_path_buf()]);
        assert_eq!(out.kind, FileKind::Regular);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escaping_the_authorized_root_is_refused() {
        let outside = temp();
        let inside = temp();
        let target = outside.path().join("secret.json");
        std::fs::write(&target, "{\"k\":\"SEEDED\"}").expect("write");
        let link = inside.path().join("link.json");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        let out = RealFileAccess.read_capped(&link, 1024, &[inside.path().to_path_buf()]);
        assert_eq!(out.kind, FileKind::SymlinkEscape);
        assert!(out.content.is_none());
        assert_eq!(out.kind.diagnostic_code(), "symlink_escape");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_loops_do_not_hang_and_are_refused() {
        let dir = temp();
        let a = dir.path().join("a.json");
        let b = dir.path().join("b.json");
        std::os::unix::fs::symlink(&b, &a).expect("symlink a");
        std::os::unix::fs::symlink(&a, &b).expect("symlink b");
        let out = RealFileAccess.read_capped(&a, 1024, &[dir.path().to_path_buf()]);
        assert!(!out.is_readable());
    }

    #[cfg(unix)]
    #[test]
    fn sockets_are_refused() {
        let dir = temp();
        let path = dir.path().join("sock");
        let _listener = std::os::unix::net::UnixListener::bind(&path).expect("bind");
        let kind = RealFileAccess.classify(&path, &[dir.path().to_path_buf()]);
        assert_eq!(kind, FileKind::Socket);
        assert_eq!(kind.diagnostic_code(), "not_a_regular_file");
    }

    #[test]
    fn non_utf8_files_are_refused_as_not_utf8() {
        let dir = temp();
        let path = dir.path().join("bin.json");
        std::fs::write(&path, [0xff, 0xfe, 0x00]).expect("write");
        let out = RealFileAccess.read_capped(&path, 1024, &[dir.path().to_path_buf()]);
        assert_eq!(out.kind, FileKind::NotUtf8);
    }

    #[test]
    fn budget_charges_sources_and_bytes() {
        let mut budget = Budget::new(10, 2);
        assert!(budget.charge_source().is_ok());
        assert!(budget.charge_source().is_ok());
        assert_eq!(budget.charge_source().unwrap_err(), "source_limit_exceeded");
        assert!(budget.charge_bytes(10).is_ok());
        assert_eq!(budget.charge_bytes(1).unwrap_err(), "total_bytes_exceeded");
        assert_eq!(budget.used_bytes, 10);
    }

    #[test]
    fn list_dir_reports_entries_without_recursing() {
        let dir = temp();
        std::fs::create_dir_all(dir.path().join("managed-settings.d")).expect("mkdir");
        std::fs::write(dir.path().join("managed-settings.d/20-security.json"), "{}")
            .expect("write");
        std::fs::write(
            dir.path().join("managed-settings.d/10-telemetry.json"),
            "{}",
        )
        .expect("write");
        std::fs::write(dir.path().join("managed-settings.d/.hidden.json"), "{}").expect("write");
        std::fs::write(dir.path().join("managed-settings.d/notes.txt"), "x").expect("write");
        let mut listed = RealFileAccess.list_dir(
            &dir.path().join("managed-settings.d"),
            &[dir.path().to_path_buf()],
        );
        listed.sort();
        assert_eq!(
            listed.len(),
            4,
            "listing is raw; filtering happens in the adapter"
        );
        assert!(listed.iter().any(|p| p.ends_with("10-telemetry.json")));

        assert!(RealFileAccess
            .list_dir(&dir.path().join("nope"), &[dir.path().to_path_buf()])
            .is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn permission_denied_parent_is_unreadable_not_missing() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp();
        let parent = dir.path().join("private");
        std::fs::create_dir(&parent).expect("mkdir");
        let path = parent.join("settings.json");
        std::fs::write(&path, "{}").expect("write");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        let metadata = std::fs::symlink_metadata(&path);
        let outcome = RealFileAccess.read_capped(&path, 1024, &[dir.path().to_path_buf()]);
        // Restore traversal before any assertion (including when tests run as root).
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700))
            .expect("restore permissions");
        if metadata.is_ok() {
            return; // A privileged process cannot exercise permission denial.
        }
        assert_eq!(
            metadata.expect_err("denied").kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(outcome.kind, FileKind::Unreadable);
        assert!(outcome.content.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn parent_symlink_inside_root_and_explicitly_approved_target_are_readable() {
        let inside = temp();
        let outside = temp();
        std::fs::create_dir(inside.path().join("real")).expect("mkdir");
        std::fs::write(inside.path().join("real/settings.json"), "{}").expect("write");
        let link = inside.path().join("config");
        std::os::unix::fs::symlink(inside.path().join("real"), &link).expect("symlink");
        assert!(RealFileAccess
            .read_capped(
                &link.join("settings.json"),
                1024,
                &[inside.path().to_path_buf()],
            )
            .is_readable());
        std::fs::remove_file(&link).expect("remove link");
        std::fs::write(outside.path().join("settings.json"), "{}").expect("write");
        std::os::unix::fs::symlink(outside.path(), &link).expect("symlink");
        assert!(RealFileAccess
            .read_capped(
                &link.join("settings.json"),
                1024,
                &[inside.path().to_path_buf(), outside.path().to_path_buf()],
            )
            .is_readable());
    }

    #[test]
    fn no_roots_do_not_authorize_reads_or_directory_enumeration() {
        let dir = temp();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{}").expect("write");
        assert!(!RealFileAccess.read_capped(&path, 1024, &[]).is_readable());
        assert_eq!(
            RealFileAccess.list_dir_checked(dir.path(), &[]),
            Err(FileKind::SymlinkEscape)
        );
    }

    #[cfg(unix)]
    #[test]
    fn parent_symlink_loop_is_unreadable_not_missing() {
        let dir = temp();
        let link = dir.path().join("loop");
        std::os::unix::fs::symlink(&link, &link).expect("symlink");
        let out = RealFileAccess.read_capped(
            &link.join("settings.json"),
            1024,
            &[dir.path().to_path_buf()],
        );
        assert_eq!(out.kind, FileKind::Unreadable);
    }

    #[cfg(unix)]
    #[test]
    fn listing_preserves_refused_fragments_for_diagnostics() {
        let dir = temp();
        let outside = temp();
        let fragment = dir.path().join("10-policy.json");
        std::fs::write(outside.path().join("policy.json"), "{}").expect("write");
        std::os::unix::fs::symlink(outside.path().join("policy.json"), &fragment).expect("symlink");
        let directory = dir.path().join("20-directory.json");
        std::fs::create_dir(&directory).expect("mkdir");
        let listed = RealFileAccess.list_dir(dir.path(), &[dir.path().to_path_buf()]);
        assert!(listed.contains(&fragment));
        assert!(listed.contains(&directory));
    }

    #[cfg(unix)]
    #[test]
    fn listing_does_not_follow_an_escaping_directory_symlink() {
        let dir = temp();
        let outside = temp();
        std::fs::write(outside.path().join("policy.json"), "{}").expect("write");
        let link = dir.path().join("managed-settings.d");
        std::os::unix::fs::symlink(outside.path(), &link).expect("symlink");
        assert!(RealFileAccess
            .list_dir(&link, &[dir.path().to_path_buf()])
            .is_empty());
    }

    #[test]
    fn memory_access_mirrors_the_real_rules() {
        let access = MemoryFileAccess::new()
            .with_file("/home/u/.claude/settings.json", "{\"model\": \"a\"}")
            .with_kind("/home/u/.claude/sock", FileKind::Socket);
        assert!(access
            .read_capped(Path::new("/home/u/.claude/settings.json"), 1024, &[])
            .is_readable());
        assert_eq!(
            access
                .read_capped(Path::new("/home/u/.claude/sock"), 1024, &[])
                .kind,
            FileKind::Socket
        );
        assert_eq!(
            access.read_capped(Path::new("/nope"), 1024, &[]).kind,
            FileKind::Missing
        );
    }
}
