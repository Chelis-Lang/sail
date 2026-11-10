//! Daemon ownership and preemption controls; workers are explicit protocol fixtures.
#![cfg(unix)]

use sail_api::jobs::{Client, Mode, Request, Response};
use sail_api::preparation::{Origin, Status, Submission, VERSION};
use std::collections::BTreeMap;
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Fixture {
    root: PathBuf,
    handle: sail_daemon::jobs::JobHandle,
    socket: PathBuf,
}
impl Fixture {
    fn new(script: &str) -> io::Result<Self> {
        Self::with_grace(script, Duration::from_secs(60))
    }
    fn with_grace(script: &str, grace: Duration) -> io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-prep-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("project"))?;
        let worker = root.join("worker");
        std::fs::write(&worker, format!("#!/bin/sh\n{script}\n"))?;
        std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755))?;
        let socket = root.join("jobs.sock");
        let handle = sail_daemon::jobs::bind(&socket, &root.join("cache/projects/jobs"), &worker)?
            .with_preemption_grace(grace)
            .spawn()?;
        Ok(Self {
            root,
            handle,
            socket,
        })
    }
    fn request(&self, project: &str) -> io::Result<Submission> {
        std::fs::create_dir_all(self.root.join(project))?;
        Ok(Submission {
            version: VERSION,
            verification: sail_api::jobs::Submission {
                mode: Mode::Stock,
                project_dir: self.root.join(project),
                args: Vec::new(),
                force: false,
                environment: BTreeMap::from([("PATH".into(), "/usr/bin:/bin".into())]),
                input_revision: None,
            },
            origin: None,
            json: false,
            colour: false,
        })
    }
    /// A preparation that re-runs a verification of `project` that just
    /// succeeded. The fixture worker ignores the origin's contents.
    fn origin_request(&self, project: &str) -> io::Result<Submission> {
        let mut request = self.request(project)?;
        request.origin = Some(Origin {
            key: "key".into(),
            context: "context".into(),
            revision: "a".repeat(64),
            workspace: self.root.join(project),
            readback_revision: "a".repeat(64),
        });
        Ok(request)
    }
    fn call(&self, request: &Request) -> io::Result<Status> {
        match Client::new(&self.socket).call(request)? {
            Response::Preparation(status) => Ok(status),
            _ => Err(io::Error::other("unexpected preparation response")),
        }
    }
    fn status(&self) -> io::Result<Status> {
        self.call(&Request::PreparationStatus)
    }
    fn foreground(&self) -> io::Result<UnixStream> {
        let mut stream = UnixStream::connect(&self.socket)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        sail_api::jobs::write_frame(
            &mut stream,
            &Request::Foreground {
                version: VERSION,
                project_dir: self.root.join("project"),
                args: Vec::new(),
            },
        )?;
        let response: Response = sail_api::jobs::read_frame(&mut stream)?;
        assert!(matches!(
            response,
            Response::Preparation(Status {
                version: VERSION,
                ..
            })
        ));
        Ok(stream)
    }
    fn wait(&self, predicate: impl Fn(&Status) -> bool) -> io::Result<Status> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let status = self.status()?;
            if predicate(&status) {
                return Ok(status);
            }
            assert!(Instant::now() < deadline, "preparation status: {status:?}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.handle.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn foreground_lease_blocks_preparation_and_release_allows_it() -> io::Result<()> {
    let fixture = Fixture::new("printf '%s' '{\"ready\":true,\"origin\":null}'")?;
    let lease = fixture.foreground()?;
    fixture.call(&Request::Prepare(fixture.request("project")?))?;
    std::thread::sleep(Duration::from_millis(650));
    let status = fixture.status()?;
    assert_eq!(status.queued, 1);
    assert!(status.running.is_none());
    assert_eq!(status.ready, 0);
    drop(lease);
    fixture.wait(|status| status.ready == 1)?;
    Ok(())
}

#[test]
fn foreground_preempts_real_running_process_and_shutdown_reaps_it() -> io::Result<()> {
    let fixture = Fixture::new("sleep 30")?;
    fixture.call(&Request::Prepare(fixture.request("project")?))?;
    fixture.wait(|status| status.running.is_some())?;
    let lease = fixture.foreground()?;
    let status = fixture.wait(|status| status.running.is_none())?;
    assert_eq!(status.superseded, 1);
    assert_eq!(status.ready, 0);
    drop(lease);
    Ok(())
}

/// #595: a preempted origin preparation of the caller's workspace may finish
/// within the grace, so it does not leave the compiler target without its
/// build-script export. The lease itself is granted at once.
#[test]
fn foreground_lets_its_workspaces_origin_preparation_finish() -> io::Result<()> {
    let fixture = Fixture::with_grace(
        "sleep 1; printf '%s' '{\"ready\":true,\"origin\":null}'",
        Duration::from_secs(4),
    )?;
    fixture.call(&Request::Prepare(fixture.origin_request("project")?))?;
    fixture.wait(|status| status.running.is_some())?;
    let started = Instant::now();
    let lease = fixture.foreground()?;
    assert!(started.elapsed() < Duration::from_secs(1));
    let status = fixture.wait(|status| status.running.is_none())?;
    assert_eq!(status.ready, 1);
    assert_eq!(status.superseded, 0);
    drop(lease);
    Ok(())
}

