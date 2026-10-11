#!/usr/bin/env python3
"""Prepare/run an isolated, intent-derived stock AIR transport diagnostic.

This is not a proof or acceptance collector. Preparation never launches a
compiler/solver. Build and run are explicit separate operations, with owned
process-group cleanup. The source transformation only adds metadata markers.
"""

import argparse
from collections import Counter
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import time

AIR_SHA = "d11353d624acb6bb9f0e8c104cbcb705f89a4ac0ce479e283cdbc26848e46201"
INPUT_SHA = "f59d7790f0f4f986691940dd5eb6d6a17b2b98041f399927364f005fd06fcc93"
OPEN = b";;;>>> QUERY\n"
CLOSE = b";;;<<<\n"
STAGES = {
    1: "enqueue-before", 2: "enqueue-after", 3: "writer-received",
    4: "write-before", 5: "write-after", 6: "newline-before",
    7: "newline-after", 8: "echo-before", 9: "echo-after",
    10: "flush-before", 11: "flush-after", 12: "reader-request",
    13: "read-before", 14: "read-after", 15: "done-observed",
    16: "delivery-before", 17: "delivery-after", 18: "receive-before",
    19: "receive-after", 20: "drop-before", 21: "drop-after",
}
MANIFEST_SHA = "4b6f48d278e981f9f3ce40b5e8238ffed2409cac61f003c09fd8ee0c2c9a7c41"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_bounded(path, limit):
    with Path(path).open("rb") as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError(f"bounded input exceeded: {Path(path).name}")
    return data


def identity(path, limit=128 * 1024 * 1024):
    path = Path(path)
    data = read_bounded(path, limit)
    return {"path": str(path.resolve()), "bytes": len(data), "sha256": digest(data)}


def checked(path, expected, limit):
    data = read_bounded(path, limit)
    if digest(data) != expected:
        raise ValueError(f"identity mismatch: {Path(path).name}")
    return data


def extract_commands(transcript):
    """Require the one complete QUERY block; never normalize its command bytes."""
    if not transcript.startswith(OPEN) or not transcript.endswith(CLOSE):
        raise ValueError("expected exactly one complete QUERY block")
    if transcript.count(OPEN) != 1 or transcript.count(CLOSE) != 1:
        raise ValueError("ambiguous logger markers")
    if b";;;>>> RESPONSE" in transcript:
        raise ValueError("response-bearing input is outside this diagnostic")
    return transcript[len(OPEN):-len(CLOSE)]


def replace_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError("stock extraction boundary changed")
    return source.replace(old, new, 1)


def instrument(source):
    """Preserve stock operations/bytes, adding explicit integer-only markers."""
    source = replace_once(source, "use yansi::Paint;", "use yansi::Paint;\nuse crate::mark;")
    source = replace_once(source, "while let Ok(req) = requests.recv() {", "while let Ok(req) = requests.recv() {\n        mark(3); mark(4);")
    source = replace_once(source, ".and_then(|_| writeln!(&smt_pipe_stdin))", ".and_then(|_| { mark(5); mark(6); let r = writeln!(&smt_pipe_stdin); if r.is_ok() { mark(7); } r })")
    source = replace_once(source, '.and_then(|_| writeln!(&smt_pipe_stdin, "(echo \\\"{}\\\")", DONE))', '.and_then(|_| { mark(8); let r = writeln!(&smt_pipe_stdin, "(echo \\\"{}\\\")", DONE); if r.is_ok() { mark(9); } r })')
    source = replace_once(source, ".and_then(|_| smt_pipe_stdin.flush())", ".and_then(|_| { mark(10); let r = smt_pipe_stdin.flush(); if r.is_ok() { mark(11); } r })")
    source = replace_once(source, "while let Ok(mut smt_pipe_stdout) = recv_requests.recv() {", "while let Ok(mut smt_pipe_stdout) = recv_requests.recv() {\n        mark(12);")
    source = replace_once(source, "let mut line = String::new();", "let mut line = String::new();\n            mark(13);")
    source = replace_once(source, 'line = line.replace("\\n", "").replace("\\r", "");', 'mark(14);\n            line = line.replace("\\n", "").replace("\\r", "");')
    source = replace_once(source, "                    responses\n", "                    mark(15); mark(16);\n                    responses\n")
    source = replace_once(source, '                        .expect("internal error: solver reader thread failure");\n                    break;', '                        .expect("internal error: solver reader thread failure");\n                    mark(17);\n                    break;')
    source = replace_once(source, "        self.requests\n", "        mark(1);\n        self.requests\n")
    source = replace_once(source, '.expect("internal error: failed to send to writer thread");', '.expect("internal error: failed to send to writer thread");\n        mark(2);')
    source = replace_once(source, "    pub fn wait(mut self) -> Vec<String> {", "    pub fn wait(mut self) -> Vec<String> {\n        mark(18);")
    source = replace_once(source, "        self.log_result(&result);\n        self.smt_process.responses_buf_recv", "        mark(19);\n        self.log_result(&result);\n        self.smt_process.responses_buf_recv")
    return source


