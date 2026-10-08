//! The daemon wire protocol: length-prefixed binary frames (ADR 0011).
//!
//! A request is `u32` (big-endian) body length, then the body: one operation
//! byte, then its payload.
//!
//! | Op | Payload | Response |
//! |---|---|---|
//! | 1 `HELLO` | solver path (UTF-8) | `0`, `u32` len, identity JSON; or `1`, `u32` len, error text |
//! | 2 `GET` | 32-byte key | `0` (miss); or `1`, `u32` len, reply bytes |
//! | 3 `PUT` | 32-byte key, `u32` meta len, meta JSON, reply bytes | `0` |
//!
//! Frames larger than [`MAX_FRAME`] are rejected.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

/// Largest accepted frame body, in bytes.
pub const MAX_FRAME: u32 = 256 * 1024 * 1024;

const OP_HELLO: u8 = 1;
const OP_GET: u8 = 2;
const OP_PUT: u8 = 3;

/// Solver identity as the daemon reports it (see `sail_key::SolverIdentity`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// Solver family.
    pub name: String,
    /// Version the solver reports.
    pub version: String,
    /// SHA-256 of the binary, as hex.
    pub binary_sha256: String,
    /// Platform class.
    pub platform: String,
}

/// Solver effort recorded with a `check-sat` reply (ADR 0009).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effort {
    /// The `:rlimit` in force for the check (0 means unlimited).
    pub limit: Option<u64>,
    /// Resource units the check used (`rlimit-count` after minus before).
    pub used: Option<u64>,
    /// The `get-info :reason-unknown` reply, for `unknown` answers.
    pub reason: Option<String>,
}

/// Metadata sent with a `PUT`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PutMeta {
    /// The command the reply answers.
    pub command: String,
    /// Provenance (charter rule 7), for example `pinned-solver`.
    pub provenance: String,
    /// Effort, for `check-sat` replies.
    #[serde(default)]
    pub effort: Option<Effort>,
}

/// A decoded request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Identify the solver at this path.
    Hello(String),
    /// Look up a key.
    Get([u8; 32]),
    /// Store a reply.
    Put {
        /// The key.
        key: [u8; 32],
        /// Metadata.
        meta: PutMeta,
        /// The exact reply bytes.
        reply: Vec<u8>,
    },
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

fn write_frame(w: &mut impl Write, body: &[u8]) -> io::Result<()> {
    let len = u32::try_from(body.len()).map_err(|_| invalid("frame too large"))?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(body)?;
    w.flush()
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_be_bytes(b))
}

