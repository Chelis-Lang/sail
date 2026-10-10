//! Fault injection for cached shim sessions (charter, "Testing tiers";
//! ADR 0011; `docs/fault-injection.md`).
//!
//! Each fault must end in the pinned solver's exact replies, or in a failure
//! exit whose output so far is a prefix of them. Never a wrong reply, never a
//! hang, and never a wrong cache entry.
//!
//! Ignored by default; run with
//! `SAIL_TEST_Z3=<pinned z3> cargo nextest run -p sail-shim --run-ignored all`.
#![cfg(unix)]

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use sail_cache::shared::{
    PublicKey, PublishOptions, SecretKey, SharedStore, Solver, add_trusted, publish, sign_entry,
    trust_dir,
};
use sail_key::{QueryKey, SolverIdentity};
use support::{
    DEADLINE, Fault, Op, Proxy, Scratch, Shim, TOY, Want, assert_cache_correct,
    assert_cache_correct_except, assert_equivalent, assert_prefix, copy_dir, direct, entry_files,
    entry_path, identity, kill9, killed, logging_solver, ok_session, replies, session, solver_pids,
    start_daemon, wait_until, z3,
};

/// A short client timeout for the hung-daemon tests.
const SHORT_TIMEOUT: (&str, &str) = ("SAIL_DAEMON_TIMEOUT_MS", "300");

/// A daemon with its cache and a logging solver wrapper.
struct Rig {
    dir: Scratch,
    solver: PathBuf,
    socket: PathBuf,
    cache: PathBuf,
    identity: SolverIdentity,
}

impl Rig {
    fn new(tag: &str) -> Self {
        let dir = Scratch::new(tag);
        let solver = logging_solver(&dir.0, &z3());
        let (socket, cache) = (dir.path("d.sock"), dir.path("cache"));
        start_daemon(&socket, &cache);
        let identity = identity(&socket, &solver);
        Self {
            dir,
            solver,
            socket,
            cache,
            identity,
        }
    }

    fn proxy(&self, fault: Fault) -> (Arc<Proxy>, PathBuf) {
        let at = self.dir.path("p.sock");
        (Proxy::start(&at, &self.socket, fault), at)
    }

    /// The direct-Z3 replies for `input`, keyed for this rig's solver.
    fn want(&self, input: &[u8]) -> Vec<Want> {
        replies(&self.identity, input, &direct(input))
    }

    fn fill(&self, input: &[u8]) {
        let out = ok_session(&self.solver, Some(&self.socket), input);
        assert_equivalent(&self.want(input), &out);
        support::drain(&self.cache);
    }
}

// ---------------------------------------------------------------------------
// 1. The daemon dies mid-session.

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn daemon_gone_before_the_session_falls_back_to_pass_through() {
    let rig = Rig::new("gone");
    let want = rig.want(TOY);
    // No socket at all, and a stale socket file nobody listens on.
    let stale = rig.dir.path("stale.sock");
    drop(std::os::unix::net::UnixListener::bind(&stale));
    for socket in [rig.dir.path("missing.sock"), stale] {
        let out = ok_session(&rig.solver, Some(&socket), TOY);
        assert_equivalent(&want, &out);
    }
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn daemon_dies_while_serving_from_cache() {
    let rig = Rig::new("dserve");
    let want = rig.want(TOY);
    rig.fill(TOY);
    for fault in [
        Fault::Die {
            op: Op::Hello,
            nth: 1,
        },
        Fault::Die {
            op: Op::Get,
            nth: 1,
        },
        Fault::Die {
            op: Op::Get,
            nth: 4,
        },
        Fault::Die {
            op: Op::Get,
            nth: 9,
        },
        Fault::DieMidReply {
            op: Op::Hello,
            nth: 1,
        },
        Fault::DieMidReply {
            op: Op::Get,
            nth: 3,
        },
        Fault::DieMidReply {
            op: Op::Get,
            nth: 7,
        },
    ] {
        let (proxy, at) = rig.proxy(fault);
        let out = ok_session(&rig.solver, Some(&at), TOY);
        assert!(proxy.triggered(), "{fault:?} did not fire");
        assert_equivalent(&want, &out);
    }
    assert!(assert_cache_correct(&rig.cache, &want) > 0);
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn daemon_dies_while_recording_after_a_miss() {
    let rig = Rig::new("drec");
    let want = rig.want(TOY);
    for fault in [
        Fault::Die {
            op: Op::Put,
            nth: 1,
        },
        Fault::Die {
            op: Op::Put,
            nth: 5,
        },
        Fault::Die {
            op: Op::Put,
            nth: 20,
        },
        Fault::DieMidReply {
            op: Op::Put,
            nth: 2,
        },
        Fault::DieMidReply {
            op: Op::Put,
            nth: 12,
        },
    ] {
        // A cold cache each time, so the session records from its first
        // command.
        let _ = std::fs::remove_dir_all(&rig.cache);
        let (proxy, at) = rig.proxy(fault);
        let out = ok_session(&rig.solver, Some(&at), TOY);
        assert!(proxy.triggered(), "{fault:?} did not fire");
        assert_equivalent(&want, &out);
        assert_cache_correct(&rig.cache, &want);
    }
}

// ---------------------------------------------------------------------------
// 2. Corrupted cache entries.

/// A deterministic byte stream for garbage.
fn garbage(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x.to_le_bytes()[0]
        })
        .collect()
}

