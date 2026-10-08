//! The thin daemon client used by `sail-z3` (ADR 0011).
//!
//! Every call has timeouts. Any failure is reported as an error, which the
//! shim treats as a miss and falls back to the pinned solver.

use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use crate::protocol::{Identity, PutMeta, Request, read_response, split_status_payload};

/// Timeout for each request/response exchange.
pub const TIMEOUT: Duration = Duration::from_secs(10);

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
        let stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
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
        let body = self.call(&Request::Hello(solver_path.to_owned()))?;
        let (status, payload) = split_status_payload(&body)?;
        if status == 0 {
            serde_json::from_slice(payload).map_err(std::io::Error::other)
        } else {
            Err(std::io::Error::other(
                String::from_utf8_lossy(payload).into_owned(),
            ))
        }
    }

    /// The cached reply for `key`, or `None` on a miss.
    ///
    /// # Errors
    ///
    /// I/O errors.
    pub fn get(&mut self, key: &[u8; 32]) -> std::io::Result<Option<Vec<u8>>> {
        let body = self.call(&Request::Get(*key))?;
        let (status, payload) = split_status_payload(&body)?;
        Ok((status == 1).then(|| payload.to_vec()))
    }

    /// Stores `reply` for `key`.
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
}