MAIN = r'''#![forbid(unsafe_code)]
use std::io::Write;
use std::os::unix::net::UnixDatagram;
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::time::{Duration, Instant};
mod context { #[derive(Debug, Clone, Copy)] pub enum SmtSolver { Z3, Cvc5 } }
mod stock { include!("instrumented-smt_process.rs"); }
static QUEUE: OnceLock<SyncSender<[u8; 32]>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
static QUEUE_DROPPED: AtomicU64 = AtomicU64::new(0);
static QUEUE_FULL: AtomicU64 = AtomicU64::new(0);
static QUEUE_DISCONNECTED: AtomicU64 = AtomicU64::new(0);
static SEND_DROPPED: AtomicU64 = AtomicU64::new(0);
static SEND_ERRORS: AtomicU64 = AtomicU64::new(0);
static DEADLINE_DROPPED: AtomicU64 = AtomicU64::new(0);
static PAUSE: OnceLock<u64> = OnceLock::new();
fn dropped() -> u64 {
    QUEUE_DROPPED.load(Ordering::Relaxed) + SEND_DROPPED.load(Ordering::Relaxed)
}
fn mark(stage: u64) {
    let seq = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let elapsed = START.get().map_or(0, |s| s.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
    let mut row = [0u8; 32];
    for (slot, value) in row.chunks_exact_mut(8).zip([seq, elapsed, dropped(), stage]) {
        slot.copy_from_slice(&value.to_le_bytes());
    }
    match QUEUE.get().map(|sender| sender.try_send(row)) {
        Some(Ok(())) => {}
        Some(Err(TrySendError::Full(_))) => {
            QUEUE_DROPPED.fetch_add(1, Ordering::Relaxed);
            QUEUE_FULL.fetch_add(1, Ordering::Relaxed);
        }
        _ => {
            QUEUE_DROPPED.fetch_add(1, Ordering::Relaxed);
            QUEUE_DISCONNECTED.fetch_add(1, Ordering::Relaxed);
        }
    }
    // Only sensitivity controls select a pause. Baseline selects zero.
    if PAUSE.get() == Some(&stage) { loop { std::thread::park(); } }
}
struct MetadataOwner {
    stop: Arc<AtomicBool>,
    writer: Option<std::thread::JoinHandle<()>>,
}
impl MetadataOwner {
    fn finish(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stop.store(true, Ordering::Release);
        if let Some(writer) = self.writer.take() {
            writer.join().map_err(|_| "metadata writer panicked")?;
        }
        Ok(())
    }
}
impl Drop for MetadataOwner {
    fn drop(&mut self) { let _ = self.finish(); }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 { return Err("command, metadata socket, response file, pause stage, whole deadline required".into()); }
    let sock = UnixDatagram::unbound()?;
    sock.connect(&args[2])?;
    let began = Instant::now();
    START.set(began).map_err(|_| "clock already initialized")?;
    PAUSE.set(args[4].parse()?).map_err(|_| "pause already initialized")?;
    let budget = Duration::from_secs(args[5].parse()?);
    if budget.is_zero() || budget > Duration::from_secs(600) { return Err("invalid whole deadline".into()); }
    let (sender, receiver) = sync_channel::<[u8; 32]>(64);
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let writer = std::thread::Builder::new().name("sail-diag-metadata".into()).spawn(move || {
        loop {
            let row = if worker_stop.load(Ordering::Acquire) {
                match receiver.try_recv() { Ok(row) => row, Err(_) => break }
            } else {
                match receiver.recv_timeout(Duration::from_millis(10)) {
                    Ok(row) => row,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            };
            let remaining = budget.saturating_sub(began.elapsed());
            if remaining.is_zero() {
                SEND_DROPPED.fetch_add(1, Ordering::Relaxed);
                DEADLINE_DROPPED.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let timeout = remaining.min(Duration::from_millis(25));
            let sent = sock.set_write_timeout(Some(timeout)).and_then(|()| sock.send(&row));
            if !sent.is_ok_and(|n| n == 32) {
                SEND_DROPPED.fetch_add(1, Ordering::Relaxed);
                SEND_ERRORS.fetch_add(1, Ordering::Relaxed);
            }
        }
    })?;
    let mut metadata = MetadataOwner { stop, writer: Some(writer) };
    QUEUE.set(sender).map_err(|_| "metadata queue already initialized")?;
    let commands = std::fs::read(&args[1])?;
    if commands.len() > 1024 * 1024 { return Err("command bound".into()); }
    let mut process = stock::SmtProcess::launch(&context::SmtSolver::Z3, None);
    let responses = process.send_commands(commands);
    let bytes = responses.iter().try_fold(0usize, |n, line| n.checked_add(line.len() + 1)).ok_or("response overflow")?;
    if bytes > 1024 * 1024 { return Err("response bound".into()); }
    let mut output = std::fs::File::create(&args[3])?;
    for line in &responses { writeln!(output, "{line}")?; }
    output.flush()?;
    mark(20);
    drop(process);
    mark(21);
    metadata.finish()?;
    println!("{{\"responses\":{},\"observations\":{},\"dropped\":{},\"queue_dropped\":{},\"queue_full\":{},\"queue_disconnected\":{},\"send_dropped\":{},\"sender_errors\":{},\"deadline_dropped\":{}}}", responses.len(), SEQUENCE.load(Ordering::Relaxed), dropped(), QUEUE_DROPPED.load(Ordering::Relaxed), QUEUE_FULL.load(Ordering::Relaxed), QUEUE_DISCONNECTED.load(Ordering::Relaxed), SEND_DROPPED.load(Ordering::Relaxed), SEND_ERRORS.load(Ordering::Relaxed), DEADLINE_DROPPED.load(Ordering::Relaxed));
    Ok(())
}
'''

