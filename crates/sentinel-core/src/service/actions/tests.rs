use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use super::*;
use crate::audit::{AuditQuery, OriginFilter};
use crate::model::{ProcessIdentity, ProcessStatus, TerminateMethod};
use crate::provider::FileOps;

#[derive(Default)]
struct FakeWorld {
    processes: Mutex<HashMap<Pid, ProcessInfo>>,
    signals: Mutex<Vec<(Pid, &'static str)>>,
}

impl ActionContext for FakeWorld {
    fn lookup_process(&self, pid: Pid) -> CoreResult<ProcessInfo> {
        self.processes
            .lock()
            .get(&pid)
            .cloned()
            .ok_or(SentinelError::ProcessNotFound { pid })
    }
}

struct FakeControl(Arc<FakeWorld>);

impl ProcessControl for FakeControl {
    fn identity(&self, pid: Pid) -> CoreResult<ProcessIdentity> {
        self.0.lookup_process(pid).map(|p| p.identity())
    }
    fn has_window(&self, pid: Pid) -> bool {
        pid == 500
    }
    fn terminate(&self, target: &ProcessIdentity) -> CoreResult<TerminateMethod> {
        self.0.signals.lock().push((target.pid, "term"));
        self.0.processes.lock().remove(&target.pid);
        Ok(TerminateMethod::Signal)
    }
    fn force_kill(&self, target: &ProcessIdentity) -> CoreResult<()> {
        self.0.signals.lock().push((target.pid, "kill"));
        self.0.processes.lock().remove(&target.pid);
        Ok(())
    }
    fn set_priority(&self, target: &ProcessIdentity, nice: i32) -> CoreResult<()> {
        if let Some(p) = self.0.processes.lock().get_mut(&target.pid) {
            p.nice = Some(nice);
        }
        Ok(())
    }
}

#[derive(Default)]
struct MemoryAudit(Mutex<Vec<AuditEntry>>);

impl AuditStore for MemoryAudit {
    fn append(&self, mut entry: AuditEntry) -> CoreResult<i64> {
        let mut entries = self.0.lock();
        entry.id = entries.len() as i64 + 1;
        entries.push(entry);
        Ok(entries.len() as i64)
    }
    fn query(&self, query: &AuditQuery) -> CoreResult<Vec<AuditEntry>> {
        Ok(self
            .0
            .lock()
            .iter()
            .rev()
            .take(query.limit as usize)
            .cloned()
            .collect())
    }
}

fn process(pid: Pid, name: &str, user: &str) -> ProcessInfo {
    ProcessInfo {
        pid,
        ppid: Some(1),
        name: name.into(),
        cmd: vec![format!("/Applications/{name}")],
        exe: Some(format!("/Applications/{name}")),
        user: Some(user.into()),
        status: ProcessStatus::Sleeping,
        cpu_percent: 3.0,
        cpu_percent_avg: 2.5,
        memory_rss: 250 * 1024 * 1024,
        memory_virtual: 1 << 32,
        start_time: 1_700_000_000,
        run_time_secs: 60,
        thread_count: Some(4),
        fd_count: Some(10),
        nice: Some(0),
    }
}

fn setup() -> (Arc<FakeWorld>, Arc<MemoryAudit>, ActionService) {
    let world = Arc::new(FakeWorld::default());
    {
        let mut procs = world.processes.lock();
        procs.insert(
            std::process::id(),
            process(std::process::id(), "sentinel", "me"),
        );
        procs.insert(400, process(400, "worker", "me"));
        procs.insert(500, process(500, "Safari", "me"));
        procs.insert(600, process(600, "coreaudiod", "_coreaudiod"));
    }
    let audit = Arc::new(MemoryAudit::default());
    let service = ActionService::new(
        world.clone(),
        Arc::new(FakeControl(world.clone())),
        audit.clone(),
        ActionServiceConfig {
            platform: Platform::Macos,
            running_elevated: false,
        },
    );
    (world, audit, service)
}

fn identity(pid: Pid) -> ProcessIdentity {
    ProcessIdentity {
        pid,
        start_time: 1_700_000_000,
    }
}

fn agent_origin() -> Origin {
    Origin::Agent {
        conversation_id: "c1".into(),
        plan_id: "p1".into(),
        provider: "anthropic".into(),
        model: "claude".into(),
        request: "what's eating my CPU".into(),
    }
}

#[test]
fn terminate_preview_restates_target_and_commit_audits() {
    let (world, audit, service) = setup();
    let preview = service
        .prepare(
            Action::TerminateProcess {
                target: identity(400),
            },
            agent_origin(),
        )
        .unwrap();
    assert_eq!(preview.title, "Terminate worker (PID 400)");
    assert!(preview.description.contains("SIGTERM"));
    assert_eq!(preview.targets[0].label, "worker (PID 400)");
    assert_eq!(preview.risk, ActionRisk::Moderate);
    assert!(preview.expires_at_ms > preview.created_at_ms);
    assert!(world.signals.lock().is_empty(), "prepare must not act");

    let outcome = service.commit(&preview.token).unwrap();
    assert_eq!(outcome.status, OutcomeStatus::Succeeded);
    assert_eq!(world.signals.lock().as_slice(), &[(400, "term")]);
    assert_eq!(outcome.after[0].value, 0.0);
    assert!(outcome.summary.contains("Terminated worker (PID 400)"));
    let entries = audit.0.lock();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].trigger.as_deref(), Some("what's eating my CPU"));
    assert_eq!(entries[0].status, AuditStatus::Succeeded);
    assert_eq!(outcome.audit_id, 1);
}

