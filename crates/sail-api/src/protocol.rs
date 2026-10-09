//! The daemon wire protocol: length-prefixed binary frames (ADR 0011).
//!
//! A request is `u32` (big-endian) body length, then the body: one operation
//! byte, then its payload.
//!
//! | Op | Payload | Response |
//! |---|---|---|
//! | 1 `HELLO` | [`Hello`] JSON (path and mode) | `0`, `u32` len, identity JSON; or `1`, `u32` len, error text |
//! | 2 `GET` | 32-byte key | `0` (miss); or `1`, `u32` len, reply bytes, then optionally `u32` len, effort JSON |
//! | 3 `PUT` | 32-byte key, `u32` meta len, meta JSON, reply bytes | `0` |
//! | 4 `STATS` | empty | `0`, `u32` len, [`StatsSnapshot`] JSON |
//! | 5 `INFO` | empty | `0`, `u32` len, [`Info`] JSON |
//!
//! Operations 4 and 5 are read-only and additive: they kept
//! the protocol at 1 (ADR 0012, ADR 0015). Version 4 requires silent-session publication and negotiates the mode
//! in HELLO (ADR 0020). A daemon that predates
//! one rejects it by closing that connection; nothing else changes.
//!
//! Frames larger than [`MAX_FRAME`] are rejected.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

/// The protocol version. Bump on any incompatible change; the daemon reports
/// it in `HELLO`, clients refuse a mismatch, and the default socket name
/// includes it, so an upgraded client never talks to a stale daemon.
pub const PROTOCOL_VERSION: u32 = 4;

/// A cache namespace negotiated for one solver connection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationMode {
    /// Exact default-mode replies.
    #[default]
    Default,
    /// Complete verification attempts in the seed portfolio.
    AcceptFirstProof,
}

impl VerificationMode {
    /// The stable namespace name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptFirstProof => "accept-first-proof",
        }
    }
}

/// A protocol-v4 solver handshake, including the cache namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// The exact solver executable to identify.
    pub solver_path: String,
    /// The namespace used for every subsequent lookup and store.
    pub mode: VerificationMode,
}

/// Largest accepted frame body, in bytes.
pub const MAX_FRAME: u32 = 256 * 1024 * 1024;

const OP_HELLO: u8 = 1;
const OP_GET: u8 = 2;
const OP_PUT: u8 = 3;
const OP_STATS: u8 = 4;
const OP_INFO: u8 = 5;

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
    /// The daemon's protocol version ([`PROTOCOL_VERSION`]).
    #[serde(default)]
    pub protocol_version: u32,
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

/// The daemon's counters since it started (the `STATS` operation). Benchmarks
/// read them before and after a run and report the difference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsSnapshot {
    /// `HELLO` requests: one per cached shim session.
    pub hellos: u64,
    /// `GET` requests. A session stops asking at its first miss.
    pub gets: u64,
    /// `GET` requests answered from the cache.
    pub hits: u64,
    /// `PUT` requests: replies recorded from the solver.
    pub puts: u64,
    /// `GET` hits served from the in-memory tier (included in `hits`).
    #[serde(default)]
    pub memory_hits: u64,
    /// `PUT`s dropped because the write-behind queue stayed full. Each
    /// costs a future miss, never correctness.
    #[serde(default)]
    pub dropped_puts: u64,
    /// Local misses looked up in the shared store (ADR 0017).
    #[serde(default)]
    pub shared_lookups: u64,
    /// Shared entries that passed every check and were served (included
    /// in `hits`).
    #[serde(default)]
    pub shared_hits: u64,
    /// Shared entries that failed a check, including the two counts below.
    /// Each is a miss.
    #[serde(default)]
    pub shared_rejected: u64,
    /// Shared entries signed for another solver: refused.
    #[serde(default)]
    pub shared_other_solver: u64,
    /// Shared entries signed for this solver on another platform class:
    /// refused (ADR 0010).
    #[serde(default)]
    pub shared_other_platform: u64,
    /// Shared lookups that timed out.
    #[serde(default)]
    pub shared_timeouts: u64,
    /// Local misses not looked up in the shared store: backing off after a
    /// timeout, a recent miss for the key, or too many reads in flight.
    #[serde(default)]
    pub shared_skipped: u64,
}

