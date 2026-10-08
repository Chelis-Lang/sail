//! Cached sessions with a lazily started solver (ADR 0011).
//!
//! The shim answers each output-producing command from the daemon's cache,
//! keyed by the v2 prefix key, without starting a solver. On the first miss,
//! or anything unexpected, it starts the pinned Z3 and writes the *entire*
//! input received so far to it, so the solver's history is exactly what
//! plain Verus would have given it. It discards the solver's replies to the
//! commands already served from cache, and from then on forwards the
//! solver's output byte for byte. While the solver runs, replies are paired
//! with their commands and stored in the cache.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, ExitCode, Stdio};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use sail_api::client::Client;
use sail_api::protocol::{Identity, PutMeta};
use sail_key::{Mode, PrefixHasher, QueryKey, SolverIdentity};
use sail_session::Session;
use sail_smtlib::command::{Output, classify};
use sail_smtlib::split::{CommandSplitter, Item, ItemKind, ResponseSplitter};

/// What the output side should do with the next reply.
enum Expect {
    /// Store the reply under this key.
    Store {
        key: QueryKey,
        command: String,
        /// The command's exact bytes, for the shape check.
        text: Vec<u8>,
    },
    /// Pairing can no longer be trusted: stop recording.
    Stop,
}

fn parse_identity(id: &Identity) -> Option<SolverIdentity> {
    let mut sha = [0u8; 32];
    let hex = id.binary_sha256.as_bytes();
    if hex.len() != 64 {
        return None;
    }
    for (i, byte) in sha.iter_mut().enumerate() {
        let pair = std::str::from_utf8(hex.get(2 * i..2 * i + 2)?).ok()?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(SolverIdentity {
        name: id.name.clone(),
        version: id.version.clone(),
        binary_sha256: sha,
        platform: id.platform.clone(),
    })
}

fn write_out(bytes: &[u8]) -> bool {
    let mut out = std::io::stdout().lock();
    out.write_all(bytes).and_then(|()| out.flush()).is_ok()
}

/// Whether `reply` has the shape the solver gives for `command`. This is
/// defence in depth against any pairing or storage fault: a mismatch is
/// treated as a miss and the session falls back to the solver.
fn plausible(command: &[u8], reply: &[u8]) -> bool {
    use sail_smtlib::command::CommandKind;
    let Ok(c) = classify(command) else {
        return false;
    };
    let body = reply.trim_ascii();
    match c.kind {
        CommandKind::Echo => {
            echo_text(command).is_some_and(|t| reply == [t.as_slice(), b"\n"].concat())
        }
        k if k.is_check() => matches!(body, b"sat" | b"unsat" | b"unknown"),
        CommandKind::GetInfo {
            keyword: b":version",
        } => body.starts_with(b"(:version "),
        CommandKind::GetInfo {
            keyword: b":reason-unknown",
        } => body.starts_with(b"(:reason-unknown "),
        CommandKind::GetInfo {
            keyword: b":all-statistics",
        } => body.starts_with(b"(:") && body.ends_with(b")"),
        _ => !body.is_empty() && !body.starts_with(b"(error"),
    }
}

/// The text `(echo "...")` prints: the string literal's contents, with `""`
/// unescaped to `"`.
fn echo_text(command: &[u8]) -> Option<Vec<u8>> {
    let start = command.iter().position(|&b| b == b'"')? + 1;
    let end = command.iter().rposition(|&b| b == b'"')?;
    (end >= start).then(|| {
        let inner = command.get(start..end).unwrap_or_default();
        let mut out = Vec::with_capacity(inner.len());
        let mut i = 0;
        while let Some(&b) = inner.get(i) {
            out.push(b);
            i += if b == b'"' && inner.get(i + 1) == Some(&b'"') {
                2
            } else {
                1
            };
        }
        out
    })
}

/// A cached reply that must not be served: implausible for its command, or a
/// `check-sat` answer of `unknown` (ADR 0009 treats every cached `unknown`
/// as unstable for now).
fn unservable(command: &[u8], reply: &[u8]) -> bool {
    !plausible(command, reply)
        || (classify(command).is_ok_and(|c| c.kind.is_check())
            && reply.trim_ascii_start().starts_with(b"unknown"))
}

/// Pairs solver replies with the expectations the input side queues, in
/// order, whichever arrives first. Recording stops for good at an `Expect::Stop`,
/// a solver error reply, or [`Pairer::stop`].
#[derive(Default)]
struct Pairer {
    replies: VecDeque<Vec<u8>>,
    expects: VecDeque<Expect>,
    stopped: bool,
}

impl Pairer {
    fn reply(&mut self, reply: Vec<u8>) {
        self.replies.push_back(reply);
    }

    fn expect(&mut self, expect: Expect) {
        self.expects.push_back(expect);
    }

    fn stop(&mut self) {
        self.stopped = true;
    }

    /// Every pair that can be formed now and should be stored.
    fn drain(&mut self) -> Vec<(QueryKey, String, Vec<u8>)> {
        let mut out = Vec::new();
        // Pop only when both sides have an entry: popping one and finding the
        // other empty would drop it and shift every later pairing by one.
        while !self.stopped && !self.replies.is_empty() && !self.expects.is_empty() {
            let (Some(reply), Some(expect)) = (self.replies.pop_front(), self.expects.pop_front())
            else {
                break;
            };
            match expect {
                Expect::Stop => self.stopped = true,
                Expect::Store { .. } if reply.starts_with(b"(error") => self.stopped = true,
                Expect::Store { command, .. }
                    if command == "check-sat"
                        && reply.trim_ascii_start().starts_with(b"unknown") => {}
                // Never store a reply that does not fit its command; stop
                // recording, since pairing can no longer be trusted.
                Expect::Store { ref text, .. } if !plausible(text, &reply) => self.stopped = true,
                Expect::Store { key, command, .. } => out.push((key, command, reply)),
            }
        }
        out
    }
}

struct Solving {
    child: Child,
    stdin: Option<ChildStdin>,
    expects: Sender<Expect>,
    output: Option<JoinHandle<()>>,
}

/// The output side once a solver runs: forwards replies after the first
/// `skip`, and records paired replies.
fn output_side(mut from: impl Read, skip: u64, expects: &Receiver<Expect>, mut client: Client) {
    let mut all = Vec::new(); // every output byte so far
    let mut splitter = ResponseSplitter::new();
    let mut forward_from: Option<usize> = (skip == 0).then_some(0);
    let mut forwarded = 0usize;
    let mut seen = 0u64; // replies completed so far
    let mut prev: Option<(u64, usize)> = None; // (index, start) of the last completed reply
    let mut buf = vec![0u8; 64 * 1024];

    // Pairs replies with expectations as either arrives; stores the pairs.
    let mut pairer = Pairer::default();
    let mut store = |pairer: &mut Pairer, wait: bool| {
        if wait {
            for e in expects {
                pairer.expect(e);
            }
        } else {
            for e in expects.try_iter() {
                pairer.expect(e);
            }
        }
        for (key, command, reply) in pairer.drain() {
            let meta = PutMeta {
                command,
                provenance: "pinned-solver".to_owned(),
            };
            if client.put(&key.0, meta, reply).is_err() {
                pairer.stop();
            }
        }
    };

    loop {
        let n = match from.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let chunk = buf.get(..n).unwrap_or_default();
        all.extend_from_slice(chunk);
        let mut items: Vec<Item> = Vec::new();
        splitter.push(chunk, &mut items);
        for item in items {
            let start = usize::try_from(item.offset).unwrap_or(usize::MAX);
            if let Some((k, s)) = prev.take()
                && k >= skip
            {
                pairer.reply(all.get(s..start).unwrap_or_default().to_vec());
            }
            if seen == skip && forward_from.is_none() {
                forward_from = Some(start);
            }
            prev = Some((seen, start));
            seen += 1;
        }
        if let Some(f) = forward_from
            && all.len() > forwarded.max(f)
        {
            if !write_out(all.get(forwarded.max(f)..).unwrap_or_default()) {
                return;
            }
            forwarded = all.len();
        }
        store(&mut pairer, false);
    }
    // End of output: the last reply runs to the end of the stream.
    if let Some((k, s)) = prev.take()
        && k >= skip
    {
        pairer.reply(all.get(s..).unwrap_or_default().to_vec());
    }
    if forward_from.is_none() {
        // Fewer replies than were already served: forward nothing more.
    } else if all.len() > forwarded {
        let _ = write_out(all.get(forwarded..).unwrap_or_default());
    }
    store(&mut pairer, true);
}

fn start_solver(
    solver: &Path,
    args: &[std::ffi::OsString],
    raw: &[u8],
    skip: u64,
    client: Client,
) -> Option<Solving> {
    let mut child = Command::new(solver)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .ok()?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        return None;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let output = std::thread::Builder::new()
        .name("sail-z3-cached-out".to_owned())
        .spawn(move || output_side(stdout, skip, &rx, client))
        .ok();
    let mut s = Solving {
        child,
        stdin: Some(stdin),
        expects: tx,
        output,
    };
    s.write(raw);
    Some(s)
}

impl Solving {
    fn write(&mut self, bytes: &[u8]) {
        if let Some(stdin) = &mut self.stdin
            && stdin.write_all(bytes).and_then(|()| stdin.flush()).is_err()
        {
            self.stdin = None;
        }
    }

    fn finish(mut self) -> ExitCode {
        drop(self.stdin.take());
        drop(self.expects);
        let status = self.child.wait();
        if let Some(h) = self.output.take() {
            let _ = h.join();
        }
        match status {
            Ok(s) => crate::exit_code(s),
            Err(_) => ExitCode::from(1),
        }
    }
}

/// Runs a cached session. Returns `None`, having consumed no input, if the
/// daemon cannot be reached or cannot identify the solver; the caller then
/// runs plain pass-through.
pub(crate) fn run(socket: &Path, solver: &Path, args: &[std::ffi::OsString]) -> Option<ExitCode> {
    let mut client = Client::connect(socket).ok()?;
    let identity = parse_identity(&client.hello(&solver.to_string_lossy()).ok()?)?;
    let mut prefix = PrefixHasher::new(&identity, &Mode::Default);
    let mut session = Session::new();
    let mut splitter = CommandSplitter::new();
    let mut raw: Vec<u8> = Vec::new();
    let mut served = 0u64;
    let mut solving: Option<Solving> = None;
    let mut client = Some(client);
    let mut stdin = std::io::stdin().lock();
    let mut buf = vec![0u8; 64 * 1024];

    // Starts the solver with everything received so far.
    let start = |raw: &[u8], served: u64, client: &mut Option<Client>| -> Option<Solving> {
        let c = client.take().or_else(|| Client::connect(socket).ok())?;
        start_solver(solver, args, raw, served, c)
    };

    loop {
        let n = match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let chunk = buf.get(..n).unwrap_or_default().to_vec();
        raw.extend_from_slice(&chunk);
        let mut items = Vec::new();
        splitter.push(&chunk, &mut items);
        // Expectations for commands in this chunk are queued before the
        // chunk reaches the solver.
        let mut fresh_solver = false;
        for item in items {
            let end = usize::try_from(item.offset + item.bytes.len() as u64).unwrap_or(usize::MAX);
            let from = usize::try_from(prefix.len()).unwrap_or(usize::MAX);
            prefix.update(raw.get(from..end).unwrap_or_default());
            let expects = if item.kind == ItemKind::List {
                session.expects_output(&item.bytes)
            } else {
                Output::Unknown
            };
            session.apply(&item.bytes);
            let name = classify(&item.bytes)
                .map(|c| String::from_utf8_lossy(c.name).into_owned())
                .unwrap_or_default();
            if let Some(s) = &solving {
                let e = match expects {
                    Output::None => continue,
                    Output::One => Expect::Store {
                        key: prefix.key(),
                        command: name,
                        text: item.bytes.clone(),
                    },
                    Output::Unknown => Expect::Stop,
                };
                let _ = s.expects.send(e);
                continue;
            }
            let hit = match (expects, client.as_mut()) {
                (Output::None, _) => continue,
                (Output::One, Some(c)) => c
                    .get(&prefix.key().0)
                    .ok()
                    .flatten()
                    .filter(|r| !unservable(&item.bytes, r)),
                _ => None,
            };
            if let Some(reply) = hit {
                if !write_out(&reply) {
                    return Some(ExitCode::SUCCESS);
                }
                served += 1;
                continue;
            }
            // Miss: start the solver with the whole input so far.
            let Some(s) = start(&raw, served, &mut client) else {
                return Some(ExitCode::from(1));
            };
            let e = match expects {
                Output::One => Expect::Store {
                    key: prefix.key(),
                    command: name,
                    text: item.bytes.clone(),
                },
                _ => Expect::Stop,
            };
            let _ = s.expects.send(e);
            solving = Some(s);
            fresh_solver = true;
        }
        if let Some(s) = &mut solving
            && !fresh_solver
        {
            s.write(&chunk);
        }
    }
    if let Some(s) = solving {
        return Some(s.finish());
    }
    // End of input while serving. Incomplete trailing input goes to the
    // solver, which decides what it means.
    if splitter.finish().is_some() {
        return start(&raw, served, &mut client).map(Solving::finish);
    }
    Some(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::{Expect, Pairer};
    use proptest::prelude::*;
    use sail_key::QueryKey;

    fn key(i: u8) -> QueryKey {
        QueryKey([i; 32])
    }

    fn store(i: u8, text: &str) -> Expect {
        let command = text
            .trim_start_matches('(')
            .split([' ', ')'])
            .next()
            .unwrap_or_default()
            .to_owned();
        Expect::Store {
            key: key(i),
            command,
            text: text.as_bytes().to_vec(),
        }
    }

    fn model(i: u8) -> Expect {
        store(i, "(get-model)")
    }

    proptest! {
        /// Whatever order replies and expectations arrive in, and however
        /// often pairs are drained, reply `i` is stored under key `i`.
        #[test]
        fn pairing_is_order_independent(order in prop::collection::vec(any::<bool>(), 0..80), drains in prop::collection::vec(any::<bool>(), 0..80)) {
            let n = order.iter().filter(|b| **b).count().min(order.len() - order.iter().filter(|b| **b).count());
            let mut p = Pairer::default();
            let (mut r, mut e) = (0u8, 0u8);
            let mut stored = Vec::new();
            for (i, is_reply) in order.iter().enumerate() {
                if *is_reply && usize::from(r) < n {
                    p.reply(vec![b'm', r]);
                    r += 1;
                } else if usize::from(e) < n {
                    p.expect(model(e));
                    e += 1;
                }
                if drains.get(i).copied().unwrap_or(false) {
                    stored.extend(p.drain());
                }
            }
            while usize::from(r) < n { p.reply(vec![b'm', r]); r += 1; }
            while usize::from(e) < n { p.expect(model(e)); e += 1; }
            stored.extend(p.drain());
            prop_assert_eq!(stored.len(), n);
            for (k, _, reply) in &stored {
                prop_assert_eq!(k.0[0], reply[1]);
            }
        }
    }

    #[test]
    fn errors_stop_and_unknown_checks_are_not_stored() {
        let mut p = Pairer::default();
        p.expect(store(1, "(check-sat)"));
        p.reply(b"unknown\n".to_vec());
        p.expect(store(2, "(get-model)"));
        p.reply(b"(model)\n".to_vec());
        p.expect(store(3, "(echo \"<<DONE>>\")"));
        p.reply(b"(error \"x\")\n".to_vec());
        p.expect(store(4, "(echo \"<<DONE>>\")"));
        p.reply(b"<<DONE>>\n".to_vec());
        let stored = p.drain();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].0, key(2));
    }

    #[test]
    fn implausible_replies_stop_recording() {
        // The bug this guards against: a statistics reply paired with an echo.
        let mut p = Pairer::default();
        p.expect(store(1, "(echo \"<<DONE>>\")"));
        p.reply(b"(:rlimit-count 5)\n".to_vec());
        p.expect(store(2, "(get-model)"));
        p.reply(b"(model)\n".to_vec());
        assert_eq!(p.drain().len(), 0);
    }

    #[test]
    fn plausibility_by_command() {
        use super::plausible;
        assert!(plausible(b"(echo \"<<DONE>>\")", b"<<DONE>>\n"));
        assert!(plausible(b"(echo \"a\"\"b\")", b"a\"b\n"));
        assert!(!plausible(b"(echo \"<<DONE>>\")", b"(:x 1)\n"));
        assert!(plausible(b"(check-sat)", b"unsat\n"));
        assert!(!plausible(b"(check-sat)", b"<<DONE>>\n"));
        assert!(plausible(
            b"(get-info :version)",
            b"(:version \"4.16.0\")\n"
        ));
        assert!(!plausible(b"(get-info :version)", b"unsat\n"));
        assert!(plausible(
            b"(get-info :all-statistics)",
            b"(:rlimit-count 5)\n"
        ));
        assert!(!plausible(b"(get-model)", b"(error \"no model\")\n"));
    }
}