#[test]
fn tokens_cannot_be_reused_or_committed_after_reject() {
    let (world, audit, service) = setup();
    let preview = service
        .prepare(
            Action::ForceKillProcess {
                target: identity(400),
            },
            Origin::User,
        )
        .unwrap();
    assert_eq!(preview.risk, ActionRisk::High);
    service.reject(&preview.token).unwrap();
    assert_eq!(
        service.commit(&preview.token),
        Err(SentinelError::ActionTokenInvalid)
    );
    assert!(world.signals.lock().is_empty());
    let entries = audit.0.lock();
    assert_eq!(entries[0].status, AuditStatus::Rejected);
    assert!(entries[0].summary.starts_with("Declined"));
}

#[test]
fn window_app_and_system_process_wording() {
    let (_world, _audit, service) = setup();
    let quit = service
        .prepare(
            Action::TerminateProcess {
                target: identity(500),
            },
            Origin::User,
        )
        .unwrap();
    assert_eq!(quit.title, "Quit Safari (PID 500)");
    let system = service
        .prepare(
            Action::TerminateProcess {
                target: identity(600),
            },
            Origin::User,
        )
        .unwrap();
    assert!(
        system
            .warnings
            .iter()
            .any(|w| w.starts_with("System process"))
    );
    assert!(
        system
            .warnings
            .iter()
            .any(|w| w.contains("Owned by _coreaudiod"))
    );
    assert!(system.requires_elevation);
}

#[test]
fn refuses_self_special_and_stale_targets() {
    let (world, _audit, service) = setup();
    for pid in [0, 1, std::process::id()] {
        let err = service
            .prepare(
                Action::ForceKillProcess {
                    target: identity(pid),
                },
                Origin::User,
            )
            .unwrap_err();
        assert!(
            matches!(err, SentinelError::InvalidInput { .. }),
            "{pid}: {err:?}"
        );
    }
    let stale = ProcessIdentity {
        pid: 400,
        start_time: 5,
    };
    assert_eq!(
        service.prepare(Action::TerminateProcess { target: stale }, Origin::User),
        Err(SentinelError::ProcessChanged { pid: 400 })
    );
    assert_eq!(
        service.prepare(
            Action::TerminateProcess {
                target: identity(999)
            },
            Origin::User
        ),
        Err(SentinelError::ProcessNotFound { pid: 999 })
    );

    let preview = service
        .prepare(
            Action::TerminateProcess {
                target: identity(400),
            },
            Origin::User,
        )
        .unwrap();
    world.processes.lock().remove(&400);
    let outcome = service.commit(&preview.token).unwrap();
    assert_eq!(outcome.status, OutcomeStatus::Failed);
    assert_eq!(
        outcome.items[0].error.as_ref().map(|e| e.error.clone()),
        Some(SentinelError::ProcessNotFound { pid: 400 })
    );
    assert!(world.signals.lock().is_empty());
}