FAKE_SOLVER = '''#!/usr/bin/env python3
import os
import sys
total = 0
with open(os.environ["SAIL_DIAG_WIRE_PATH"], "xb") as wire:
 for line in sys.stdin.buffer:
  total += len(line)
  if total > 1048576:
   raise ValueError("control input bound")
  wire.write(line)
  wire.flush()
  if line == b'(echo "<<DONE>>")\\n':
   print('(:control 1)')
   print('<<DONE>>', flush=True)
'''


def fixed_stages(pause):
    """Exact one-request/two-line fake-peer prefixes of the bound stock source."""
    return {
        1: [1],
        4: [1, 2, 3, 4, 12, 13, 18],
        13: list(range(1, 14)) + [18],
        16: list(range(1, 17)) + [13, 14, 18],
        0: list(range(1, 22)) + [13, 14],
    }[pause]


def sensitivity(rows, pause, timed_out):
    """Certify only the exact, loss-free fixed-control prefix, not general tails."""
    expected = Counter(fixed_stages(pause))
    observed = Counter(row["stage"] for row in rows)
    sequences = [row.get("sequence") for row in rows]
    valid_sequence = (all(type(n) is int for n in sequences)
                      and sorted(sequences) == list(range(len(sequences))))
    passed = (bool(timed_out) == bool(pause) and valid_sequence
              and not any(row["dropped"] for row in rows) and observed == expected)
    result = {"passed": passed, "expected_rows": sum(expected.values()),
              "expected_stage_counts": dict(sorted(expected.items())),
              "observed_stage_counts": dict(sorted(observed.items())),
              "fixed_control_prefix_only": True}
    if pause:
        following = {1: 2, 4: 5, 13: 14, 16: 17}
        result.update(confirmed_boundary=STAGES[pause],
                      following_boundary_absent=STAGES[following[pause]])
    return result