fn flip(path: &Path, at: impl Fn(usize) -> usize) {
    let mut data = std::fs::read(path).unwrap_or_default();
    let i = at(data.len());
    if let Some(b) = data.get_mut(i) {
        *b ^= 0x20;
    }
    assert!(std::fs::write(path, data).is_ok());
}

/// A copy of entry `data` whose header claims `key`.
fn rekey(data: &[u8], key: &QueryKey) -> Vec<u8> {
    let nl = data.iter().position(|&b| b == b'\n').unwrap_or(data.len());
    let mut meta: serde_json::Value =
        serde_json::from_slice(data.get(..nl).unwrap_or_default()).unwrap_or_default();
    meta["key"] = serde_json::Value::String(key.to_string());
    let mut out = serde_json::to_vec(&meta).unwrap_or_default();
    out.extend_from_slice(data.get(nl..).unwrap_or_default());
    out
}

#[derive(Debug, Clone, Copy)]
enum Corruption {
    FlipReplyByte,
    FlipHeaderByte,
    TruncateHalf,
    TruncateEmpty,
    Swap,
    CopyOtherKeysEntry,
    Garbage,
    /// A well-formed entry for another command's reply, re-headered to claim
    /// this key: passes the integrity check, caught by the shape check.
    ForgedOtherShape,
    /// A directory where the entry file should be.
    Directory,
    /// Garbage at the key of a `check-sat` that answers `unknown` (never
    /// stored), plus stray files the cache never wrote.
    StrayFiles,
}

/// Where to apply corruptions: the stored entries of one session, in order.
struct Target<'a> {
    cache: &'a Path,
    scratch: PathBuf,
    stored: Vec<&'a Want>,
    /// Indices into `stored` of the first `echo` and the first `check-sat`.
    echo_at: usize,
    check_at: usize,
    /// A `check-sat` that answers `unknown`, so is never stored.
    unknown: Option<QueryKey>,
}

