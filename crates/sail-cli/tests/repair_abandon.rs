//! A foreground repair cancels its exact owned job when result polling fails.
#![cfg(unix)]

use std::os::unix::net::UnixListener;
use std::process::Command;
use std::time::{Duration, Instant};

use sail_api::jobs::{Currentness, Lifecycle, Request, Response, Status};

fn fake_service(
    listener: &UnixListener,
    project: std::path::PathBuf,
    unexpected: bool,
) -> Vec<String> {
    let mut status = Status {
        version: 1,
        id: "owned-repair-job".into(),
        project_dir: project,
        args: vec![],
        state: Lifecycle::Queued,
        submitted_at_ms: 1,
        started_at_ms: None,
        finished_at_ms: None,
        exit_code: None,
        input_revision: None,
        currentness: Currentness::Unknown,
        error: None,
    };
    let mut cancellations = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "CLI did not cancel its owned job"
        );
        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(error) => unreachable!("{error}"),
        };
        assert!(
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .is_ok()
        );
        // Darwin can inherit the listener's nonblocking flag on accepted sockets.
        assert!(stream.set_nonblocking(false).is_ok());
        let request = sail_api::jobs::read_frame::<Request>(&mut stream)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let response = match request {
            Request::List { .. } => Response::List(vec![]),
            Request::Capabilities => Response::Capabilities {
                neutral_contracts_version: None,
                assurance_version: None,
                proof_version: 1,
                repair: true,
            },
            Request::Submit { submission } => {
                assert!(matches!(submission.mode, sail_api::jobs::Mode::Repair(_)));
                Response::Status(status.clone())
            }
            Request::Result { id } => {
                assert_eq!(id, status.id);
                if unexpected {
                    Response::List(vec![])
                } else {
                    Response::Error("SAIL0814: original result-read failure".into())
                }
            }
            Request::Cancel { id, superseded } => {
                assert!(!superseded);
                cancellations.push(id);
                // Cleanup failure must not mask the original response error.
                let response = if unexpected {
                    Response::Error("SAIL0815: cleanup unavailable".into())
                } else {
                    status.state = Lifecycle::Cancelled;
                    Response::Status(status)
                };
                assert!(sail_api::jobs::write_frame(&mut stream, &response).is_ok());
                return cancellations;
            }
            _ => unreachable!("unexpected CLI operation"),
        };
        assert!(sail_api::jobs::write_frame(&mut stream, &response).is_ok());
    }
}

#[test]
fn polling_errors_and_unexpected_responses_cancel_only_the_owned_job() {
    for unexpected in [false, true] {
        let root = std::path::Path::new("/tmp").join(format!(
            "sr-abandon-{}-{}",
            std::process::id(),
            u8::from(unexpected)
        ));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let cache_socket = root.join("d.sock");
        let socket = sail_api::jobs::socket_path(&cache_socket);
        let listener = UnixListener::bind(&socket).unwrap_or_else(|error| unreachable!("{error}"));
        assert!(listener.set_nonblocking(true).is_ok());
        let project = root.clone();
        let server = std::thread::spawn(move || fake_service(&listener, project, unexpected));
        let began = Instant::now();
        let result = Command::new(env!("CARGO_BIN_EXE_sail"))
            .args([
                "repair-check",
                "--engine",
                "interactive",
                "--package",
                "demo",
                "--lib",
                "--function",
                "demo::proof",
                "--output",
                "json",
            ])
            .current_dir(&root)
            .env("SAIL_DAEMON_SOCKET", &cache_socket)
            .env("SAIL_CACHE_DIR", root.join("cache"))
            .output()
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(result.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains(if unexpected {
                "unexpected repair response"
            } else {
                "SAIL0814: original result-read failure"
            }),
            "{stderr}"
        );
        assert!(
            !stderr.contains("SAIL0815"),
            "cleanup masked the original error"
        );
        let cancelled = server
            .join()
            .unwrap_or_else(|_| unreachable!("server failed"));
        assert_eq!(cancelled, ["owned-repair-job"]);
        assert!(began.elapsed() < Duration::from_secs(2));
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
