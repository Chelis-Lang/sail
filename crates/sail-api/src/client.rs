//! The thin daemon client used by `sail-z3` (ADR 0011).
//!
//! Every call has timeouts. Any failure is reported as an error, which the
//! shim treats as a miss and falls back to the pinned solver.

use std::io::Write as _;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use crate::protocol::{
    Identity, Info, PutMeta, Request, StatsSnapshot, read_response, split_status_payload,
};

/// Timeout for each request/response exchange.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// Most `PUT`s [`Client::put_all`] sends before reading their
/// acknowledgements. The unread acknowledgements stay far below any socket
/// buffer, so the daemon never blocks writing them while the client writes.
const PUT_WINDOW: usize = 64;

/// A connection to the daemon.
#[derive(Debug)]
pub struct Client {
    stream: UnixStream,
}

impl Client {
    /// Connects to the daemon socket at `path`.
    ///
    /// # Errors
    ///
    /// Connection errors.
    pub fn connect(path: &Path) -> std::io::Result<Self> {
        Self::connect_with_timeout(path, TIMEOUT)
    }

    /// Connects to the daemon socket at `path`, with `timeout` for each
    /// read and write instead of [`TIMEOUT`].
    ///
    /// # Errors
    ///
    /// Connection errors, or a zero `timeout`.
    pub fn connect_with_timeout(path: &Path, timeout: Duration) -> std::io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(Self { stream })
    }

    fn call(&mut self, req: &Request) -> std::io::Result<Vec<u8>> {
        req.write(&mut self.stream)?;
        read_response(&mut self.stream)
    }

    /// The identity of the solver at `solver_path`.
    ///
    /// # Errors
    ///
    /// I/O errors, or the daemon's error message.
    pub fn hello(&mut self, solver_path: &str) -> std::io::Result<Identity> {
        self.hello_in_mode(solver_path, crate::protocol::VerificationMode::Default)
    }

    /// Identifies a solver and negotiates the connection's cache namespace.
    ///
    /// Call once before any cache lookup or store. A failed or repeated
    /// handshake retires the connection; connect again to retry.
    ///
    /// # Errors
    ///
    /// I/O errors, a rejected handshake, or a stale daemon protocol.
    pub fn hello_in_mode(
        &mut self,
        solver_path: &str,
        mode: crate::protocol::VerificationMode,
    ) -> std::io::Result<Identity> {
        let body = self.call(&Request::Hello(crate::protocol::Hello {
            solver_path: solver_path.to_owned(),
            mode,
        }))?;
        let (status, payload) = split_status_payload(&body)?;
        if status == 0 {
            let id: Identity = serde_json::from_slice(payload).map_err(std::io::Error::other)?;
            if id.protocol_version == crate::protocol::PROTOCOL_VERSION {
                Ok(id)
            } else {
                Err(std::io::Error::other(format!(
                    "daemon speaks protocol {}, client {}",
                    id.protocol_version,
                    crate::protocol::PROTOCOL_VERSION
                )))
            }
        } else {
            Err(std::io::Error::other(
                String::from_utf8_lossy(payload).into_owned(),
            ))
        }
    }

    /// The cached reply for `key`, or `None` on a miss.
    /// Requires a successful [`Self::hello`] or [`Self::hello_in_mode`].
    ///
    /// # Errors
    ///
    /// I/O errors.
    pub fn get(&mut self, key: &[u8; 32]) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.get_with_effort(key)?.map(|(reply, _)| reply))
    }

    /// The cached reply for `key` with its recorded effort, if the daemon
    /// sent one, or `None` on a miss.
    ///
    /// # Errors
    ///
    /// I/O errors.
    pub fn get_with_effort(
        &mut self,
        key: &[u8; 32],
    ) -> std::io::Result<Option<(Vec<u8>, Option<crate::protocol::Effort>)>> {
        let body = self.call(&Request::Get(*key))?;
        let (status, payload) = split_status_payload(&body)?;
        if status != 1 {
            return Ok(None);
        }
        Ok(Some((
            payload.to_vec(),
            crate::protocol::get_effort_trailer(&body),
        )))
    }

    /// Stores `reply` for `key`.
    /// Requires a successful [`Self::hello`] or [`Self::hello_in_mode`].
    ///
    /// # Errors
    ///
    /// I/O errors.
    pub fn put(&mut self, key: &[u8; 32], meta: PutMeta, reply: Vec<u8>) -> std::io::Result<()> {
        self.call(&Request::Put {
            key: *key,
            meta,
            reply,
        })
        .map(|_| ())
    }

    /// Stores every `(key, meta, reply)` in order, sending up to a window of
    /// requests before reading their acknowledgements, so a session's stores
    /// cost a few round trips instead of one each. The daemon answers each
    /// request in order, exactly as for [`Client::put`].
    ///
    /// # Errors
    ///
    /// I/O errors. Stores before the failing window may have been made.
    pub fn put_all(
        &mut self,
        puts: impl IntoIterator<Item = ([u8; 32], PutMeta, Vec<u8>)>,
    ) -> std::io::Result<()> {
        let mut puts = puts.into_iter().peekable();
        let mut frames = Vec::new();
        while puts.peek().is_some() {
            frames.clear();
            let mut sent = 0usize;
            for (key, meta, reply) in puts.by_ref().take(PUT_WINDOW) {
                Request::Put { key, meta, reply }.write(&mut frames)?;
                sent += 1;
            }
            (&self.stream).write_all(&frames)?;
            // Only acknowledgements of these requests can arrive, so the
            // buffer never holds bytes past the last one.
            let mut acks = std::io::BufReader::new(&self.stream);
            for _ in 0..sent {
                read_response(&mut acks)?;
            }
        }
        Ok(())
    }

    /// The daemon's counters since it started.
    ///
    /// # Errors
    ///
    /// I/O errors and malformed replies.
    pub fn stats(&mut self) -> std::io::Result<StatsSnapshot> {
        let body = self.call(&Request::Stats)?;
        let (_, payload) = split_status_payload(&body)?;
        serde_json::from_slice(payload).map_err(std::io::Error::other)
    }

    /// The daemon's configuration and state.
    ///
    /// # Errors
    ///
    /// I/O errors and malformed replies. A daemon that predates `INFO`
    /// closes the connection, which is an I/O error.
    pub fn info(&mut self) -> std::io::Result<Info> {
        let body = self.call(&Request::Info)?;
        let (_, payload) = split_status_payload(&body)?;
        serde_json::from_slice(payload).map_err(std::io::Error::other)
    }
}