impl StatsSnapshot {
    /// The counters accumulated between `earlier` and `self`.
    #[must_use]
    pub fn since(&self, earlier: &Self) -> Self {
        Self {
            hellos: self.hellos.saturating_sub(earlier.hellos),
            gets: self.gets.saturating_sub(earlier.gets),
            hits: self.hits.saturating_sub(earlier.hits),
            puts: self.puts.saturating_sub(earlier.puts),
            memory_hits: self.memory_hits.saturating_sub(earlier.memory_hits),
            dropped_puts: self.dropped_puts.saturating_sub(earlier.dropped_puts),
            shared_lookups: self.shared_lookups.saturating_sub(earlier.shared_lookups),
            shared_hits: self.shared_hits.saturating_sub(earlier.shared_hits),
            shared_rejected: self.shared_rejected.saturating_sub(earlier.shared_rejected),
            shared_other_solver: self
                .shared_other_solver
                .saturating_sub(earlier.shared_other_solver),
            shared_other_platform: self
                .shared_other_platform
                .saturating_sub(earlier.shared_other_platform),
            shared_timeouts: self.shared_timeouts.saturating_sub(earlier.shared_timeouts),
            shared_skipped: self.shared_skipped.saturating_sub(earlier.shared_skipped),
        }
    }
}

/// The in-memory tier's state, as `INFO` reports it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Budget in bytes (0: the tier is off).
    pub capacity_bytes: u64,
    /// Replies held.
    pub entries: u64,
    /// Bytes charged for them.
    pub bytes: u64,
    /// Replies held whose disk write is still queued.
    pub pending: u64,
}

/// The write-behind queue's state, as `INFO` reports it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueInfo {
    /// Budget in bytes.
    pub capacity_bytes: u64,
    /// Writes queued or in progress.
    pub entries: u64,
    /// Their reply bytes.
    pub bytes: u64,
}

/// The daemon's configuration and state (the `INFO` operation).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// The daemon's protocol version ([`PROTOCOL_VERSION`]).
    pub protocol_version: u32,
    /// The daemon's sail version.
    pub sail_version: String,
    /// The daemon's process id.
    pub pid: u32,
    /// Seconds since the daemon started.
    pub uptime_secs: u64,
    /// The socket it serves.
    pub socket: String,
    /// The cache root it serves.
    pub cache_root: String,
    /// Its log file, if it has one.
    #[serde(default)]
    pub log: Option<String>,
    /// Seconds without connections after which it stops (0: never).
    pub idle_timeout_secs: u64,
    /// The in-memory tier.
    pub memory: MemoryInfo,
    /// The write-behind queue.
    pub queue: QueueInfo,
    /// The shared tier, if one is configured (ADR 0017).
    #[serde(default)]
    pub shared: Option<SharedInfo>,
}

/// The shared tier's configuration, as `INFO` reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedInfo {
    /// The shared store's directory.
    pub store: String,
    /// The directory of trusted public keys.
    pub trust_dir: String,
    /// Ids of the keys trusted now.
    pub trusted_keys: Vec<String>,
    /// How long one lookup may take, in milliseconds.
    pub timeout_ms: u64,
}