#[test]
fn priority_preview_and_commit_measure_nice() {
    let (_world, _audit, service) = setup();
    let lower = service
        .prepare(
            Action::SetProcessPriority {
                target: identity(400),
                nice: 10,
            },
            Origin::User,
        )
        .unwrap();
    assert!(!lower.requires_elevation);
    assert!(matches!(
        lower.reversibility,
        Reversibility::Undoable { .. }
    ));
    let outcome = service.commit(&lower.token).unwrap();
    assert_eq!(outcome.after[0].value, 10.0);
    assert!(outcome.summary.contains("from nice 0 to nice 10"));

    let raise = service
        .prepare(
            Action::SetProcessPriority {
                target: identity(400),
                nice: -5,
            },
            Origin::User,
        )
        .unwrap();
    assert!(raise.requires_elevation);
    assert!(
        service
            .prepare(
                Action::SetProcessPriority {
                    target: identity(400),
                    nice: 40
                },
                Origin::User
            )
            .is_err()
    );
    let _ = OriginFilter::Any;
}

struct FakeFiles {
    trash_dir: std::path::PathBuf,
    refuse: Option<String>,
}

impl FileOps for FakeFiles {
    fn trash(&self, path: &std::path::Path) -> CoreResult<()> {
        if self
            .refuse
            .as_deref()
            .is_some_and(|name| path.ends_with(name))
        {
            return Err(SentinelError::PermissionDenied {
                operation: "move to Trash".into(),
                target: Some(path.display().to_string()),
                hint: None,
            });
        }
        let target = self.trash_dir.join(path.file_name().unwrap());
        std::fs::rename(path, target).map_err(|e| SentinelError::io(&e, Some(path)))
    }
    fn move_into(
        &self,
        path: &std::path::Path,
        destination_dir: &std::path::Path,
    ) -> CoreResult<std::path::PathBuf> {
        let target = destination_dir.join(path.file_name().unwrap());
        std::fs::rename(path, &target).map_err(|e| SentinelError::io(&e, Some(path)))?;
        Ok(target)
    }
    fn reveal(&self, _path: &std::path::Path) -> CoreResult<()> {
        Ok(())
    }
}

struct FileWorld {
    files: Arc<FakeFiles>,
}

impl ActionContext for FileWorld {
    fn lookup_process(&self, pid: Pid) -> CoreResult<ProcessInfo> {
        Err(SentinelError::ProcessNotFound { pid })
    }
    fn file_ops(&self) -> CoreResult<Arc<dyn FileOps>> {
        Ok(self.files.clone())
    }
    fn path_size(&self, path: &std::path::Path) -> Option<PathSize> {
        let meta = std::fs::metadata(path).ok()?;
        Some(PathSize {
            bytes: meta.len(),
            items: 1,
            modified: Some(crate::util::now_secs() - 47 * 86_400),
            complete: true,
        })
    }
    fn volume_usage(&self, _path: &std::path::Path) -> CoreResult<(u64, u64)> {
        Ok((1000, 380))
    }
    fn volume_label(&self, _path: &std::path::Path) -> Option<String> {
        Some("Macintosh HD".into())
    }
    fn same_volume(&self, _a: &std::path::Path, _b: &std::path::Path) -> Option<bool> {
        Some(true)
    }
}

fn file_service(refuse: Option<&str>) -> (tempfile::TempDir, Arc<MemoryAudit>, ActionService) {
    let dir = tempfile::tempdir().unwrap();
    let trash_dir = dir.path().join("fake-trash");
    std::fs::create_dir(&trash_dir).unwrap();
    let world = Arc::new(FileWorld {
        files: Arc::new(FakeFiles {
            trash_dir,
            refuse: refuse.map(str::to_owned),
        }),
    });
    let audit = Arc::new(MemoryAudit::default());
    let service = ActionService::new(
        world,
        Arc::new(FakeControl(Arc::new(FakeWorld::default()))),
        audit.clone(),
        ActionServiceConfig {
            platform: Platform::Macos,
            running_elevated: false,
        },
    );
    (dir, audit, service)
}