impl Target<'_> {
    fn key(&self, i: usize) -> QueryKey {
        self.stored.get(i).map_or(QueryKey([0; 32]), |r| r.key)
    }

    fn path(&self, i: usize) -> PathBuf {
        entry_path(self.cache, &self.key(i))
    }

    /// Keys that hold forged entries after `corruption`.
    fn forged(&self, corruption: Corruption) -> Vec<QueryKey> {
        match corruption {
            Corruption::ForgedOtherShape => vec![self.key(self.echo_at), self.key(self.check_at)],
            _ => Vec::new(),
        }
    }

    /// Applies `corruption` to the `i`th stored entry (`ForgedOtherShape`
    /// always forges the first echo and check-sat entries).
    fn apply(&self, corruption: Corruption, i: usize) {
        let p = self.path(i);
        match corruption {
            Corruption::FlipReplyByte => flip(&p, |n| n - 2),
            Corruption::FlipHeaderByte => flip(&p, |_| 30),
            Corruption::TruncateHalf => {
                let data = std::fs::read(&p).unwrap_or_default();
                let half = data.get(..data.len() / 2).unwrap_or_default();
                assert!(std::fs::write(&p, half).is_ok());
            }
            Corruption::TruncateEmpty => assert!(std::fs::write(&p, b"").is_ok()),
            Corruption::Swap => {
                let (q, tmp) = (self.path(i + 1), self.scratch.join("swap"));
                assert!(std::fs::rename(&p, &tmp).is_ok());
                assert!(std::fs::rename(&q, &p).is_ok());
                assert!(std::fs::rename(&tmp, &q).is_ok());
            }
            Corruption::CopyOtherKeysEntry => assert!(std::fs::copy(self.path(i + 1), &p).is_ok()),
            Corruption::Garbage => assert!(std::fs::write(&p, garbage(200, i as u64)).is_ok()),
            Corruption::ForgedOtherShape => {
                // An echo reply claiming a check-sat key, and vice versa.
                let (src, dst) = (self.path(self.echo_at), self.path(self.check_at));
                let a = std::fs::read(&src).unwrap_or_default();
                let b = std::fs::read(&dst).unwrap_or_default();
                let echo_key = self.key(self.echo_at);
                let check_key = self.key(self.check_at);
                assert!(std::fs::write(&dst, rekey(&a, &check_key)).is_ok());
                assert!(std::fs::write(&src, rekey(&b, &echo_key)).is_ok());
            }
            Corruption::Directory => {
                assert!(std::fs::remove_file(&p).is_ok());
                assert!(std::fs::create_dir(&p).is_ok());
            }
            Corruption::StrayFiles => {
                if let Some(u) = self.unknown {
                    let up = entry_path(self.cache, &u);
                    let _ = std::fs::create_dir_all(up.parent().unwrap_or(self.cache));
                    assert!(std::fs::write(&up, garbage(64, 7)).is_ok());
                }
                let dir = p.parent().unwrap_or(self.cache).to_path_buf();
                assert!(std::fs::write(dir.join(".tmp-1-x"), b"partial").is_ok());
                assert!(std::fs::write(dir.join("not-a-key"), garbage(10, 3)).is_ok());
                let zz = self.cache.join("v2").join("zz");
                let _ = std::fs::create_dir_all(&zz);
                assert!(std::fs::write(zz.join("zz00"), garbage(10, 5)).is_ok());
            }
        }
    }
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn corrupted_entries_are_misses_and_the_session_stays_exact() {
    let rig = Rig::new("corrupt");
    let want = rig.want(TOY);
    rig.fill(TOY);
    let pristine = rig.dir.path("pristine");
    copy_dir(&rig.cache, &pristine);
    // Entries in session order. The early ones are served before the first
    // `unknown` restarts the solver, so faults there hit the serving path;
    // the last ones are met while recording.
    let stored: Vec<&Want> = want
        .iter()
        .filter(|r| entry_path(&rig.cache, &r.key).is_file())
        .collect();
    assert!(stored.len() > 12);
    let first = |p: &[u8]| {
        stored
            .iter()
            .position(|r| r.command.starts_with(p))
            .unwrap_or_default()
    };
    let target = Target {
        cache: &rig.cache,
        scratch: rig.dir.0.clone(),
        echo_at: first(b"(echo"),
        check_at: first(b"(check-sat"),
        unknown: want
            .iter()
            .find(|r| r.command.starts_with(b"(check-sat") && r.reply.starts_with(b"unknown"))
            .map(|r| r.key),
        stored,
    };
    for corruption in [
        Corruption::FlipReplyByte,
        Corruption::FlipHeaderByte,
        Corruption::TruncateHalf,
        Corruption::TruncateEmpty,
        Corruption::Swap,
        Corruption::CopyOtherKeysEntry,
        Corruption::Garbage,
        Corruption::ForgedOtherShape,
        Corruption::Directory,
        Corruption::StrayFiles,
    ] {
        let _ = std::fs::remove_dir_all(&rig.cache);
        copy_dir(&pristine, &rig.cache);
        let at = match corruption {
            Corruption::ForgedOtherShape => vec![0],
            _ => vec![1, 4, target.stored.len() - 2],
        };
        for i in at {
            target.apply(corruption, i);
        }
        // Twice: the first session meets the corruption, the second meets
        // whatever the first left behind.
        for _ in 0..2 {
            let out = ok_session(&rig.solver, Some(&rig.socket), TOY);
            assert_equivalent(&want, &out);
        }
        // A forged entry passes the integrity check and stays on disk (the
        // daemon never overwrites an entry); the shape check keeps it from
        // being served, which the sessions above confirm.
        let forged = target.forged(corruption);
        assert!(
            assert_cache_correct_except(&rig.cache, &want, &forged) > 0,
            "{corruption:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. The solver is killed mid-query.

/// Quick commands, then a marker, then a query Z3 cannot finish (a sum of
/// three cubes).
const QUICK: &[u8] = b"(declare-const x Int)\n(declare-const y Int)\n(declare-const z Int)\n\
(assert (< 0 x))\n(check-sat)\n(get-info :version)\n\
(assert (= (+ (* x x x) (* y y y) (* z z z)) 33))\n(echo \"go\")\n";
const HARD: &[u8] = b"(check-sat)\n";

fn quick_then_hard() -> Vec<u8> {
    [QUICK, HARD].concat()
}

/// Starts a session that reaches the hard query with stdin left open, kills
/// its solver mid-query, and returns how the shim ended.
fn kill_mid_query(solver_dir: &Path, solver: &Path, socket: Option<&Path>) -> support::Ended {
    let before = solver_pids(solver_dir).len();
    let mut shim = Shim::spawn(solver, socket, &[]);
    shim.feed(&quick_then_hard(), false);
    wait_until(DEADLINE, "the marker", || {
        shim.output().ends_with(b"go\n") && solver_pids(solver_dir).len() > before
    });
    // Let the solver replay the prefix and settle into the hard query.
    std::thread::sleep(Duration::from_millis(500));
    for pid in solver_pids(solver_dir).into_iter().skip(before) {
        kill9(pid);
    }
    shim.wait(Duration::from_secs(20))
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn solver_killed_mid_query_in_pass_through() {
    let dir = Scratch::new("kpass");
    let solver = logging_solver(&dir.0, &z3());
    let want = replies(&unkeyed(), &quick_then_hard(), &direct(QUICK));
    let ended = kill_mid_query(&dir.0, &solver, None);
    assert!(
        ended.status.is_some(),
        "the shim hung after its solver died"
    );
    assert!(killed(ended.status), "status {:?}", ended.status);
    assert_prefix(&want, &ended.stdout);
    assert_eq!(support::split(&ended.stdout).len(), want.len() - 1);
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn solver_killed_mid_query_after_a_miss() {
    let rig = Rig::new("kmiss");
    let input = quick_then_hard();
    let want = replies(&rig.identity, &input, &direct(QUICK));
    // Everything but the hard query is cached, so it is the first miss.
    rig.fill(QUICK);
    let ended = kill_mid_query(&rig.dir.0, &rig.solver, Some(&rig.socket));
    assert!(
        ended.status.is_some(),
        "the shim hung after its solver died"
    );
    assert!(killed(ended.status), "status {:?}", ended.status);
    assert_prefix(&want, &ended.stdout);
    assert_eq!(support::split(&ended.stdout).len(), want.len() - 1);
    let hard = want.last().map_or(QueryKey([0; 32]), |r| r.key);
    support::drain(&rig.cache);
    assert!(!entry_path(&rig.cache, &hard).exists());
    // The prefill's complete-input silence record survives independently of
    // the longer killed session; validate it against that exact direct run.
    let mut known = rig.want(QUICK);
    known.extend(want);
    assert_cache_correct(&rig.cache, &known);
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn solver_killed_mid_reply_stores_no_partial_reply() {
    let dir = Scratch::new("kreply");
    let full = direct(TOY);
    // Die after the first model and partway into the echo that follows it.
    // The splitter cannot tell where the model ends until the echo's reply
    // is complete, so the model reply is the one in flight.
    let rs = replies(&unkeyed(), TOY, &full);
    let first_model = rs
        .iter()
        .position(|r| r.command.starts_with(b"(get-model"))
        .unwrap_or_default();
    let cut: usize = rs
        .iter()
        .take(first_model + 1)
        .map(|r| r.reply.len())
        .sum::<usize>()
        + 4;
    assert!(
        full.get(cut - 4..cut) == Some(b"<<DO"),
        "the fixture echoes after its model"
    );
    let solver = support::dying_solver(&dir.0, &z3(), &full, cut);
    let (socket, cache) = (dir.path("d.sock"), dir.path("cache"));
    start_daemon(&socket, &cache);
    let want = replies(&identity(&socket, &solver), TOY, &full);
    let ended = session(&solver, Some(&socket), TOY, &[]);
    assert!(
        ended.status.is_some(),
        "the shim hung after its solver died"
    );
    assert!(killed(ended.status), "status {:?}", ended.status);
    assert_prefix(&want, &ended.stdout);
    // Publication now requires a clean, silent whole session. A killed
    // solver cannot certify that guarantee, even for its earlier replies.
    assert_eq!(assert_cache_correct(&cache, &want), 0);
}

// ---------------------------------------------------------------------------
// 4. Concurrent clients.

/// Session inputs that share prefixes: cuts of the fixture after whole
/// commands, and variants that diverge after a cut.
fn overlapping_inputs() -> Vec<Vec<u8>> {
    let mut items = Vec::new();
    sail_smtlib::split::CommandSplitter::new().push(TOY, &mut items);
    let ends: Vec<usize> = items
        .iter()
        .filter(|i| i.bytes.starts_with(b"(echo"))
        .map(|i| usize::try_from(i.offset + i.bytes.len() as u64).unwrap_or_default() + 1)
        .collect();
    let mut inputs = vec![TOY.to_vec(), TOY.to_vec()];
    for k in [1, 3, 6, 9, 12, 15, 18] {
        let Some(&end) = ends.get(ends.len() * k / 20) else {
            continue;
        };
        let cut = TOY.get(..end).unwrap_or_default();
        inputs.push(cut.to_vec());
        let tail = TOY.get(end..).unwrap_or_default();
        inputs.push([cut, format!("(echo \"variant {k}\")\n").as_bytes(), tail].concat());
    }
    inputs
}

fn concurrently(rig: &Rig, inputs: &[Vec<u8>], wants: &[Vec<Want>]) {
    std::thread::scope(|s| {
        let runs: Vec<_> = inputs
            .iter()
            .map(|input| s.spawn(|| ok_session(&rig.solver, Some(&rig.socket), input)))
            .collect();
        for (run, want) in runs.into_iter().zip(wants) {
            match run.join() {
                Ok(out) => assert_equivalent(want, &out),
                Err(_) => unreachable!("a session thread failed"),
            }
        }
    });
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn concurrent_clients_fill_and_read_one_cache() {
    let rig = Rig::new("conc");
    let inputs = overlapping_inputs();
    assert!(inputs.len() >= 16);
    let wants: Vec<Vec<Want>> = inputs.iter().map(|i| rig.want(i)).collect();
    // Cold, then warm, then a mix of both with a further copy of each.
    concurrently(&rig, &inputs, &wants);
    concurrently(&rig, &inputs, &wants);
    let all: Vec<Want> = wants.iter().flatten().cloned().collect();
    let entries = assert_cache_correct(&rig.cache, &all);
    assert!(entries > 50, "{entries} entries");
    let _ = std::fs::remove_dir_all(&rig.cache);
    let doubled: Vec<Vec<u8>> = inputs.iter().chain(&inputs).cloned().collect();
    let doubled_wants: Vec<Vec<Want>> = wants.iter().chain(&wants).cloned().collect();
    concurrently(&rig, &doubled, &doubled_wants);
    assert!(assert_cache_correct(&rig.cache, &all) > 50);
    assert!(entry_files(&rig.cache).iter().all(|(_, k)| k.is_some()));
}

// ---------------------------------------------------------------------------
// 5. A hung or slow daemon.

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn hung_daemon_falls_back_within_the_client_timeout() {
    let rig = Rig::new("hung");
    let want = rig.want(TOY);
    rig.fill(TOY);
    for fault in [
        Fault::Hang {
            op: Op::Hello,
            nth: 1,
        },
        Fault::Hang {
            op: Op::Get,
            nth: 1,
        },
        Fault::Hang {
            op: Op::Get,
            nth: 6,
        },
        Fault::Delay {
            op: Op::Get,
            nth: 3,
            by: Duration::from_millis(800),
        },
        Fault::Delay {
            op: Op::Hello,
            nth: 1,
            by: Duration::from_millis(800),
        },
    ] {
        let (proxy, at) = rig.proxy(fault);
        let ended = session(&rig.solver, Some(&at), TOY, &[SHORT_TIMEOUT]);
        assert!(proxy.triggered(), "{fault:?} did not fire");
        assert!(
            ended.status.is_some_and(|s| s.success()),
            "{fault:?}: {:?}",
            ended.status
        );
        assert_equivalent(&want, &ended.stdout);
        assert!(
            ended.elapsed < Duration::from_secs(10),
            "{fault:?}: {:?}",
            ended.elapsed
        );
    }
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn hung_daemon_while_recording_does_not_stall_the_session() {
    let rig = Rig::new("hungrec");
    let want = rig.want(TOY);
    for fault in [
        Fault::Hang {
            op: Op::Put,
            nth: 1,
        },
        Fault::Hang {
            op: Op::Put,
            nth: 10,
        },
        Fault::Delay {
            op: Op::Put,
            nth: 4,
            by: Duration::from_millis(800),
        },
    ] {
        let _ = std::fs::remove_dir_all(&rig.cache);
        let (proxy, at) = rig.proxy(fault);
        let ended = session(&rig.solver, Some(&at), TOY, &[SHORT_TIMEOUT]);
        assert!(proxy.triggered(), "{fault:?} did not fire");
        assert!(
            ended.status.is_some_and(|s| s.success()),
            "{fault:?}: {:?}",
            ended.status
        );
        assert_equivalent(&want, &ended.stdout);
        assert!(
            ended.elapsed < Duration::from_secs(10),
            "{fault:?}: {:?}",
            ended.elapsed
        );
        assert_cache_correct(&rig.cache, &want);
    }
}

/// An identity for pairing replies where keys do not matter.
fn unkeyed() -> SolverIdentity {
    SolverIdentity {
        name: "z3".to_owned(),
        version: String::new(),
        binary_sha256: [0; 32],
        platform: String::new(),
    }
}

// ---------------------------------------------------------------------------
// 6. The in-memory tier and cache maintenance (ADR 0015).

/// What the disk tier holds for `key`: the reply of an entry that passes
/// the integrity check, or `None`. Reads without removing anything.
fn disk_reply(cache: &Path, key: &QueryKey) -> Option<Vec<u8>> {
    let data = std::fs::read(entry_path(cache, key)).ok()?;
    sail_cache::parse_entry(&data, &key.to_string())
        .ok()
        .map(|(_, r)| r)
}

fn daemon_stats(socket: &Path) -> sail_api::protocol::StatsSnapshot {
    sail_api::client::Client::connect(socket)
        .and_then(|mut c| c.stats())
        .unwrap_or_default()
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn memory_tier_never_serves_what_the_disk_does_not_hold() {
    let rig = Rig::new("memory");
    let want = rig.want(TOY);
    rig.fill(TOY);
    // Warm: served from memory, exactly.
    let before = daemon_stats(&rig.socket);
    assert_equivalent(&want, &ok_session(&rig.solver, Some(&rig.socket), TOY));
    let warm = daemon_stats(&rig.socket).since(&before);
    assert!(
        warm.memory_hits > 0 && warm.memory_hits == warm.hits,
        "{warm:?}"
    );
    // Change every stored entry under the running daemon: remove it, flip a
    // byte in place, swap it with the next one, or re-header the next one's
    // entry to claim it (a forgery with a valid hash).
    let stored: Vec<QueryKey> = want
        .iter()
        .map(|r| r.key)
        .filter(|k| entry_path(&rig.cache, k).is_file())
        .collect();
    assert!(stored.len() > 12);
    let mut forged = Vec::new();
    let mut i = 0;
    while i < stored.len() {
        let k = stored[i];
        let p = entry_path(&rig.cache, &k);
        let next_key = stored[(i + 1) % stored.len()];
        let next = entry_path(&rig.cache, &next_key);
        match i % 5 {
            0 => assert!(std::fs::remove_file(&p).is_ok()),
            1 => flip(&p, |n| n - 2),
            2 if i + 1 < stored.len() => {
                let tmp = rig.dir.path("swap");
                assert!(std::fs::rename(&p, &tmp).is_ok());
                assert!(std::fs::rename(&next, &p).is_ok());
                assert!(std::fs::rename(&tmp, &next).is_ok());
                // The next entry was changed too; leave it be.
                i += 1;
            }
            3 => {
                let other = std::fs::read(&next).unwrap_or_default();
                assert!(std::fs::write(rig.dir.path("forged"), rekey(&other, &k)).is_ok());
                assert!(std::fs::rename(rig.dir.path("forged"), &p).is_ok());
                forged.push(k);
            }
            _ => {}
        }
        i += 1;
    }
    // Every GET is exactly what the disk tier holds now: the daemon never
    // answers from memory for an entry the disk no longer has.
    let snapshot: Vec<Option<Vec<u8>>> = stored.iter().map(|k| disk_reply(&rig.cache, k)).collect();
    let Ok(mut c) = sail_api::client::Client::connect(&rig.socket) else {
        unreachable!("cannot connect")
    };
    for (k, on_disk) in stored.iter().zip(&snapshot) {
        let got = c.get(&k.0).ok().flatten();
        assert_eq!(&got, on_disk, "GET {k}");
    }
    drop(c);
    // Sessions stay exact (the shim's shape check rejects the forgeries),
    // and whatever they leave on disk is right.
    for _ in 0..2 {
        assert_equivalent(&want, &ok_session(&rig.solver, Some(&rig.socket), TOY));
    }
    assert!(assert_cache_correct_except(&rig.cache, &want, &forged) > 0);
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn gc_and_clear_during_sessions_only_cost_misses() {
    let rig = Rig::new("gc");
    let inputs = overlapping_inputs();
    let wants: Vec<Vec<Want>> = inputs.iter().map(|i| rig.want(i)).collect();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let disk = sail_cache::DiskCache::new(&rig.cache);
    let rounds = std::thread::scope(|s| {
        let maint = s.spawn(|| {
            let mut rounds = 0u32;
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = disk.gc(disk.usage().bytes / 2);
                if rounds % 8 == 7 {
                    let _ = disk.clear();
                }
                let _ = disk.verify(true);
                rounds += 1;
                std::thread::sleep(Duration::from_millis(5));
            }
            rounds
        });
        // Cold, then warm, with entries vanishing underneath.
        concurrently(&rig, &inputs, &wants);
        concurrently(&rig, &inputs, &wants);
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        maint.join().unwrap_or_default()
    });
    assert!(rounds > 0);
    let all: Vec<Want> = wants.iter().flatten().cloned().collect();
    assert_cache_correct(&rig.cache, &all);
    // Once maintenance stops, the cache fills and serves as usual.
    concurrently(&rig, &inputs, &wants);
    assert!(assert_cache_correct(&rig.cache, &all) > 0);
}

// ---------------------------------------------------------------------------
// 7. The shared tier (charter rule 9, ADR 0017).

/// The key the shared-tier tests publish with, and readers trust.
fn publisher() -> SecretKey {
    SecretKey::from_seed([5; 32])
}

/// A rig whose cache holds the toy session, published to `<dir>/store`.
fn published(tag: &str) -> (Rig, PathBuf) {
    let rig = Rig::new(tag);
    rig.fill(TOY);
    let store = rig.dir.path("store");
    let policy = |e: Option<&sail_cache::Effort>| {
        let wire = e.map(|e| sail_api::protocol::Effort {
            limit: e.limit,
            used: e.used,
            reason: e.reason.clone(),
        });
        sail_api::policy::unknown_servable(wire.as_ref())
    };
    let r = publish(
        &sail_cache::DiskCache::new(&rig.cache),
        &SharedStore::new(&store),
        &publisher(),
        &PublishOptions::default(),
        &policy,
    );
    assert!(r.published > 12 && r.failed == 0, "{r:?}");
    (rig, store)
}

/// Starts a second daemon on an empty cache under `rig`'s directory that
/// reads `store` and trusts `trusted`. Returns its socket and cache.
fn reader(rig: &Rig, tag: &str, store: &Path, trusted: &PublicKey) -> (PathBuf, PathBuf) {
    let (socket, cache) = (rig.dir.path(&format!("{tag}.sock")), rig.dir.path(tag));
    assert!(add_trusted(&cache, trusted).is_ok());
    let config = sail_daemon::Config {
        shared: Some(sail_daemon::SharedConfig {
            store: store.to_path_buf(),
            trust_dir: trust_dir(&cache),
            timeout: Duration::from_millis(300),
            backoff: Duration::from_secs(30),
        }),
        ..sail_daemon::Config::default()
    };
    support::start_daemon_with(&socket, &cache, config);
    (socket, cache)
}

/// Every file under `dir`.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(files_under(&p));
        } else {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// A shared entry's header and reply.
fn split_entry(data: &[u8]) -> (serde_json::Value, Vec<u8>) {
    let nl = data.iter().position(|&b| b == b'\n').unwrap_or(0);
    let header = serde_json::from_slice(data.get(..nl).unwrap_or_default()).unwrap_or_default();
    (header, data.get(nl + 1..).unwrap_or_default().to_vec())
}

fn join_entry(header: &serde_json::Value, reply: &[u8]) -> Vec<u8> {
    let mut out = serde_json::to_vec(header).unwrap_or_default();
    out.push(b'\n');
    out.extend_from_slice(reply);
    out
}

/// The opposite answer for a `check-sat` reply: what a forger would plant.
fn wrong_answer(reply: &[u8]) -> Vec<u8> {
    match reply.trim_ascii() {
        b"unsat" => b"sat\n".to_vec(),
        _ => b"unsat\n".to_vec(),
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// How a test rewrites one shared entry: key, header and reply in, new
/// file contents out.
type Rewrite<'a> = &'a dyn Fn(&QueryKey, &serde_json::Value, &[u8]) -> Vec<u8>;

/// Copies `store` to `to`, rewriting every entry with `f`. Returns the
/// number of entries.
fn rewrite_store(store: &Path, to: &Path, f: Rewrite<'_>) -> usize {
    copy_dir(store, to);
    let mut n = 0;
    for path in files_under(to) {
        let data = std::fs::read(&path).unwrap_or_default();
        let (header, reply) = split_entry(&data);
        let key = header["key"]
            .as_str()
            .and_then(sail_cache::maint::parse_key)
            .unwrap_or(QueryKey([0; 32]));
        assert!(std::fs::write(&path, f(&key, &header, &reply)).is_ok());
        n += 1;
    }
    n
}

/// A rewrite that signs wrong `check-sat` answers with `key`, for `solver`.
fn resign(
    key: &SecretKey,
    solver: Solver,
) -> impl Fn(&QueryKey, &serde_json::Value, &[u8]) -> Vec<u8> {
    move |k, h, r| {
        let command = h["command"].as_str().unwrap_or_default().to_owned();
        let reply = if command == "check-sat" {
            wrong_answer(r)
        } else {
            r.to_vec()
        };
        sign_entry(key, k, &reply, &command, &solver, None, 1)
    }
}

/// The solver identity the entries in `store` were signed for.
fn store_solver(store: &Path) -> Solver {
    let data = files_under(store)
        .first()
        .and_then(|p| std::fs::read(p).ok())
        .unwrap_or_default();
    match serde_json::from_value(split_entry(&data).0["solver"].clone()) {
        Ok(s) => s,
        Err(e) => unreachable!("no solver in the store: {e}"),
    }
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn a_verified_shared_store_serves_a_fresh_cache_exactly() {
    let (rig, store) = published("shared-ok");
    let want = rig.want(TOY);
    let (socket, cache) = reader(&rig, "b", &store, &publisher().public());
    let starts = solver_pids(&rig.dir.0).len();
    assert_equivalent(&want, &ok_session(&rig.solver, Some(&socket), TOY));
    let s = daemon_stats(&socket);
    assert!(s.shared_hits > 12 && s.shared_rejected == 0, "{s:?}");
    // The toy session's one `unknown` is not servable (ADR 0009), so it was
    // not published: the session hits up to it, then the solver replays.
    assert_eq!(s.gets, s.shared_hits + 1, "{s:?}");
    assert_eq!(solver_pids(&rig.dir.0).len(), starts + 1);
    // The local copies are correct and say where they came from.
    assert!(assert_cache_correct(&cache, &want) > 12);
    let provenance = format!("shared:{}", publisher().public().id());
    let disk = sail_cache::DiskCache::new(&cache);
    let copied = entry_files(&cache)
        .into_iter()
        .filter_map(|(_, k)| k)
        .filter_map(|k| disk.get(&k))
        .filter(|(m, _)| m.provenance == provenance)
        .count();
    assert!(copied > 12, "{copied}");
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn unverifiable_shared_entries_never_change_an_answer() {
    let began = std::time::Instant::now();
    eprintln!("shared-bad: published/start");
    let (rig, store) = published("shared-bad");
    eprintln!("shared-bad: published/end {:?}", began.elapsed());
    let want = rig.want(TOY);
    eprintln!("shared-bad: direct-want/end {:?}", began.elapsed());
    let trusted = publisher();
    let untrusted = SecretKey::from_seed([6; 32]);
    let local = store_solver(&store);
    let mut other_solver = local.clone();
    other_solver.binary_sha256 = "cd".repeat(32);
    let mut other_platform = local.clone();
    "plan9-mips".clone_into(&mut other_platform.platform);

    // A forger without a key: wrong answers, with valid hashes.
    let forged = |_: &QueryKey, h: &serde_json::Value, r: &[u8]| {
        if h["command"] != "check-sat" {
            return join_entry(h, r);
        }
        let wrong = wrong_answer(r);
        let mut h = h.clone();
        h["reply_sha256"] = hex(&sail_key::sha256(&wrong)).into();
        join_entry(&h, &wrong)
    };
    let by_untrusted = resign(&untrusted, local);
    let for_other_solver = resign(&trusted, other_solver);
    let for_other_platform = resign(&trusted, other_platform);
    let truncated = |_: &QueryKey, h: &serde_json::Value, r: &[u8]| {
        let mut out = join_entry(h, r);
        out.truncate(out.len() / 2);
        out
    };
    let cases: [(&str, Rewrite<'_>); 5] = [
        ("forged", &forged),
        ("untrusted", &by_untrusted),
        ("solver", &for_other_solver),
        ("platform", &for_other_platform),
        ("trunc", &truncated),
    ];
    assert_ne!(trusted.public(), untrusted.public());
    for (tag, f) in cases {
        eprintln!("shared-bad {tag}: rewrite/start {:?}", began.elapsed());
        let bad = rig.dir.path(&format!("store-{tag}"));
        assert!(rewrite_store(&store, &bad, f) > 12);
        eprintln!("shared-bad {tag}: rewrite/end {:?}", began.elapsed());
        let (socket, cache) = reader(&rig, tag, &bad, &trusted.public());
        eprintln!("shared-bad {tag}: reader/end {:?}", began.elapsed());
        for round in 0..2 {
            eprintln!(
                "shared-bad {tag}: session/{round}/start {:?}",
                began.elapsed()
            );
            assert_equivalent(&want, &ok_session(&rig.solver, Some(&socket), TOY));
            eprintln!(
                "shared-bad {tag}: session/{round}/end {:?}",
                began.elapsed()
            );
        }
        let s = daemon_stats(&socket);
        eprintln!("shared-bad {tag}: stats/end {:?}: {s:?}", began.elapsed());
        // The forger left the replies before the first check intact and
        // validly signed; those may hit. Every other case rewrote them all.
        if tag != "forged" {
            assert_eq!(s.shared_hits, 0, "{tag}: {s:?}");
        }
        assert!(s.shared_rejected > 0, "{tag}: {s:?}");
        match tag {
            "solver" => assert!(s.shared_other_solver > 0, "{s:?}"),
            "platform" => assert!(s.shared_other_platform > 0, "{s:?}"),
            _ => {}
        }
        // No forged answer reached the local tier.
        assert_cache_correct(&cache, &want);
        eprintln!("shared-bad {tag}: cache-audit/end {:?}", began.elapsed());
    }
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn a_hung_shared_store_costs_one_timeout_and_answers_stay_exact() {
    let (rig, store) = published("shared-hung");
    let want = rig.want(TOY);
    // Every entry becomes a FIFO: opening one blocks, like a hung mount.
    let hung = rig.dir.path("store-hung");
    copy_dir(&store, &hung);
    for path in files_under(&hung) {
        assert!(std::fs::remove_file(&path).is_ok());
        let made = std::process::Command::new("mkfifo").arg(&path).status();
        assert!(made.is_ok_and(|s| s.success()));
    }
    let (socket, cache) = reader(&rig, "h", &hung, &publisher().public());
    let start = std::time::Instant::now();
    assert_equivalent(&want, &ok_session(&rig.solver, Some(&socket), TOY));
    assert_equivalent(&want, &ok_session(&rig.solver, Some(&socket), TOY));
    assert!(start.elapsed() < DEADLINE);
    let s = daemon_stats(&socket);
    assert_eq!((s.shared_hits, s.shared_timeouts), (0, 1), "{s:?}");
    assert_cache_correct(&cache, &want);
}
