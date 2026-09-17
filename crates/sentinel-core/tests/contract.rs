//! Trait-contract tests run against the real providers of the host OS (`platform::current()`).

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use sentinel_core::SentinelError;
use sentinel_core::model::ProcessIdentity;
use sentinel_core::platform::{self, PlatformConfig, Providers};

fn providers() -> (tempfile::TempDir, Providers) {
    let dir = tempfile::tempdir().expect("temp dir");
    let providers = platform::current(&PlatformConfig {
        data_dir: dir.path().to_path_buf(),
    });
    (dir, providers)
}

fn spawn_sleeper() -> Child {
    #[cfg(windows)]
    let mut command = {
        let mut c = Command::new("ping");
        c.args(["-n", "60", "127.0.0.1"]);
        c
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut c = Command::new("sleep");
        c.arg("60");
        c
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn child")
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn own_process_is_listed_with_matching_identity() {
    let (_dir, mut providers) = providers();
    let own = std::process::id();
    let snapshot = providers.processes.refresh().expect("snapshot");
    let listed = snapshot
        .processes
        .iter()
        .find(|p| p.pid == own)
        .expect("own pid in snapshot");
    let identity = providers.process_control.identity(own).expect("identity");
    assert_eq!(listed.start_time, identity.start_time);
    assert!(listed.memory_rss > 0);
    assert!(!listed.name.is_empty());
    assert!(snapshot.logical_cores >= 1);
    let fresh = providers.processes.lookup(own).expect("lookup");
    assert_eq!(fresh.identity(), identity);
    assert!(fresh.thread_count.is_some_and(|n| n >= 1));
    assert!(fresh.fd_count.is_some());
    let files = providers.processes.open_files(own).expect("own open files");
    assert!(!files.is_empty());
}

#[test]
fn acting_on_own_pid_is_refused() {
    let (_dir, providers) = providers();
    let identity = providers
        .process_control
        .identity(std::process::id())
        .expect("identity");
    let err = providers.process_control.force_kill(&identity).unwrap_err();
    assert!(matches!(err, SentinelError::InvalidInput { .. }), "{err:?}");
}

#[test]
fn dead_pid_reports_process_not_found() {
    let (_dir, mut providers) = providers();
    let mut child = spawn_sleeper();
    let identity = providers
        .process_control
        .identity(child.id())
        .expect("identity");
    child.kill().expect("kill child");
    child.wait().expect("reap child");
    let err = providers.process_control.force_kill(&identity).unwrap_err();
    assert!(
        matches!(err, SentinelError::ProcessNotFound { .. }),
        "{err:?}"
    );
    let err = providers.processes.lookup(identity.pid).unwrap_err();
    assert!(
        matches!(err, SentinelError::ProcessNotFound { .. }),
        "{err:?}"
    );
}

#[test]
fn stale_identity_reports_process_changed() {
    let (_dir, providers) = providers();
    let mut child = spawn_sleeper();
    let identity = providers
        .process_control
        .identity(child.id())
        .expect("identity");
    let stale = ProcessIdentity {
        pid: identity.pid,
        start_time: identity.start_time.saturating_sub(3600),
    };
    let err = providers.process_control.terminate(&stale).unwrap_err();
    assert!(
        matches!(err, SentinelError::ProcessChanged { .. }),
        "{err:?}"
    );
    assert!(matches!(child.try_wait(), Ok(None)), "child must survive");
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn spawned_child_terminates() {
    let (_dir, providers) = providers();
    let mut child = spawn_sleeper();
    let identity = providers
        .process_control
        .identity(child.id())
        .expect("identity");
    assert!(!providers.process_control.has_window(identity.pid));
    providers
        .process_control
        .terminate(&identity)
        .expect("terminate");
    assert!(wait_with_timeout(&mut child, Duration::from_secs(10)));
}

#[test]
fn lowering_priority_of_own_child_succeeds() {
    let (_dir, mut providers) = providers();
    let mut child = spawn_sleeper();
    let identity = providers
        .process_control
        .identity(child.id())
        .expect("identity");
    providers
        .process_control
        .set_priority(&identity, 10)
        .expect("lower priority");
    let info = providers.processes.lookup(identity.pid).expect("lookup");
    #[cfg(unix)]
    assert_eq!(info.nice, Some(10));
    #[cfg(windows)]
    assert_eq!(info.nice, Some(5));
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn resource_samples_are_real() {
    let (_dir, mut providers) = providers();
    let info = providers.resources.system_info();
    assert!(info.logical_cores >= 1);
    assert!(info.total_memory > 0);
    providers.resources.sample().expect("prime");
    std::thread::sleep(Duration::from_millis(300));
    let sample = providers.resources.sample().expect("sample");
    assert_eq!(sample.per_core.len(), info.logical_cores as usize);
    assert!((0.0..=100.0).contains(&sample.cpu_total));
    assert!(sample.memory.total > 0 && sample.memory.used <= sample.memory.total);
    #[cfg(unix)]
    assert!(sample.load.is_some());
    if let Some(thermal) = &sample.thermal {
        assert!(thermal.temperatures.iter().all(|t| t.celsius > 0.0));
    }
}

#[test]
fn permission_probe_reports_platform() {
    let (_dir, providers) = providers();
    let status = providers.permissions.status();
    #[cfg(target_os = "macos")]
    assert_eq!(status.platform, sentinel_core::model::Platform::Macos);
    #[cfg(target_os = "linux")]
    assert_eq!(status.platform, sentinel_core::model::Platform::Linux);
    #[cfg(windows)]
    assert_eq!(status.platform, sentinel_core::model::Platform::Windows);
}