def completed_counters(value):
    """Require complete normal-exit sender accounting, including every loss class."""
    fields = {"responses", "observations", "dropped", "queue_dropped", "queue_full",
              "queue_disconnected", "send_dropped", "sender_errors", "deadline_dropped"}
    if (not isinstance(value, dict) or set(value) != fields
            or any(type(n) is not int or n < 0 for n in value.values())
            or value["queue_dropped"] != value["queue_full"] + value["queue_disconnected"]
            or value["send_dropped"] != value["sender_errors"] + value["deadline_dropped"]
            or value["dropped"] != value["queue_dropped"] + value["send_dropped"]
            or value["dropped"] > value["observations"]):
        raise ValueError("completed counter shape/accounting")
    return value


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def prepare(args):
    source = checked(args.air, AIR_SHA, 16 * 1024).decode()
    transcript = checked(args.transcript, INPUT_SHA, 1024 * 1024)
    commands = extract_commands(transcript)
    config_bytes = read_bounded(args.config, 64 * 1024)
    config = json.loads(config_bytes)
    # Require identities for all supplied executables/dependency and the full
    # selected runtime inventory; no historical environment is printed.
    if set(config) != {"files", "rustc", "yansi", "solver", "environment", "runtime_manifest", "runtime_root", "runtime_manifest_sha256"}:
        raise ValueError("execution config shape")
    if config["runtime_manifest_sha256"] != MANIFEST_SHA:
        raise ValueError("selected runtime manifest differs")
    manifest = json.loads(checked(config["runtime_manifest"], MANIFEST_SHA, 65536))
    if len(manifest["artifacts"]) != 13:
        raise ValueError("selected complete artifact inventory differs")
    if not 16 <= len(config["files"]) <= 32:
        raise ValueError("complete runtime plus executable inventory required")
    paths = set()
    for entry in config["files"]:
        if set(entry) != {"path", "bytes", "sha256"} or entry["path"] in paths:
            raise ValueError("identity inventory shape/duplicate")
        paths.add(entry["path"])
        actual = identity(entry["path"])
        if actual != entry:
            raise ValueError("execution inventory identity changed")
    if any(config[name] not in paths for name in ("rustc", "yansi", "solver")):
        raise ValueError("unsealed tool/dependency")
    inventory = {entry["path"]: entry["sha256"] for entry in config["files"]}
    for relative, sha in manifest["artifacts"].items():
        target = Path(relative)
        if target.is_absolute() or ".." in target.parts:
            raise ValueError("unsafe runtime artifact path")
        if inventory.get(str((Path(config["runtime_root"]) / target).resolve())) != sha:
            raise ValueError("incomplete selected runtime inventory")
    if set(config["environment"]) - {"PATH", "VERUS_Z3_PATH", "SAIL_Z3_PATH", "SAIL_DAEMON_SOCKET", "SAIL_DAEMON_TIMEOUT_MS"}:
        raise ValueError("only explicit solver/cache choices are allowed")
    if config["environment"].get("VERUS_Z3_PATH") != config["solver"]:
        raise ValueError("solver environment mismatch")
    real_solver = config["environment"].get("SAIL_Z3_PATH")
    if real_solver not in paths or inventory[real_solver] != manifest["z3_sha256"]:
        raise ValueError("SAIL_Z3_PATH is not the sealed selected pinned solver")
    if "SAIL_DAEMON_SOCKET" in config["environment"]:
        raise ValueError("this component arm is forwarding-only; daemon context is not reproduced")
    out = args.out.resolve()
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    for name, data in (("commands.bin", commands), ("expected-wire.bin", commands + b'\n(echo "<<DONE>>")\n'),
                       ("pinned-smt_process.rs", source.encode()), ("instrumented-smt_process.rs", instrument(source).encode()),
                       ("main.rs", MAIN.encode()), ("execution.json", config_bytes)):
        (out / name).write_bytes(data)
    write_json(out / "prepared.json", {"kind": "intent-derived-stock-transport-component", "historical_wire_capture": False,
               "historical_concurrency_reproduced": False, "proof_or_acceptance": False,
               "source_extraction": {"opening": OPEN.decode(), "closing": CLOSE.decode(), "normalization": False},
               "input": identity(args.transcript, 1024 * 1024), "source": identity(args.air, 16384),
               "files": [identity(out / n) for n in ("commands.bin", "expected-wire.bin", "pinned-smt_process.rs", "instrumented-smt_process.rs", "main.rs", "execution.json")],
               "controller": identity(__file__, 64 * 1024), "operation_deadline_s": 600,
               "ring_rows": 256, "metadata_datagram_bytes": 32, "observer_samples": 3,
               "observer": {"pids": 128, "tasks_per_pid": 128, "fds_per_pid": 64, "bytes": 1048576, "seconds": 0.5},
               "output_watch_bytes": 4194304, "stages": STAGES})