#[test]
fn trash_preview_and_commit_report_real_outcome() {
    let (dir, audit, service) = file_service(Some("locked.bin"));
    let work = dir.path().join("work");
    std::fs::create_dir_all(work.join("nested")).unwrap();
    let work = std::fs::canonicalize(work).unwrap();
    std::fs::write(work.join("a.bin"), vec![0u8; 3000]).unwrap();
    std::fs::write(work.join("locked.bin"), vec![0u8; 1000]).unwrap();
    std::fs::write(work.join("nested/inner.bin"), vec![0u8; 10]).unwrap();
    let paths = vec![
        work.join("a.bin").display().to_string(),
        work.join("locked.bin").display().to_string(),
        work.join("gone.bin").display().to_string(),
    ];
    let preview = service
        .prepare(Action::TrashPaths { paths }, agent_origin())
        .unwrap();
    assert!(
        preview.title.starts_with("Move 2 items"),
        "{}",
        preview.title
    );
    assert!(preview.description.contains("put back from the Trash"));
    assert_eq!(preview.estimated_bytes_freed, Some(4000));
    assert!(matches!(
        preview.reversibility,
        Reversibility::Recoverable { .. }
    ));
    let gone = preview
        .targets
        .iter()
        .find(|t| t.label.ends_with("gone.bin"))
        .unwrap();
    assert!(gone.problem.is_some());
    assert!(
        preview.targets[0]
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("47 days ago")
    );
    if let Action::TrashPaths { paths } = &preview.action {
        assert_eq!(
            paths.len(),
            2,
            "vanished path is not part of the normalized action"
        );
    }

    let outcome = service.commit(&preview.token).unwrap();
    assert_eq!(outcome.status, OutcomeStatus::PartiallySucceeded);
    assert!(!work.join("a.bin").exists());
    assert!(
        dir.path().join("fake-trash/a.bin").exists(),
        "moved, not unlinked"
    );
    assert!(
        outcome.summary.starts_with("Moved 1 item"),
        "{}",
        outcome.summary
    );
    assert!(
        outcome.summary.contains("Macintosh HD is 62% used"),
        "{}",
        outcome.summary
    );
    assert!(outcome.summary.contains("1 item(s) could not be moved"));
    let entry = &audit.0.lock()[0];
    assert_eq!(entry.status, AuditStatus::PartiallySucceeded);
    assert_eq!(
        entry.affected_paths,
        vec![work.join("a.bin").display().to_string()]
    );
    assert_eq!(entry.bytes_freed, Some(3000));
}

#[test]
fn trash_refuses_protected_paths_and_dedupes_nested() {
    let (dir, _audit, service) = file_service(None);
    assert!(matches!(
        service.prepare(
            Action::TrashPaths {
                paths: vec!["/System".into()]
            },
            Origin::User
        ),
        Err(SentinelError::InvalidInput { .. })
    ));
    assert!(matches!(
        service.prepare(
            Action::TrashPaths {
                paths: vec!["relative/path".into()]
            },
            Origin::User
        ),
        Err(SentinelError::InvalidInput { .. })
    ));
    let folder = dir.path().join("folder");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("x"), b"x").unwrap();
    let preview = service
        .prepare(
            Action::TrashPaths {
                paths: vec![
                    folder.display().to_string(),
                    folder.join("x").display().to_string(),
                ],
            },
            Origin::User,
        )
        .unwrap();
    assert_eq!(preview.targets.len(), 1);
    assert!(preview.warnings.iter().any(|w| w.contains("included once")));
}

#[test]
fn move_preview_validates_destination() {
    let (dir, _audit, service) = file_service(None);
    let source = dir.path().join("report.pdf");
    std::fs::write(&source, b"pdf").unwrap();
    let dest = dir.path().join("archive");
    std::fs::create_dir(&dest).unwrap();
    let preview = service
        .prepare(
            Action::MovePaths {
                paths: vec![source.display().to_string()],
                destination_dir: dest.display().to_string(),
            },
            Origin::User,
        )
        .unwrap();
    assert!(preview.description.contains("same volume"));
    let outcome = service.commit(&preview.token).unwrap();
    assert_eq!(outcome.status, OutcomeStatus::Succeeded);
    assert!(dest.join("report.pdf").exists());
    assert!(matches!(
        service.prepare(
            Action::MovePaths {
                paths: vec![dest.join("report.pdf").display().to_string()],
                destination_dir: dir.path().join("missing").display().to_string(),
            },
            Origin::User
        ),
        Err(SentinelError::PathNotFound { .. })
    ));
}