/// The grace is a bound: a preempted origin preparation still running when it
/// ends is killed.
#[test]
fn foreground_kills_its_workspaces_origin_preparation_after_the_grace() -> io::Result<()> {
    let grace = Duration::from_millis(500);
    let fixture = Fixture::with_grace("sleep 300", grace)?;
    fixture.call(&Request::Prepare(fixture.origin_request("project")?))?;
    fixture.wait(|status| status.running.is_some())?;
    let preempted = Instant::now();
    let lease = fixture.foreground()?;
    let status = fixture.wait(|status| status.running.is_none())?;
    assert!(preempted.elapsed() >= grace);
    assert_eq!(status.superseded, 1);
    assert_eq!(status.ready, 0);
    drop(lease);
    Ok(())
}

/// Only the caller's own workspace waits: another workspace's origin
/// preparation is killed at once, by a lease or by job admission.
#[test]
fn foreground_kills_another_workspaces_origin_preparation_at_once() -> io::Result<()> {
    let fixture = Fixture::with_grace("sleep 300", Duration::from_secs(600))?;
    fixture.call(&Request::Prepare(fixture.origin_request("other")?))?;
    fixture.wait(|status| status.running.is_some())?;
    let lease = fixture.foreground()?;
    let status = fixture.wait(|status| status.running.is_none())?;
    assert_eq!(status.superseded, 1);
    drop(lease);
    fixture.call(&Request::Prepare(fixture.origin_request("other")?))?;
    fixture.wait(|status| status.running.is_some())?;
    let mut submission = fixture.request("project")?.verification;
    submission.input_revision = Some("a".repeat(64));
    assert!(matches!(
        Client::new(&fixture.socket).call(&Request::Submit { submission })?,
        Response::Status(_)
    ));
    let status = fixture.wait(|status| status.running.is_none())?;
    assert_eq!(status.superseded, 2);
    assert_eq!(status.ready, 0);
    Ok(())
}

#[test]
fn newest_request_replaces_workspace_and_queue_is_bounded() -> io::Result<()> {
    let fixture = Fixture::new("sleep 30")?;
    let lease = fixture.foreground()?;
    fixture.call(&Request::Prepare(fixture.request("project")?))?;
    let replaced = fixture.call(&Request::Prepare(fixture.request("project")?))?;
    assert_eq!(replaced.queued, 1);
    assert_eq!(replaced.superseded, 1);
    for index in 0..63 {
        fixture.call(&Request::Prepare(fixture.request(&format!("p{index}"))?))?;
    }
    assert!(
        fixture
            .call(&Request::Prepare(fixture.request("overflow")?))
            .is_err()
    );
    let mut invalid = fixture.request("invalid")?;
    invalid.version = VERSION + 1;
    assert!(fixture.call(&Request::Prepare(invalid)).is_err());
    drop(lease);
    Ok(())
}

#[test]
fn malformed_worker_output_never_counts_as_ready() -> io::Result<()> {
    let fixture = Fixture::new("printf '%s' '{\"ready\":true'")?;
    fixture.call(&Request::Prepare(fixture.request("project")?))?;
    let status = fixture.wait(|status| status.failed > 0)?;
    assert_eq!(status.ready, 0);
    Ok(())
}

#[test]
fn ordinary_jobs_block_queued_preparation_in_another_workspace() -> io::Result<()> {
    let fixture = Fixture::new(
        "case \"$1\" in _prepare-worker) touch ../prepared ;; *) touch ../ordinary ;; esac\nsleep 30",
    )?;
    let mut submission = fixture.request("project")?.verification;
    submission.input_revision = Some("a".repeat(64));
    assert!(matches!(
        Client::new(&fixture.socket).call(&Request::Submit { submission })?,
        Response::Status(_)
    ));
    fixture.wait(|_| fixture.root.join("ordinary").exists())?;
    fixture.call(&Request::Prepare(fixture.request("other")?))?;
    std::thread::sleep(Duration::from_millis(650));
    let status = fixture.status()?;
    assert_eq!(status.queued, 1);
    assert!(status.running.is_none());
    assert!(!fixture.root.join("prepared").exists());
    Ok(())
}

#[test]
fn ordinary_job_admission_cancels_another_workspaces_running_preparation() -> io::Result<()> {
    let fixture = Fixture::new("sleep 30")?;
    fixture.call(&Request::Prepare(fixture.request("other")?))?;
    fixture.wait(|status| status.running.is_some())?;
    let mut submission = fixture.request("project")?.verification;
    submission.input_revision = Some("a".repeat(64));
    assert!(matches!(
        Client::new(&fixture.socket).call(&Request::Submit { submission })?,
        Response::Status(_)
    ));
    let status = fixture.wait(|status| status.running.is_none())?;
    assert_eq!(status.superseded, 1);
    assert_eq!(status.ready, 0);
    Ok(())
}