/// A decoded request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Identify the solver at this path.
    Hello(Hello),
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
    /// Report the daemon's counters.
    Stats,
    /// Report the daemon's configuration and state.
    Info,
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
            Self::Hello(hello) => {
                body.push(OP_HELLO);
                body.extend_from_slice(&serde_json::to_vec(hello).map_err(io::Error::other)?);
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
            Self::Stats => body.push(OP_STATS),
            Self::Info => body.push(OP_INFO),
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
                serde_json::from_slice(rest).map_err(|_| invalid("bad solver handshake"))?,
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
            OP_STATS => Ok(Some(Self::Stats)),
            OP_INFO => Ok(Some(Self::Info)),
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
pub fn write_get_response(
    w: &mut impl Write,
    reply: Option<(&[u8], Option<&Effort>)>,
) -> io::Result<()> {
    let mut body = Vec::new();
    match reply {
        None => body.push(0),
        Some((r, effort)) => {
            body.push(1);
            let len = u32::try_from(r.len()).map_err(|_| invalid("too large"))?;
            body.extend_from_slice(&len.to_be_bytes());
            body.extend_from_slice(r);
            // Optional trailer: the entry's effort as JSON. Clients that do
            // not know it ignore bytes after the reply.
            if let Some(e) = effort {
                let json = serde_json::to_vec(e).map_err(io::Error::other)?;
                let len = u32::try_from(json.len()).map_err(|_| invalid("too large"))?;
                body.extend_from_slice(&len.to_be_bytes());
                body.extend_from_slice(&json);
            }
        }
    }
    write_frame(w, &body)
}

/// The effort trailer of a `GET` hit body, if present.
#[must_use]
pub fn get_effort_trailer(body: &[u8]) -> Option<Effort> {
    let reply_len = usize::try_from(u32::from_be_bytes(
        <[u8; 4]>::try_from(body.get(1..5)?).ok()?,
    ))
    .ok()?;
    let rest = body.get(5 + reply_len..)?;
    let len = usize::try_from(u32::from_be_bytes(
        <[u8; 4]>::try_from(rest.get(..4)?).ok()?,
    ))
    .ok()?;
    serde_json::from_slice(rest.get(4..4 + len)?).ok()
}

/// Writes a `STATS` response.
///
/// # Errors
///
/// I/O errors.
pub fn write_stats_response(w: &mut impl Write, stats: &StatsSnapshot) -> io::Result<()> {
    let payload = serde_json::to_vec(stats).map_err(io::Error::other)?;
    let mut body = vec![0u8];
    let len = u32::try_from(payload.len()).map_err(|_| invalid("too large"))?;
    body.extend_from_slice(&len.to_be_bytes());
    body.extend_from_slice(&payload);
    write_frame(w, &body)
}

/// Writes an `INFO` response.
///
/// # Errors
///
/// I/O errors.
pub fn write_info_response(w: &mut impl Write, info: &Info) -> io::Result<()> {
    let payload = serde_json::to_vec(info).map_err(io::Error::other)?;
    let mut body = vec![0u8];
    let len = u32::try_from(payload.len()).map_err(|_| invalid("too large"))?;
    body.extend_from_slice(&len.to_be_bytes());
    body.extend_from_slice(&payload);
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
            Request::Hello(Hello {
                solver_path: "/opt/z3".to_owned(),
                mode: VerificationMode::Default,
            }),
            Request::Hello(Hello {
                solver_path: "/opt/z3".to_owned(),
                mode: VerificationMode::AcceptFirstProof,
            }),
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
            Request::Stats,
            Request::Info,
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
    fn stats_response_round_trips() {
        let stats = StatsSnapshot {
            hellos: 1,
            gets: 5,
            hits: 4,
            puts: 3,
            memory_hits: 2,
            dropped_puts: 0,
            ..StatsSnapshot::default()
        };
        let mut buf = Vec::new();
        assert!(write_stats_response(&mut buf, &stats).is_ok());
        let body = read_response(&mut std::io::Cursor::new(buf)).unwrap_or_default();
        let parsed = split_status_payload(&body)
            .ok()
            .and_then(|(_, p)| serde_json::from_slice::<StatsSnapshot>(p).ok());
        assert_eq!(parsed, Some(stats));
        assert_eq!(stats.since(&StatsSnapshot { gets: 2, ..stats }).gets, 3);
        assert_eq!(
            StatsSnapshot::default().since(&stats),
            StatsSnapshot::default()
        );
    }

    #[test]
    fn info_round_trips_and_old_stats_json_reads() {
        let info = Info {
            protocol_version: PROTOCOL_VERSION,
            sail_version: "0.0.0".to_owned(),
            pid: 7,
            uptime_secs: 3,
            socket: "/tmp/d.sock".to_owned(),
            cache_root: "/tmp/c".to_owned(),
            log: None,
            idle_timeout_secs: 1800,
            memory: MemoryInfo::default(),
            queue: QueueInfo::default(),
            shared: Some(SharedInfo {
                store: "/srv/sail".to_owned(),
                trust_dir: "/tmp/c/trust".to_owned(),
                trusted_keys: vec!["0123456789abcdef".to_owned()],
                timeout_ms: 250,
            }),
        };
        let mut buf = Vec::new();
        assert!(write_info_response(&mut buf, &info).is_ok());
        let body = read_response(&mut std::io::Cursor::new(buf)).unwrap_or_default();
        let parsed = split_status_payload(&body)
            .ok()
            .and_then(|(_, p)| serde_json::from_slice::<Info>(p).ok());
        assert_eq!(parsed, Some(info));
        // A daemon from before the per-tier counters.
        let old: Option<StatsSnapshot> =
            serde_json::from_str(r#"{"hellos":1,"gets":2,"hits":1,"puts":1}"#).ok();
        assert_eq!(old.map(|s| s.memory_hits), Some(0));
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
