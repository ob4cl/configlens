//! Deterministic filesystem interleavings; included only in commands' unit tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use std::fs;

struct HookGuard;
impl Drop for HookGuard {
    fn drop(&mut self) {
        SNAPSHOT_HOOK.with(|hook| *hook.borrow_mut() = None);
    }
}
fn hook(callback: impl FnMut(SnapshotStage, &Path) + 'static) -> HookGuard {
    SNAPSHOT_HOOK.with(|hook| *hook.borrow_mut() = Some(Box::new(callback)));
    HookGuard
}
#[test]
fn report_limit_is_inclusive_for_writer_and_parser() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out");
    let envelope = serde_json::json!({
        "schema_version": crate::REPORT_SCHEMA_VERSION,
        "client": "claude", "resolutions": []
    })
    .to_string();
    let content = format!(
        "{}{}",
        envelope,
        " ".repeat(MAX_REPORT_BYTES - envelope.len())
    );
    assert_eq!(content.len(), MAX_REPORT_BYTES);
    let outcome = write_snapshot(&target, &content, false).unwrap();
    assert_eq!(outcome.bytes, MAX_REPORT_BYTES as u64);
    assert!(parse_report(&fs::read_to_string(&target).unwrap()).is_ok());
    assert_eq!(entries(dir.path()), 1);
}

#[test]
fn oversized_reports_are_rejected_before_filesystem_effects_or_parsing() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out");
    fs::write(&target, "preserve me").unwrap();
    let _hook = hook(|_, _| panic!("oversized report must not create a temporary"));
    // Invalid JSON proves the size guard runs before the parser as well.
    let content = "x".repeat(MAX_REPORT_BYTES + 1);
    for overwrite in [false, true] {
        let error = write_snapshot(&target, &content, overwrite).unwrap_err();
        assert_eq!(error.code(), "resource_limit_exceeded");
        assert_eq!(error.exit_code(), 4);
    }
    let error =
        write_snapshot(&dir.path().join("missing").join("out"), &content, true).unwrap_err();
    assert_eq!(error.code(), "resource_limit_exceeded");
    let error = parse_report(&content).unwrap_err();
    assert_eq!(error.code(), "resource_limit_exceeded");
    assert_eq!(error.exit_code(), 4);
    assert_eq!(fs::read_to_string(&target).unwrap(), "preserve me");
    assert_eq!(entries(dir.path()), 1);
}

fn entries(path: &Path) -> usize {
    fs::read_dir(path).unwrap().count()
}

#[cfg(unix)]
#[test]
fn temporary_symlink_collision_never_truncates_its_target() {
    let dir = tempfile::tempdir().unwrap();
    let victim = dir.path().join("synthetic-victim");
    fs::write(&victim, "preserve me").unwrap();
    let victim_hook = victim.clone();
    let _hook = hook(move |stage, temporary| {
        if stage == SnapshotStage::BeforeCreate {
            std::os::unix::fs::symlink(&victim_hook, temporary).unwrap();
        }
    });
    let result = write_snapshot(&dir.path().join("out"), "synthetic report", false);
    assert!(
        result.is_err(),
        "exclusive creation must reject a collision"
    );
    assert_eq!(fs::read_to_string(victim).unwrap(), "preserve me");
    // A colliding entry is not ours to remove.
    assert_eq!(entries(dir.path()), 2);
}

#[test]
fn temporary_regular_collision_never_truncates_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let _hook = hook(|stage, temporary| {
        if stage == SnapshotStage::BeforeCreate {
            fs::write(temporary, "preserve me").unwrap();
        }
    });
    assert!(write_snapshot(&dir.path().join("out"), "report", false).is_err());
    let file = fs::read_dir(dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(fs::read_to_string(file).unwrap(), "preserve me");
}

#[cfg(unix)]
#[test]
fn temporary_is_private_before_any_content_is_written() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let _hook = hook(|stage, temporary| {
        if stage == SnapshotStage::AfterCreate {
            assert_eq!(fs::metadata(temporary).unwrap().len(), 0);
            assert_eq!(
                fs::metadata(temporary).unwrap().permissions().mode() & 0o077,
                0
            );
        }
    });
    write_snapshot(&dir.path().join("out"), "synthetic report", false).unwrap();
}

