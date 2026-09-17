use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use super::*;
use crate::platform::{self, PlatformConfig};

#[derive(Default)]
struct Collect(Mutex<Vec<CoreEvent>>);

impl EventSink for Collect {
    fn emit(&self, event: CoreEvent) {
        self.0.lock().push(event);
    }
}

fn service() -> (Arc<StorageService>, Arc<Collect>, tempfile::TempDir) {
    let data = tempfile::tempdir().unwrap();
    let providers = platform::current(&PlatformConfig {
        data_dir: data.path().to_path_buf(),
    });
    let sink = Arc::new(Collect::default());
    (
        StorageService::new(providers.storage, sink.clone()),
        sink,
        data,
    )
}

fn write(path: &Path, bytes: usize, fill: u8) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![fill; bytes]).unwrap();
}

#[test]
fn scans_tree_with_small_files_links_and_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join("big.bin"), 400 * 1024, 1);
    write(&root.join("small/a.txt"), 10, 2);
    write(&root.join("small/b.txt"), 20, 3);
    write(&root.join("dup1.bin"), 128 * 1024, 7);
    write(&root.join("nested/deeper/dup2.bin"), 128 * 1024, 7);
    write(&root.join("nested/deeper/not-dup.bin"), 128 * 1024, 8);
    write(&root.join("project/package.json"), 2, b'{');
    write(
        &root.join("project/node_modules/lib/index.js"),
        100 * 1024,
        9,
    );
    #[cfg(unix)]
    {
        std::fs::hard_link(root.join("big.bin"), root.join("big-link.bin")).unwrap();
        std::os::unix::fs::symlink(root.join("nested"), root.join("link-to-nested")).unwrap();
    }

    let (service, sink, _data) = service();
    let id = service
        .start_scan(ScanRequest {
            root: root.to_string_lossy().into_owned(),
            cross_mounts: false,
        })
        .unwrap();
    let summary = service.wait_for_scan(&id, Duration::from_secs(30)).unwrap();
    assert_eq!(
        summary.unreadable_entries, 0,
        "{:?}",
        summary.unreadable_samples
    );
    #[cfg(unix)]
    assert_eq!(summary.file_count, 9);
    let canonical = crate::util::strip_verbatim(std::fs::canonicalize(root).unwrap());
    // Which of the two hard-linked names keeps the real size is a deterministic tiebreak
    // (scanner.rs sorts directory entries before choosing), not a contract either name owns —
    // assert the invariant that matters rather than pinning one name to this test run.
    let top = &summary.largest_files[0];
    assert!(
        top.path == canonical.join("big.bin").to_string_lossy()
            || top.path == canonical.join("big-link.bin").to_string_lossy(),
        "expected the largest file to be one of the hard-linked names, got {}",
        top.path
    );
    assert_eq!(top.size_bytes, 400 * 1024);
    #[cfg(unix)]
    {
        let bigs = summary
            .largest_files
            .iter()
            .filter(|f| f.name.starts_with("big"))
            .count();
        assert_eq!(bigs, 1, "hard link counted once");
        assert!(
            !summary
                .largest_files
                .iter()
                .any(|f| f.path.contains("link-to-nested"))
        );
    }
    assert!(
        summary
            .by_extension
            .iter()
            .any(|e| e.extension.as_deref() == Some("bin"))
    );

    let tree = service
        .scan_tree(&TreeQuery {
            scan_id: id.clone(),
            node_id: None,
            depth: 2,
            max_children: 50,
        })
        .unwrap();
    let small = tree
        .children
        .as_ref()
        .unwrap()
        .iter()
        .find(|n| n.name == "small")
        .unwrap();
    let collapsed = &small.children.as_ref().unwrap()[0];
    assert_eq!(collapsed.kind, NodeKind::SmallFiles);
    assert_eq!(collapsed.item_count, 2);
    let project = tree
        .children
        .as_ref()
        .unwrap()
        .iter()
        .find(|n| n.name == "project")
        .unwrap();
    let modules = project
        .children
        .as_ref()
        .unwrap()
        .iter()
        .find(|n| n.name == "node_modules")
        .unwrap();
    assert_eq!(modules.category, Some(CleanupCategory::BuildArtifacts));

    let large = service
        .find_files(
            &id,
            &FileFilter {
                under_path: Some(canonical.join("nested").to_string_lossy().into_owned()),
                min_size_bytes: 100 * 1024,
                unused_for_days: None,
                limit: 10,
            },
        )
        .unwrap();
    assert_eq!(large.len(), 2);
    assert_eq!(
        service.scan_covering(&canonical.join("nested").to_string_lossy()),
        Some(id.clone())
    );

    let job = service.start_duplicate_scan(&id, 64 * 1024).unwrap();
    let report = service
        .wait_for_duplicates(&job, Duration::from_secs(30))
        .unwrap();
    let group = report
        .groups
        .iter()
        .find(|g| g.files.iter().any(|f| f.name == "dup1.bin"))
        .expect("duplicate group");
    assert_eq!(group.files.len(), 2);
    assert_eq!(group.reclaimable_bytes, 128 * 1024);
    assert!(
        !report
            .groups
            .iter()
            .any(|g| g.files.iter().any(|f| f.name == "not-dup.bin"))
    );

    let events = sink.0.lock();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CoreEvent::ScanComplete(s) if s.scan_id == id))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CoreEvent::ScanProgress(p) if p.phase == ScanPhase::Complete))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CoreEvent::DuplicateComplete(r) if r.job_id == job))
    );
}