def validate_prepared(out):
    record = json.loads(read_bounded(out / "prepared.json", 64 * 1024))
    for entry in record["files"]:
        if identity(entry["path"]) != entry:
            raise ValueError("prepared file changed")
    config = json.loads(read_bounded(out / "execution.json", 64 * 1024))
    checked(config["runtime_manifest"], MANIFEST_SHA, 65536)
    for entry in config["files"]:
        if identity(entry["path"]) != entry:
            raise ValueError("sealed executable/runtime changed")
    return config


def cleanup(actor):
    """Signal only the process group created by this retained Popen handle."""
    errors = []
    if actor.poll() is None:
        try:
            os.killpg(actor.pid, signal.SIGINT)
        except ProcessLookupError:
            pass
        except OSError as error:
            errors.append(type(error).__name__)
        try:
            actor.wait(timeout=5)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(actor.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            except OSError as error:
                errors.append(type(error).__name__)
            try:
                actor.wait(timeout=5)
            except subprocess.TimeoutExpired:
                errors.append("owned actor did not reap after SIGKILL")
    return errors


def build(args):
    out = args.out.resolve()
    config = validate_prepared(out)
    command = [config["rustc"], "--edition=2021", "-C", "opt-level=0", "-C", "debug-assertions=yes",
               "--extern", "yansi=" + config["yansi"], str(out / "main.rs"), "-o", str(out / "transport")]
    started = time.monotonic()
    code = None
    timed_out = False
    with (out / "build.stdout").open("xb") as stdout, (out / "build.stderr").open("xb") as stderr:
        actor = subprocess.Popen(command, stdout=stdout, stderr=stderr, start_new_session=True)
        try:
            code = actor.wait(timeout=120)
        except subprocess.TimeoutExpired:
            timed_out = True
        finally:
            closure_errors = cleanup(actor)
    write_json(out / "build.json", {"argv": command, "exit_code": code, "elapsed_s": time.monotonic() - started,
                                   "timed_out": timed_out,
                                   "cleanup_errors": closure_errors,
                                   "binary": identity(out / "transport") if code == 0 else None})
    if timed_out or code != 0 or closure_errors:
        raise RuntimeError("diagnostic compilation failed")


def run(args):
    out = args.out.resolve()
    config = validate_prepared(out)
    built = json.loads(read_bounded(out / "build.json", 64 * 1024))
    if identity(out / "transport") != built["binary"]:
        raise ValueError("diagnostic binary changed")
    spec = importlib.util.spec_from_file_location("bounded_owned_observer", args.observer)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    run_out = out / args.label
    run_out.mkdir(mode=0o700, exist_ok=False)
    metadata = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    socket_path = run_out / "metadata.sock"
    if len(os.fsencode(socket_path)) > 100:
        raise ValueError("metadata socket path must be at most 100 bytes")
    metadata.bind(str(socket_path))
    metadata.setblocking(False)
    command_file = out / "commands.bin"
    solver = config["solver"]
    # Sensitivity controls deliberately use a separately owned fake peer and
    # tiny command; they never replace the original component input silently.
    if args.pause or args.fake_peer:
        command_file = run_out / "control.bin"
        command_file.write_bytes(b"(get-info :all-statistics)\n")
        solver = str(run_out / "fake-solver")
        Path(solver).write_text(FAKE_SOLVER)
        Path(solver).chmod(0o700)
    env = dict(config["environment"], VERUS_Z3_PATH=solver)
    if args.pause or args.fake_peer:
        env["SAIL_DIAG_WIRE_PATH"] = str(run_out / "fake-peer-wire.bin")
    rows, total, overwritten = [], 0, 0
    final_drain_limited = False

    def drain(limit):
        nonlocal total, overwritten
        for _ in range(limit):
            try:
                row = metadata.recv(33)
            except BlockingIOError:
                return
            if len(row) != 32:
                raise ValueError("invalid diagnostic datagram")
            values = struct.unpack("<QQQQ", row)
            rows.append(dict(zip(("sequence", "elapsed_ns", "dropped", "stage"), values)))
            total += 1
            if len(rows) > 256:
                del rows[0]
                overwritten += 1
    deadline = 3 if args.pause or args.fake_peer else 600
    started = time.monotonic()
    command = [str(out / "transport"), str(command_file), str(socket_path), str(run_out / "responses"), str(args.pause), str(deadline)]
    timed_out = False
    observations = []
    operation_error = None
    with (run_out / "stdout").open("xb") as stdout, (run_out / "stderr").open("xb") as stderr:
        actor = subprocess.Popen(command, env=env, stdout=stdout, stderr=stderr, start_new_session=True)
        root_capture = module.Capture()
        root = module.process_stat(root_capture.read(Path('/proc') / str(actor.pid) / 'stat'))
        birth = root["start_ticks"] if root else None
        if birth is None:
            cleanup(actor)
            metadata.close()
            socket_path.unlink(missing_ok=True)
            raise RuntimeError("owned diagnostic root birth unavailable")
        sample_at = iter((started + 1, started + deadline / 2, started + deadline - 0.6))
        next_sample = next(sample_at, float("inf"))
        try:
            while True:
                drain(256)
                now = time.monotonic()
                if actor.poll() is not None:
                    break
                if now >= started + deadline:
                    timed_out = True
                    break
                if now >= next_sample:
                    capture = module.Capture()
                    observations.append({"elapsed_s": now - started, "processes": module.owned_processes(actor.pid, capture, expected_start_ticks=birth),
                                         "limited": capture.limited, "errors": capture.errors})
                    next_sample = next(sample_at, float("inf"))
                if max((run_out / "stdout").stat().st_size, (run_out / "stderr").stat().st_size) > 4194304:
                    raise ValueError("diagnostic stream bound")
                time.sleep(min(0.01, max(0, started + deadline - time.monotonic())))
        except BaseException as error:
            operation_error = type(error).__name__
            raise
        finally:
            closure_errors = cleanup(actor)
            # Writers have either completed or the owned process group was
            # terminated. Bound this last metadata-only drain independently.
            try:
                drain(512)
                try:
                    metadata.recv(1, socket.MSG_PEEK)
                    final_drain_limited = True
                except BlockingIOError:
                    pass
            except (OSError, ValueError) as error:
                closure_errors.append("final metadata drain: " + type(error).__name__)
            complete_counts = None
            if actor.returncode == 0:
                try:
                    complete_counts = completed_counters(json.loads(read_bounded(run_out / "stdout", 4096)))
                except (OSError, ValueError):
                    complete_counts = None
                    closure_errors.append("completed metadata counters unavailable")
            loss = {"final_drain_limited": final_drain_limited,
                    "sender_dropped_at_last_row": max((r["dropped"] for r in rows), default=0),
                    "completed_counts": complete_counts,
                    "tail_loss_unknown_without_completion_counts": complete_counts is None,
                    "fixed_control_prefix_certified": (args.pause or args.fake_peer)
                        and sensitivity(rows, args.pause, timed_out)["passed"],
                    "metadata_writer_joined_on_normal_exit": actor.returncode == 0 and complete_counts is not None}
            if complete_counts is not None:
                loss["missing_received_rows"] = complete_counts["observations"] - complete_counts["dropped"] - total
            wire_comparison = None
            if args.pause or args.fake_peer:
                expected = read_bounded(command_file, 1048576) + b'\n(echo "<<DONE>>")\n'
                wire_path = run_out / "fake-peer-wire.bin"
                try:
                    actual = read_bounded(wire_path, 1048576) if wire_path.is_file() else None
                except (OSError, ValueError) as error:
                    closure_errors.append("wire capture: " + type(error).__name__)
                    actual = None
                required = b"" if args.pause in (1, 4) else expected
                wire_comparison = {"expected_bytes": len(expected), "expected_sha256": digest(expected),
                                   "actual_bytes": len(actual) if actual is not None else None,
                                   "actual_sha256": digest(actual) if actual is not None else None,
                                   "required_prefix_bytes": len(required), "passed": actual == required}
            metadata.close()
            socket_path.unlink(missing_ok=True)
            postseals = []
            for entry in config["files"]:
                try:
                    postseals.append(identity(entry["path"]))
                except (OSError, ValueError) as error:
                    postseals.append({"path": entry["path"], "error": type(error).__name__})
                    closure_errors.append("postseal: " + type(error).__name__)
            postseals_equal = postseals == config["files"]
            if not postseals_equal:
                closure_errors.append("sealed runtime/tool bytes changed")
            write_json(run_out / "terminal.json", {"kind": "sensitivity" if args.pause or args.fake_peer else "intent-derived-component",
                       "proof_or_acceptance": False, "pause_stage": args.pause, "deadline_s": deadline,
                       "timed_out": timed_out, "elapsed_inclusive_s": time.monotonic() - started, "exit_code": actor.returncode,
                       "cleanup_errors": closure_errors,
                       "operation_error": operation_error,
                       "metadata_rows": rows, "received_rows": total, "overwritten_rows": overwritten,
                       "observation_loss": loss, "fake_peer_wire_comparison": wire_comparison,
                       "owned_observations": observations, "observer": identity(args.observer, 65536),
                       "sensitivity": sensitivity(rows, args.pause, timed_out) if args.pause or args.fake_peer else None,
                       "command": identity(command_file, 1048576), "postsealed_files": postseals,
                       "postseals_equal": postseals_equal})
    if closure_errors:
        raise RuntimeError("owned cleanup failed; terminal evidence retained")
    if args.pause or args.fake_peer:
        if (not sensitivity(rows, args.pause, timed_out)["passed"] or not wire_comparison["passed"]
                or overwritten or final_drain_limited or (not args.pause and (actor.returncode != 0
                    or complete_counts is None or complete_counts["responses"] != 1
                    or complete_counts["dropped"] != 0 or loss["missing_received_rows"] != 0))):
            raise RuntimeError("sensitivity control failed; terminal evidence retained")
    elif timed_out or actor.returncode != 0:
        raise RuntimeError("component failed; terminal evidence retained")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    prep = sub.add_parser("prepare")
    for name in ("air", "transcript", "config", "out"):
        prep.add_argument("--" + name, type=Path, required=True)
    sub.add_parser("build").add_argument("--out", type=Path, required=True)
    execute = sub.add_parser("run")
    execute.add_argument("--out", type=Path, required=True)
    execute.add_argument("--observer", type=Path, required=True)
    execute.add_argument("--label", required=True)
    execute.add_argument("--pause", type=int, choices=(0, 1, 4, 13, 16), default=0)
    execute.add_argument("--fake-peer", action="store_true", help="normal complete-delivery sensitivity control only")
    args = parser.parse_args()
    if args.mode == "run" and (not args.label or Path(args.label).name != args.label):
        parser.error("label must be a single path component")
    {"prepare": prepare, "build": build, "run": run}[args.mode](args)


if __name__ == "__main__":
    main()
