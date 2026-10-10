//! Persistent decoding clients for read-only requests; the service authorizes every read.
use crate::run::Cancel;
use sail_api::jobs::{Client, Request, Response};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// One shared cache family bounds retained/inflight decoded frames across endpoints.
#[derive(Default)]
pub(super) struct ReadClients {
    clients: Mutex<VecDeque<(PathBuf, Client)>>,
    #[cfg(test)]
    fixed_socket: Option<PathBuf>,
}
impl std::fmt::Debug for ReadClients {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReadClients")
            .finish_non_exhaustive()
    }
}
const ENDPOINTS: usize = 8;

pub(super) enum ReadError {
    Operation(std::io::Error),
    InvalidResponse,
    CancelledBefore,
    CancelledRead,
}
impl ReadClients {
    fn client(&self, socket: PathBuf) -> std::io::Result<Client> {
        let mut clients = self
            .clients
            .lock()
            .map_err(|_| std::io::Error::other("SAIL0806: job client table unavailable"))?;
        if let Some(index) = clients.iter().position(|(path, _)| path == &socket) {
            let entry = clients
                .remove(index)
                .ok_or_else(|| std::io::Error::other("SAIL0806: job client table inconsistent"))?;
            let client = entry.1.clone();
            clients.push_back(entry);
            return Ok(client);
        }
        let client = clients.front().map_or_else(
            || Client::with_result_decoding_cache(&socket),
            |(_, family)| family.with_socket(&socket),
        );
        if clients.len() == ENDPOINTS {
            clients.pop_front();
        }
        clients.push_back((socket, client.clone()));
        Ok(client)
    }
    pub(super) fn call(
        &self,
        project: &Path,
        request: &Request,
        cancel: &Cancel,
    ) -> Option<Result<Response, ReadError>> {
        // Writes retain the CLI's existing process-group/cancellation ownership.
        if cancel.is_cancelled() {
            return Some(Err(ReadError::CancelledBefore));
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
        match socket {
            Ok(socket) => self.call_endpoint(socket, request, cancel),
            Err(error) => Some(Err(ReadError::Operation(error))),
        }
    }
    fn call_endpoint(
        &self,
        socket: PathBuf,
        request: &Request,
        cancel: &Cancel,
    ) -> Option<Result<Response, ReadError>> {
        if cancel.is_cancelled() {
            return Some(Err(ReadError::CancelledBefore));
        }
        if !matches!(
            request,
            Request::Status { .. } | Request::Result { .. } | Request::List { .. }
        ) {
            return None;
        }
        let client = match self.client(socket) {
            Ok(client) => client,
            Err(error) => return Some(Err(ReadError::Operation(error))),
        };
        let response = match client.call_cancellable(request, || cancel.is_cancelled()) {
            Ok(response) => response,
            Err(error)
                if error.is_pre_rpc_connect_failure()
                    && matches!(
                        error.io_error().kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                    ) =>
            {
                return None;
            }
            Err(_) if cancel.is_cancelled() => return Some(Err(ReadError::CancelledRead)),
            Err(error) => {
                return Some(Err(ReadError::Operation(std::io::Error::new(
                    error.io_error().kind(),
                    error.io_error().to_string(),
                ))));
            }
        };
        Some(if response_matches(request, &response) {
            Ok(response)
        } else {
            Err(ReadError::InvalidResponse)
        })
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
            clients: Mutex::default(),
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
    fn every_read_receives_fresh_frame_and_restart_is_not_cached_authority() {
        let socket = Socket::new();
        let clients = ReadClients::default();
        let request = Request::List { project_dir: None };
        let cancel = Cancel::default();
        let server = serve(
            &socket,
            vec![
                Response::List(vec![]),
                Response::Error("fresh denied".into()),
            ],
        );
        assert!(
            matches!(clients.call_endpoint(socket.0.clone(),&request,&cancel),Some(Ok(Response::List(jobs))) if jobs.is_empty())
        );
        assert!(
            clients
                .call_endpoint(socket.0.clone(), &request, &cancel)
                .is_some_and(|result| result.is_err())
        );
        assert!(server.finish());
        assert!(std::fs::remove_file(&socket.0).is_ok());
        let server = serve(&socket, vec![Response::List(vec![])]);
        assert!(matches!(
            clients.call_endpoint(socket.0.clone(), &request, &cancel),
            Some(Ok(Response::List(_)))
        ));
        assert!(server.finish());
    }
    #[test]
    fn endpoint_switch_cancellation_and_writes_do_not_leak_or_dial() {
        let a = Socket::new();
        let b = Socket::new();
        let clients = ReadClients::default();
        let request = Request::List { project_dir: None };
        let cancel = Cancel::default();
        let first = serve(&a, vec![Response::List(vec![])]);
        assert!(
            clients
                .call_endpoint(a.0.clone(), &request, &cancel)
                .is_some()
        );
        assert!(first.finish());
        let second = serve(&b, vec![Response::Error("different endpoint".into())]);
        assert!(
            clients
                .call_endpoint(b.0.clone(), &request, &cancel)
                .is_some_and(|result| result.is_err())
        );
        assert!(second.finish());
        cancel.cancel();
        assert!(
            clients
                .call_endpoint(a.0.clone(), &request, &cancel)
                .is_some_and(|result| result.is_err())
        );
        assert!(
            clients
                .call_endpoint(
                    a.0.clone(),
                    &Request::Cancel {
                        id: "foreign".into(),
                        superseded: false
                    },
                    &Cancel::default()
                )
                .is_none()
        );
        for _ in 0..ENDPOINTS + 2 {
            assert!(clients.client(Socket::new().0.clone()).is_ok());
        }
        assert_eq!(
            clients.clients.lock().map(|clients| clients.len()).ok(),
            Some(ENDPOINTS)
        );
        assert!(!format!("{clients:?}").contains(".sock"));
    }
}