#[cfg(unix)]
#[test]
fn unreadable_directory_is_counted_not_fatal() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join("ok/file.bin"), 100 * 1024, 1);
    write(&root.join("locked/secret.bin"), 100 * 1024, 2);
    std::fs::set_permissions(root.join("locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let (service, _sink, _data) = service();
    let id = service
        .start_scan(ScanRequest {
            root: root.to_string_lossy().into_owned(),
            cross_mounts: false,
        })
        .unwrap();
    let summary = service.wait_for_scan(&id, Duration::from_secs(30));
    std::fs::set_permissions(root.join("locked"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let summary = summary.unwrap();
    if crate::platform::is_elevated() {
        return;
    }
    assert!(summary.unreadable_entries >= 1);
    assert!(
        summary
            .unreadable_samples
            .iter()
            .any(|p| p.ends_with("locked"))
    );
    let tree = service
        .scan_tree(&TreeQuery {
            scan_id: id,
            node_id: None,
            depth: 1,
            max_children: 10,
        })
        .unwrap();
    assert!(tree.unreadable_entries >= 1);
}

#[test]
fn rejects_missing_roots_and_unknown_ids() {
    let (service, _sink, _data) = service();
    assert!(matches!(
        service.start_scan(ScanRequest {
            root: "/definitely/not/here".into(),
            cross_mounts: false
        }),
        Err(SentinelError::PathNotFound { .. })
    ));
    assert!(matches!(
        service.scan_summary("nope"),
        Err(SentinelError::InvalidInput { .. })
    ));
    assert!(service.cancel_duplicate_scan("nope").is_err());
}

#[test]
fn cancellation_stops_a_scan() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..200 {
        write(&dir.path().join(format!("d{i}/f.txt")), 10, 1);
    }
    let (service, _sink, _data) = service();
    let id = service
        .start_scan(ScanRequest {
            root: dir.path().to_string_lossy().into_owned(),
            cross_mounts: false,
        })
        .unwrap();
    service.cancel_scan(&id).unwrap();
    match service.wait_for_scan(&id, Duration::from_secs(30)) {
        Ok(_) | Err(SentinelError::Cancelled) => {}
        Err(other) => panic!("unexpected {other:?}"),
    }
}
