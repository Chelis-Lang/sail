//! Direct socket reads for read-only job requests; the service authorizes every read.
use crate::run::Cancel;
use sail_api::jobs::{Client, Request, Response};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

/// Reads Status, Result and List over the configured job socket instead of
/// spawning a CLI child. Every call receives a complete fresh frame.
#[derive(Debug, Default)]
pub(super) struct ReadClients {
    #[cfg(test)]
    fixed_socket: Option<PathBuf>,
}

pub(super) enum ReadError {
    Operation(std::io::Error),
    InvalidResponse,
    Cancelled,
}
impl ReadClients {
    /// `None` means "not a direct read, or no service is listening": use the CLI.
    #[cfg_attr(
        not(test),
        expect(clippy::unused_self, reason = "only tests pin the socket")
    )]
    pub(super) fn call(
        &self,
        project: &Path,
        request: &Request,
        cancel: &Cancel,
    ) -> Option<Result<Response, ReadError>> {
        // Writes retain the CLI's existing process-group/cancellation ownership.
        if cancel.is_cancelled() {
            return Some(Err(ReadError::Cancelled));
        }
        if !matches!(
            request,
            Request::Status { .. } | Request::Result { .. } | Request::List { .. }
        ) {
            return None;
        }
        #[cfg(test)]
        let socket = self
            .fixed_socket
            .clone()
            .map_or_else(|| sail_api::jobs::configured_socket(project), Ok);
        #[cfg(not(test))]
        let socket = sail_api::jobs::configured_socket(project);
        let socket = match socket {
            Ok(socket) => socket,
            Err(error) => return Some(Err(ReadError::Operation(error))),
        };
        let mut connected = false;
        let response = Client::new(socket).call_with(request, |stream| {
            connected = true;
            cancel.attach(stream);
        });
        cancel.detach();
        match response {
            Err(error)
                if !connected
                    && matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                    ) =>
            {
                None
            }
            // A reply that arrives after cancellation is dropped.
            _ if cancel.is_cancelled() => Some(Err(ReadError::Cancelled)),
            Err(error) => Some(Err(ReadError::Operation(error))),
            Ok(response) if response_matches(request, &response) => Some(Ok(response)),
            Ok(_) => Some(Err(ReadError::InvalidResponse)),
        }
    }
}

pub(super) fn response_matches(request: &Request, response: &Response) -> bool {
    use sail_api::jobs::VERSION;
    match (request, response) {
        (Request::Status { id }, Response::Status(status)) => {
            status.version == VERSION && status.id == *id
        }
        (Request::Result { id }, Response::Result(document)) => {
            document.job.version == VERSION && document.job.id == *id
        }
        (Request::List { project_dir }, Response::List(jobs)) => jobs.iter().all(|job| {
            job.version == VERSION
                && project_dir
                    .as_ref()
                    .is_none_or(|project| job.project_dir == *project)
        }),
        _ => false,
    }
}
#[cfg(test)]
impl ReadClients {
    pub(super) fn at(socket: PathBuf) -> Self {
        Self {
            fixed_socket: Some(socket),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Socket(PathBuf);
    impl Socket {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "sail-mcp-read-{}-{}.sock",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Socket {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    fn serve(socket: &Socket, replies: Vec<Response>) -> crate::test_io::Server {
        let listener =
            UnixListener::bind(&socket.0).unwrap_or_else(|error| unreachable!("{error}"));
        let mut replies = replies.into_iter();
        crate::test_io::Server::spawn(listener, replies.len(), move |connection| {
            let request: sail_api::jobs::Request = sail_api::jobs::read_frame(connection)
                .unwrap_or_else(|error| unreachable!("{error}"));
            assert!(matches!(request, Request::List { .. }));
            let reply = replies
                .next()
                .unwrap_or_else(|| unreachable!("fixture reply"));
            assert!(sail_api::jobs::write_frame(connection, &reply).is_ok());
        })
        .unwrap_or_else(|error| unreachable!("{error}"))
    }
    #[test]
    fn every_read_receives_a_fresh_frame_and_writes_never_dial() {
        let socket = Socket::new();
        let clients = ReadClients::at(socket.0.clone());
        let request = Request::List { project_dir: None };
        let cancel = Cancel::default();
        let project = Path::new("/");
        let server = serve(
            &socket,
            vec![
                Response::List(vec![]),
                Response::Error("fresh denied".into()),
            ],
        );
        assert!(
            matches!(clients.call(project, &request, &cancel), Some(Ok(Response::List(jobs))) if jobs.is_empty())
        );
        assert!(
            clients
                .call(project, &request, &cancel)
                .is_some_and(|result| result.is_err())
        );
        assert!(server.finish());
        assert!(
            clients
                .call(
                    project,
                    &Request::Cancel {
                        id: "foreign".into(),
                        superseded: false
                    },
                    &cancel
                )
                .is_none()
        );
        assert!(std::fs::remove_file(&socket.0).is_ok());
        // No service listening: fall back to the CLI.
        assert!(clients.call(project, &request, &cancel).is_none());
        cancel.cancel();
        assert!(matches!(
            clients.call(project, &request, &cancel),
            Some(Err(ReadError::Cancelled))
        ));
    }
}