fn read_bytes(r: &mut impl Read, len: u32) -> io::Result<Vec<u8>> {
    if len > MAX_FRAME {
        return Err(invalid("frame too large"));
    }
    let mut buf = vec![0u8; usize::try_from(len).map_err(|_| invalid("frame too large"))?];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

impl Request {
    /// Writes this request as one frame.
    ///
    /// # Errors
    ///
    /// I/O errors, or a body larger than `u32::MAX`.
    pub fn write(&self, w: &mut impl Write) -> io::Result<()> {
        let mut body = Vec::new();
        match self {
            Self::Hello(path) => {
                body.push(OP_HELLO);
                body.extend_from_slice(path.as_bytes());
            }
            Self::Get(key) => {
                body.push(OP_GET);
                body.extend_from_slice(key);
            }
            Self::Put { key, meta, reply } => {
                body.push(OP_PUT);
                body.extend_from_slice(key);
                let m = serde_json::to_vec(meta).map_err(io::Error::other)?;
                let len = u32::try_from(m.len()).map_err(|_| invalid("meta too large"))?;
                body.extend_from_slice(&len.to_be_bytes());
                body.extend_from_slice(&m);
                body.extend_from_slice(reply);
            }
        }
        write_frame(w, &body)
    }

    /// Reads one request frame. `Ok(None)` at a clean end of stream.
    ///
    /// # Errors
    ///
    /// I/O errors and malformed frames.
    pub fn read(r: &mut impl Read) -> io::Result<Option<Self>> {
        let len = match read_u32(r) {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        };
        let body = read_bytes(r, len)?;
        let (&op, rest) = body.split_first().ok_or_else(|| invalid("empty frame"))?;
        let key = |b: &[u8]| -> io::Result<[u8; 32]> {
            b.get(..32)
                .and_then(|k| <[u8; 32]>::try_from(k).ok())
                .ok_or_else(|| invalid("short key"))
        };
        match op {
            OP_HELLO => Ok(Some(Self::Hello(
                String::from_utf8(rest.to_vec()).map_err(|_| invalid("path is not UTF-8"))?,
            ))),
            OP_GET => Ok(Some(Self::Get(key(rest)?))),
            OP_PUT => {
                let k = key(rest)?;
                let after = rest.get(32..).ok_or_else(|| invalid("short put"))?;
                let mlen = after
                    .get(..4)
                    .and_then(|b| <[u8; 4]>::try_from(b).ok())
                    .map(u32::from_be_bytes)
                    .ok_or_else(|| invalid("short put"))?;
                let mlen = usize::try_from(mlen).map_err(|_| invalid("bad meta length"))?;
                let meta_bytes = after
                    .get(4..4 + mlen)
                    .ok_or_else(|| invalid("short meta"))?;
                let meta: PutMeta =
                    serde_json::from_slice(meta_bytes).map_err(|_| invalid("bad meta"))?;
                let reply = after.get(4 + mlen..).unwrap_or_default().to_vec();
                Ok(Some(Self::Put {
                    key: k,
                    meta,
                    reply,
                }))
            }
            _ => Err(invalid("unknown operation")),
        }
    }
}

/// Writes a `HELLO` response.
///
/// # Errors
///
/// I/O errors.
pub fn write_hello_response(w: &mut impl Write, result: Result<&Identity, &str>) -> io::Result<()> {
    let (status, payload) = match result {
        Ok(id) => (0u8, serde_json::to_vec(id).map_err(io::Error::other)?),
        Err(msg) => (1u8, msg.as_bytes().to_vec()),
    };
    let mut body = vec![status];
    let len = u32::try_from(payload.len()).map_err(|_| invalid("too large"))?;
    body.extend_from_slice(&len.to_be_bytes());
    body.extend_from_slice(&payload);
    write_frame(w, &body)
}

/// Writes a `GET` response.
///
/// # Errors
///
/// I/O errors.
pub fn write_get_response(w: &mut impl Write, reply: Option<&[u8]>) -> io::Result<()> {
    let mut body = Vec::new();
    match reply {
        None => body.push(0),
        Some(r) => {
            body.push(1);
            let len = u32::try_from(r.len()).map_err(|_| invalid("too large"))?;
            body.extend_from_slice(&len.to_be_bytes());
            body.extend_from_slice(r);
        }
    }
    write_frame(w, &body)
}

/// Writes a `PUT` acknowledgement.
///
/// # Errors
///
/// I/O errors.
pub fn write_put_response(w: &mut impl Write) -> io::Result<()> {
    write_frame(w, &[0])
}

/// Reads one response frame body.
///
/// # Errors
///
/// I/O errors and oversize frames.
pub fn read_response(r: &mut impl Read) -> io::Result<Vec<u8>> {
    let len = read_u32(r)?;
    read_bytes(r, len)
}

/// Splits a `status, u32 len, payload` response body.
///
/// # Errors
///
/// Malformed bodies.
pub fn split_status_payload(body: &[u8]) -> io::Result<(u8, &[u8])> {
    let (&status, rest) = body
        .split_first()
        .ok_or_else(|| invalid("empty response"))?;
    if rest.is_empty() {
        return Ok((status, rest));
    }
    let len = rest
        .get(..4)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(u32::from_be_bytes)
        .ok_or_else(|| invalid("short response"))?;
    let len = usize::try_from(len).map_err(|_| invalid("bad length"))?;
    let payload = rest
        .get(4..4 + len)
        .ok_or_else(|| invalid("short response"))?;
    Ok((status, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip() {
        let reqs = [
            Request::Hello("/opt/z3".to_owned()),
            Request::Get([9; 32]),
            Request::Put {
                key: [7; 32],
                meta: PutMeta {
                    command: "check-sat".to_owned(),
                    provenance: "pinned-solver".to_owned(),
                    effort: Some(Effort {
                        limit: Some(30_000_000),
                        used: Some(1234),
                        reason: None,
                    }),
                },
                reply: b"unsat\n".to_vec(),
            },
        ];
        let mut buf = Vec::new();
        for r in &reqs {
            assert!(r.write(&mut buf).is_ok());
        }
        let mut cur = std::io::Cursor::new(buf);
        for r in &reqs {
            assert_eq!(Request::read(&mut cur).ok().flatten().as_ref(), Some(r));
        }
        assert!(matches!(Request::read(&mut cur), Ok(None)));
    }

    #[test]
    fn malformed_frames_are_errors_not_panics() {
        for bytes in [
            &[0, 0, 0, 1, 9][..],
            &[0, 0, 0, 2, 2, 1][..],
            &[0, 0, 0, 3, 3, 1, 2][..],
            &[0xff, 0xff, 0xff, 0xff][..],
            &[0, 0, 0][..],
        ] {
            let _ = Request::read(&mut std::io::Cursor::new(bytes));
        }
        assert!(Request::read(&mut std::io::Cursor::new(&[0, 0, 0, 1, 9][..])).is_err());
    }
}