#[test]
fn destination_created_before_publication_is_not_clobbered() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out");
    let competing_target = target.clone();
    let _hook = hook(move |stage, _| {
        if stage == SnapshotStage::BeforePublish {
            fs::write(&competing_target, "competing writer").unwrap();
        }
    });
    let result = write_snapshot(&target, "report", false);
    assert!(
        result.is_err(),
        "no-overwrite applies at publication, not just preflight"
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "competing writer");
    assert_eq!(
        entries(dir.path()),
        1,
        "failed publication must clean its temporary"
    );
}

#[test]
fn publication_failure_cleans_temporary() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out");
    let competing_target = target.clone();
    let _hook = hook(move |stage, _| {
        if stage == SnapshotStage::BeforePublish {
            fs::create_dir(&competing_target).unwrap();
        }
    });
    assert!(write_snapshot(&target, "report", true).is_err());
    assert_eq!(
        entries(dir.path()),
        1,
        "only the competing directory may remain"
    );
}

#[test]
fn creation_errors_do_not_export_paths_or_os_errors() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("synthetic-secret-path").join("out");
    let error = write_snapshot(&target, "report", false).unwrap_err();
    let channels = format!(
        "{error:?} {error} {}",
        serde_json::to_string(&error.to_dto()).unwrap()
    );
    assert!(!channels.contains("synthetic-secret-path"), "{channels}");
    assert!(!channels.contains("os error"), "{channels}");
    assert_eq!(error.exit_code(), 4);
}

#[test]
fn successful_outcome_does_not_export_raw_destination() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("synthetic-secret-path");
    let outcome = write_snapshot(&target, "report", false).unwrap();
    assert!(!format!("{outcome:?}").contains("synthetic-secret-path"));
}

#[cfg(unix)]
#[test]
fn racing_destination_symlink_is_preserved_without_touching_target() {
    for overwrite in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out");
        let victim = dir.path().join("synthetic-victim");
        fs::write(&victim, "preserve me").unwrap();
        let victim_hook = victim.clone();
        let target_hook = target.clone();
        let _hook = hook(move |stage, _| {
            if stage == SnapshotStage::BeforePublish {
                std::os::unix::fs::symlink(&victim_hook, &target_hook).unwrap();
            }
        });
        assert!(write_snapshot(&target, "report", overwrite).is_err());
        assert!(fs::symlink_metadata(target)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(victim).unwrap(), "preserve me");
        assert_eq!(entries(dir.path()), 2);
    }
}

#[test]
fn concurrent_create_new_writers_publish_exactly_one_complete_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|index| {
            let barrier = barrier.clone();
            let target = target.clone();
            std::thread::spawn(move || {
                let report = format!("synthetic-{index}").repeat(4096);
                barrier.wait();
                write_snapshot(&target, &report, false).ok().map(|_| report)
            })
        })
        .collect();
    let winners: Vec<_> = handles
        .into_iter()
        .filter_map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(winners.len(), 1);
    assert_eq!(fs::read_to_string(target).unwrap(), winners[0]);
    assert_eq!(entries(dir.path()), 1);
}

#[test]
fn overwrite_replaces_entry_without_modifying_old_hard_link() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out");
    let old_link = dir.path().join("old");
    fs::write(&target, "old snapshot").unwrap();
    fs::hard_link(&target, &old_link).unwrap();
    let result = write_snapshot(&target, "new snapshot", true).unwrap();
    assert!(result.overwritten);
    assert_eq!(fs::read_to_string(target).unwrap(), "new snapshot");
    assert_eq!(fs::read_to_string(old_link).unwrap(), "old snapshot");
    assert_eq!(entries(dir.path()), 2);
}

#[test]
fn empty_content_is_published_without_temporary_residue() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out");
    let result = write_snapshot(&target, "", false).unwrap();
    assert_eq!(result.bytes, 0);
    assert!(fs::read(target).unwrap().is_empty());
    assert_eq!(entries(dir.path()), 1);
}

#[cfg(unix)]
#[test]
fn fifo_destination_is_rejected_even_with_overwrite() {
    use std::os::unix::fs::FileTypeExt;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("synthetic-fifo");
    assert!(std::process::Command::new("mkfifo")
        .arg(&target)
        .status()
        .unwrap()
        .success());
    let result = write_snapshot(&target, "report", true);
    assert!(result.is_err(), "special files must not be replaced");
    assert!(fs::symlink_metadata(target).unwrap().file_type().is_fifo());
    assert_eq!(entries(dir.path()), 1);
}
