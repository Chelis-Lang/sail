//! Daemon ownership and preemption controls; workers are explicit protocol fixtures.
#![cfg(unix)]

use sail_api::jobs::{Client, Mode, Request, Response};
use sail_api::preparation::{Status, Submission, VERSION};
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
