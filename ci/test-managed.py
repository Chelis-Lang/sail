#!/usr/bin/env python3
"""Bounded real-tool acceptance for managed proof receipts; never a benchmark."""

import json
import os
import re
from pathlib import Path
import shutil
import select
import signal
import subprocess
import sys
import tempfile
import threading
import time

sail = str(Path(sys.argv[1]).resolve())
# Each CI part owns a fresh fixture. `all` keeps the complete local union on
# one project; core retains receipts/jobs/failure/prune, copies and native
# isolate the additional cold-project controls, and scripts covers generation.
part = sys.argv[2] if len(sys.argv) > 2 else "all"
if part not in ("all", "core", "copies", "native", "scripts"):
    raise SystemExit(f"managed acceptance: unknown part {part!r}; expected all, core, copies, native or scripts")
cargo_verus = os.environ.get("SAIL_CARGO_VERUS") or shutil.which("cargo-verus")
if not cargo_verus:
    raise SystemExit("managed acceptance requires the pinned cargo-verus")

verification_sequence = 0
verification_lock = threading.Lock()


def begin_verification(timeout):
    global verification_sequence
    with verification_lock:
        verification_sequence += 1
        label = f"verification-{verification_sequence:02d}"
        operation = {"operation": label, "state": "started",
                     "verification_timeout_s": timeout, "returncode": None}
        path = evidence / (label + ".operation.json")
        path.write_text(json.dumps(operation))
        print(f"managed acceptance: {part} {label} starting (limit {timeout}s)", flush=True)
    return path, operation, time.monotonic()


def finish_verification(observation, returncode, stdout, stderr, timed_out=False, cancelled=False):
    path, operation, started = observation
    # Preserve the background oracle's streams even if a concurrent managed
    # verification fails before the caller consumes the oracle's result.
    for name, data in (("stdout", stdout), ("stderr", stderr)):
        stream_path = path.with_name(operation["operation"] + "." + name)
        stream_path.write_bytes(data)
        operation[name] = {"file": stream_path.name, "bytes": len(data)}
    operation.update(state="finished", returncode=returncode, timed_out=timed_out,
                     cancelled=cancelled,
                     duration_s=time.monotonic() - started)
    path.write_text(json.dumps(operation))
    print(f"managed acceptance: {part} {operation['operation']} finished "
          f"(exit {returncode}, {operation['duration_s']:.2f}s)", flush=True)


def bounded_verification(arguments, cwd, env, timeout):
    observation = begin_verification(timeout)
    process = subprocess.Popen(arguments, cwd=cwd, env=env,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired as failure:
        # Stop the complete owned verification tree before its scratch directory
        # is removed. Killing only Sail leaves Cargo writing into that directory.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        failure.stdout, failure.stderr = process.communicate(timeout=10)
        finish_verification(observation, process.returncode,
                            failure.stdout, failure.stderr, timed_out=True)
        raise
    finish_verification(observation, process.returncode, stdout, stderr)
    return subprocess.CompletedProcess(arguments, process.returncode, stdout, stderr)

# Under /tmp: macOS's default TMPDIR makes the job service socket path longer
# than a Unix socket name allows (SAIL0800).
with tempfile.TemporaryDirectory(prefix="sail-managed-acceptance-", dir="/tmp") as temporary:
    root = Path(temporary)
    evidence = Path(os.environ.get("SAIL_MANAGED_ACCEPTANCE_OUT", str(root / "evidence"))).resolve()
    evidence.mkdir(mode=0o700, parents=True, exist_ok=True)
    managed_calls = []
    env = dict(os.environ, SAIL_CACHE_DIR=str(root / "cache"))
    # Stock runs with the toolchain Sail selects for its Cargo (ADR 0058).
    stock_toolchain = subprocess.run(
        [sys.executable, str(Path(__file__).resolve().parent / "stock-toolchain.py"), sail],
        env=env, check=True, stdout=subprocess.PIPE).stdout.decode().strip()

    def stock_env():
        return dict(env, RUSTUP_TOOLCHAIN=stock_toolchain)
    # The project is a member of a two-member workspace and every run starts
    # in its own directory without -p, as a user inside a member crate does.
    # The other member has a build script that this member never schedules;
    # it must not withdraw reuse of vstd and the macro crates (#157). A
    # standalone package is covered by the legacy phase.
    workspace = root / "ws"
    workspace.mkdir()
    subprocess.run([cargo_verus, "verus", "new", "--lib", "demo"], cwd=workspace,
                   env=env, check=True, stdout=subprocess.DEVNULL)
    (workspace / "helper/src").mkdir(parents=True)
    (workspace / "helper/Cargo.toml").write_text(
        '[package]\nname = "helper"\nversion = "0.1.0"\nedition = "2021"\n')
    (workspace / "helper/build.rs").write_text(
        'fn main() { println!("cargo:rerun-if-changed=build.rs"); }\n')
    (workspace / "helper/src/lib.rs").write_text("")
    (workspace / "Cargo.toml").write_text(
        '[workspace]\nresolver = "2"\nmembers = ["demo", "helper"]\n')
    socket = root / "d.sock"
    env["SAIL_DAEMON_SOCKET"] = str(socket)
    daemon = subprocess.Popen([sail,"serve","--cache",str(root / "cache"),
                               "--socket",str(socket),"--idle-timeout","0"],
                              env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    def stop_daemon():
        if daemon.poll() is None:
            daemon.terminate()
            try:
                daemon.wait(timeout=10)
            except subprocess.TimeoutExpired:
                daemon.kill()
                daemon.wait(timeout=10)
    try:
        deadline=time.monotonic()+5
        while not socket.exists() and time.monotonic()<deadline and daemon.poll() is None:
            time.sleep(0.02)
        assert socket.exists() and daemon.poll() is None, "owned cache daemon did not start"
        project = workspace / "demo"
        source = project / "src/lib.rs"
        good = """use vstd::prelude::*;
    verus! {
    pub fn max(a: u32, b: u32) -> (r: u32)
        ensures r >= a, r >= b,
    { if a > b { a } else { b } }
    }
    """
        bad = good.replace("r >= b", "r >  b")
        assert len(bad) == len(good)
        # A project's first managed runs usually fail. Start with a failing
        # proof: vstd must still be kept for reuse (issue #123).
        source.write_text(bad)

        def recorded_cli(label, arguments):
            # Retain only this fixture's public CLI output and operation timing.
            # Capture before check_returncode so a service error survives scratch
            # cleanup; never persist the subprocess environment or runtime config.
            started = time.monotonic()
            try:
                run = subprocess.run(arguments, cwd=project, env=env,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     timeout=30)
            except subprocess.TimeoutExpired as failure:
                (evidence / (label + ".stdout.json")).write_bytes(failure.stdout or b"")
                (evidence / (label + ".stderr")).write_bytes(failure.stderr or b"")
                (evidence / (label + ".operation.json")).write_text(json.dumps({
                    "operation": label, "duration_s": time.monotonic() - started,
                    "returncode": None, "timed_out": True}))
                raise
            (evidence / (label + ".stdout.json")).write_bytes(run.stdout)
            (evidence / (label + ".stderr")).write_bytes(run.stderr)
            (evidence / (label + ".operation.json")).write_text(json.dumps({
                "operation": label, "duration_s": time.monotonic() - started,
                "returncode": run.returncode, "timed_out": False}))
            run.check_returncode()
            return run

        def diagnostic(document):
            return {key: document[key] for key in (
                "run_id", "context_sha256", "input_revision", "currentness",
                "exit_code", "execution", "notes", "error") } | {
                    "targets": [{key: target[key] for key in (
                        "package_id", "target", "kind", "establishing_run",
                        "requested", "complete", "reused") } | {
                            "summary": target["report"]["summary"]
                        } for target in document["targets"]]
                }

        def managed(*args, expected_requested=1):
            label = f"managed-{len(managed_calls) + 1:02d}"
            managed_calls.append(label)
            started = time.monotonic()
            events = evidence / (label + ".events.jsonl")
            # This existing observation-only variable is excluded from managed
            # input identity; invocation arguments and compiler flags are unchanged.
            observed_env = dict(env, SAIL_JOB_EVENTS=str(events))
            try:
                run = bounded_verification([sail, "verify", "--output", "json", *args],
                                           project, observed_env, 240)
            except subprocess.TimeoutExpired as failure:
                (evidence / (label + ".stdout.json")).write_bytes(failure.stdout or b"")
                (evidence / (label + ".stderr")).write_bytes(failure.stderr or b"")
                (evidence / (label + ".operation.json")).write_text(json.dumps({
                    "operation": label, "duration_s": time.monotonic() - started,
                    "verification_timeout_s": 240, "returncode": None, "timed_out": True}))
                raise
            (evidence / (label + ".stdout.json")).write_bytes(run.stdout)
            (evidence / (label + ".stderr")).write_bytes(run.stderr)
            (evidence / (label + ".operation.json")).write_text(json.dumps({
                "operation": label, "duration_s": time.monotonic() - started,
                "verification_timeout_s": 240, "returncode": run.returncode, "timed_out": False}))
            result = json.loads(run.stdout)
            assert result["kind"] == "sail-project-result", result
            assert result["error"] is None, result
            selected = [target for target in result["targets"] if target["requested"]]
            assert len(selected) == expected_requested, selected
            return run, result, selected[0] if selected else None

        def vstd(document):
            found = [t for t in document["targets"] if t["target"] == "vstd"]
            assert len(found) == 1 and not found[0]["requested"], diagnostic(document)
            return found[0]

        def verdicts(target):
            return target["report"]["summary"], sorted(
                (function["module"], function["function"], function["verdict"])
                for unit in target["report"]["crates"] for function in unit["functions"])

        def errors(run):
            return [line for line in run.stderr.decode().splitlines() if line.startswith("error")]

        store = root / "cache/dependency-store/v1"

        def fresh_copies(stored_vstd, target):
            # ADR 0053: each new project must perform its own compilation and
            # establish its own proof. No native artifacts cross target directories.
            entries = [p for p in store.iterdir() if not p.name.startswith(".")] if store.is_dir() else []
            assert entries == [], entries
            # Each copy is the whole workspace, lockfile included, verified from
            # its member's directory like the original.
            copies = []
            for name in ("worktree-1", "worktree-2"):
                shutil.copytree(workspace, root / name)
                copies.append(root / name / project.name)
            fresh_runs = []
            fresh_outputs = {}
            fresh_observations = {}
            fresh_timed_out = set()
            fresh_cancelled = set()
            try:
                for copy in copies:
                    fresh_observations[copy] = begin_verification(240)
                    process = subprocess.Popen([sail, "verify", "--incremental", "--output", "json"],
                                               cwd=copy, env=env, stdout=subprocess.PIPE,
                                               stderr=subprocess.PIPE, start_new_session=True)
                    fresh_runs.append((copy, process))
                for copy, process in fresh_runs:
                    try:
                        fresh_outputs[copy] = process.communicate(timeout=240)
                    except subprocess.TimeoutExpired:
                        fresh_timed_out.add(copy)
                        raise
            finally:
                # Reap every owned process group before scratch cleanup, even
                # after launch/read failure. Retain all partial producer output.
                for copy, process in fresh_runs:
                    if copy not in fresh_outputs:
                        if copy not in fresh_timed_out:
                            fresh_cancelled.add(copy)
                        try:
                            os.killpg(process.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                for copy, process in fresh_runs:
                    if copy not in fresh_outputs:
                        fresh_outputs[copy] = process.communicate(timeout=10)
                    out, err = fresh_outputs[copy]
                    finish_verification(fresh_observations[copy], process.returncode, out, err,
                                        timed_out=copy in fresh_timed_out,
                                        cancelled=copy in fresh_cancelled)
                    (evidence / (copy.parent.name + ".stdout.json")).write_bytes(out)
                    (evidence / (copy.parent.name + ".stderr")).write_bytes(err)
            own_vstd_receipts = []
            for copy, process in fresh_runs:
                out, err = fresh_outputs[copy]
                fresh = json.loads(out)
                assert process.returncode == 0 and fresh["exit_code"] == 0, err.decode()
                assert b"SAIL3131" not in err, err.decode()
                fresh_vstd = vstd(fresh)
                assert not fresh_vstd["reused"] and fresh_vstd["complete"], diagnostic(fresh)
                assert "dependency_store" not in fresh_vstd, diagnostic(fresh)
                assert verdicts(fresh_vstd) == verdicts(stored_vstd), diagnostic(fresh)
                assert fresh["execution"]["verifier_invocations"] >= 2, diagnostic(fresh)
                assert not any(note.startswith("SAIL3130:") for note in fresh["notes"]), fresh["notes"]
                own = [t for t in fresh["targets"] if t["requested"]]
                assert len(own) == 1 and not own[0]["reused"] and own[0]["complete"], diagnostic(fresh)
                assert verdicts(own[0]) == verdicts(target), (verdicts(own[0]), verdicts(target))
                if fresh["currentness"] == "historical":
                    established = bounded_verification([sail, "verify", "--incremental", "--output", "json"],
                                                       copy, env, 240)
                    (evidence / (copy.parent.name + "-current.stdout.json")).write_bytes(established.stdout)
                    (evidence / (copy.parent.name + "-current.stderr")).write_bytes(established.stderr)
                    fresh = json.loads(established.stdout)
                    assert established.returncode == 0 and fresh["currentness"] == "current", diagnostic(fresh)
                    assert all(t["complete"] and "dependency_store" not in t for t in fresh["targets"]), diagnostic(fresh)
                assert fresh["currentness"] == "current", diagnostic(fresh)
                own = [t for t in fresh["targets"] if t["requested"]]
                assert len(own) == 1 and own[0]["complete"], diagnostic(fresh)
                assert verdicts(own[0]) == verdicts(target), diagnostic(fresh)
                own_vstd_receipts.append(vstd(fresh))
            # Each Current project owns its next reuse; no foreign establishing
            # run can be substituted for the fresh project's own proof.
            again = bounded_verification([sail, "verify", "--incremental", "--output", "json"],
                                         copies[0], env, 240)
            repeated = json.loads(again.stdout)
            assert again.returncode == 0 and repeated["execution"]["verifier_invocations"] == 0, diagnostic(repeated)
            assert repeated["currentness"] == "current", diagnostic(repeated)
            assert all(t["reused"] and "dependency_store" not in t for t in repeated["targets"]), diagnostic(repeated)
            repeated_own = [t for t in repeated["targets"] if t["requested"]]
            assert len(repeated_own) == 1 and verdicts(repeated_own[0]) == verdicts(target), diagnostic(repeated)
            assert vstd(repeated)["establishing_run"] == own_vstd_receipts[0]["establishing_run"], diagnostic(repeated)
            assert vstd(repeated)["report"] == own_vstd_receipts[0]["report"], diagnostic(repeated)

        def native_compilation(stored_vstd, target):
            # A new plain run displays both fresh summaries (#184); its own
            # unchanged run must then remain silent, like Cargo's ordinary no-op.
            plain_workspace = root / "plain-seeded"
            shutil.copytree(workspace, plain_workspace)
            plain_copy = plain_workspace / project.name
            displayed = bounded_verification([sail, "verify"], plain_copy, env, 240)
            expected_stdout = b"".join(
                f"verification results:: {crate['verus']['verified']} verified, {crate['verus']['errors']} errors\n".encode()
                for report in (stored_vstd["report"], target["report"])
                for crate in report["crates"]
            )
            assert displayed.returncode == 0, displayed.stderr.decode()
            assert displayed.stdout == expected_stdout, (displayed.stdout, expected_stdout)
            unchanged = bounded_verification([sail, "verify"], plain_copy, env, 240)
            assert unchanged.returncode == 0 and unchanged.stdout == b"", (unchanged.stdout, unchanged.stderr)

            # A changed native compiler cannot be hidden by the earlier donor.
            # This is a real fresh-target compile against the same PATH and CC;
            # both stock and Sail must execute the replacement and fail.
            broken_workspace = root / "changed-compiler"
            shutil.copytree(workspace, broken_workspace)
            broken_project = broken_workspace / project.name
            marker = b"SAIL_TEST_CHANGED_CC"
            private_cc.write_text(
                f"#!{sys.executable}\nimport sys\n"
                "sys.stderr.write('SAIL_TEST_CHANGED_CC\\n')\nraise SystemExit(42)\n")
            try:
                commands = [
                    ("stock-changed-compiler", [cargo_verus, "verus", "verify", "--target-dir", str(root / "changed-compiler-oracle")], stock_env()),
                    ("sail-changed-compiler", [sail, "verify"], env),
                ]
                for label, arguments, command_env in commands:
                    failed = bounded_verification(arguments, broken_project, command_env, 240)
                    (evidence / (label + ".stdout")).write_bytes(failed.stdout)
                    (evidence / (label + ".stderr")).write_bytes(failed.stderr)
                    assert failed.returncode == 101 and marker in failed.stderr, (label, failed.returncode, failed.stderr.decode())
                    assert b"verification results:: 1 verified, 0 errors" not in failed.stdout, failed.stdout
            finally:
                private_cc.write_text(good_cc)
            assert not list(store.glob("*/manifest.json")), "native artifacts were published"

        if part != "scripts":
            # Keep PATH and CC fixed across a real native-tool replacement.
            # The good compiler establishes the donor; the failing compiler
            # must still be executed by a later fresh target (#187, ADR 0053).
            real_cc = shutil.which("cc", path=env.get("PATH"))
            assert real_cc, "managed acceptance requires a native C compiler"
            native_bin = root / "native-bin"
            native_bin.mkdir()
            private_cc = native_bin / "cc"
            good_cc = (f"#!{sys.executable}\nimport os, sys\n"
                       f"os.execv({real_cc!r}, [{real_cc!r}, *sys.argv[1:]])\n")
            private_cc.write_text(good_cc)
            private_cc.chmod(0o700)
            env["PATH"] = str(native_bin) + os.pathsep + env.get("PATH", "")
            env["CC"] = "cc"
            env["SAIL_DEPENDENCY_STORE_ENTRIES"] = "4"
        if part in ("all", "core"):
            # The pinned Verus's verdict on the failing proof, from a copy with its
            # own target, verified while the managed runs below proceed: a cold
            # build of vstd under stock Verus that need not hold up the job.
            shutil.copytree(workspace, root / "oracle")
            oracle_project = root / "oracle" / project.name
            oracle = {}
            def run_oracle():
                try:
                    oracle["run"] = bounded_verification(
                        [cargo_verus, "verus", "verify", "--target-dir", str(root / "oracle-target")],
                        oracle_project, stock_env(), 300)
                except Exception as failure:  # reported where the verdict is compared
                    oracle["error"] = failure
            oracle_thread = threading.Thread(target=run_oracle)
            oracle_thread.start()

            first_run, first, first_failure = managed()
            assert first_run.returncode != 0 and first["exit_code"] != 0, diagnostic(first)
            assert not first_failure["complete"] and not first_failure["reused"], diagnostic(first)
            assert first_failure["report"]["summary"]["failed"] == 1, first_failure
            assert b"postcondition not satisfied" in first_run.stderr, first_run.stderr.decode()
            assert first["currentness"] in ("current", "historical"), diagnostic(first)
            # As for a passing run, a historical cold run establishes nothing; the
            # next Current run does.
            if first["currentness"] == "historical":
                first_run, first, first_failure = managed()
                assert first_run.returncode != 0 and first["currentness"] == "current", diagnostic(first)
            established_vstd = vstd(first)
            assert established_vstd["complete"], diagnostic(first)
            # ADR 0053: native-artifact publication is quarantined. Proofs in
            # this project's own managed target still establish reusable receipts.
            stored = [p.name for p in store.iterdir() if not p.name.startswith(".")] if store.is_dir() else []
            events = evidence / (managed_calls[-1] + ".events.jsonl")
            assert stored == [], (stored, events.read_text() if events.exists() else None)

            # The failing proof again: only the selected crate is verified, with the
            # same verdicts and errors; vstd is reused from the failing run.
            again_run, again, again_failure = managed()
            assert again_run.returncode == first_run.returncode, diagnostic(again)
            assert again["execution"]["verifier_invocations"] == 1, diagnostic(again)
            reused_vstd = vstd(again)
            assert reused_vstd["reused"] and reused_vstd["complete"], diagnostic(again)
            assert reused_vstd["establishing_run"] == established_vstd["establishing_run"], diagnostic(again)
            assert reused_vstd["report"] == established_vstd["report"], "reuse must preserve vstd's report"
            assert not again_failure["reused"] and not again_failure["complete"], diagnostic(again)
            assert verdicts(again_failure) == verdicts(first_failure), (verdicts(again_failure), verdicts(first_failure))
            assert errors(again_run) == errors(first_run), (errors(again_run), errors(first_run))

            source.write_text(good)
            cold_run, cold, target = managed()
            assert cold["execution"]["verifier_invocations"] == 1, diagnostic(cold)
            assert vstd(cold)["reused"], diagnostic(cold)
            assert cold_run.returncode == 0 and cold["exit_code"] == 0, cold_run.stderr.decode()
            assert target["complete"] and not target["reused"], target
            assert cold["currentness"] in ("current", "historical"), cold
            assert target["report"]["summary"]["verified"] == 1, target

            # Discovering compiler-declared inputs cannot authorize cold reuse.
            # Establish a fully observed proof before requiring zero fresh work.
            if cold["currentness"] == "historical":
                established_run, established, target = managed()
                assert established_run.returncode == 0 and established["currentness"] == "current", diagnostic(established)
                assert target["complete"] and not target["reused"], target
                assert established["execution"]["verifier_invocations"] > 0, established

            warm_run, warm, reused = managed()
            assert warm_run.returncode == 0 and reused["reused"] and reused["complete"], diagnostic(warm)
            assert warm["execution"]["verifier_invocations"] == 0, warm["execution"]
            assert warm["currentness"] == "current", diagnostic(warm)
            assert vstd(warm)["reused"], diagnostic(warm)
            # Cargo found every unit fresh: no macro crate or vstd was rebuilt.
            assert b"Compiling" not in warm_run.stderr, warm_run.stderr.decode()
            assert reused["establishing_run"] == target["establishing_run"], reused
            assert reused["report"] == target["report"], "reuse must preserve establishing report"

            stored_vstd = vstd(warm)

            # Caller target selection cannot partition the owned build or
            # exclude source files from managed metadata (#166).
            caller_target = env.pop("CARGO_TARGET_DIR", None)
            try:
                for directory in (str(project), str(project / "src"), None):
                    if directory is None:
                        env.pop("CARGO_TARGET_DIR", None)
                    else:
                        env["CARGO_TARGET_DIR"] = directory
                    switched_run, switched, _ = managed()
                    assert switched_run.returncode == 0, diagnostic(switched)
                    assert switched["context_sha256"] == warm["context_sha256"], diagnostic(switched)
                    assert switched["input_revision"] == warm["input_revision"], diagnostic(switched)
                    assert switched["execution"]["verifier_invocations"] == 0, diagnostic(switched)
                    assert not (project / ".rustc_info.json").exists()
                    assert not (project / "src/.rustc_info.json").exists()
                env["CARGO_TARGET_DIR"] = str(project)
                source.write_text(bad)
                changed_run, changed, changed_target = managed()
                assert changed_run.returncode != 0 and not changed_target["reused"], diagnostic(changed)
                assert changed["input_revision"] != warm["input_revision"], diagnostic(changed)
                source.write_text(good)
                restored_run, restored, target = managed()
                assert restored_run.returncode == 0 and target["complete"], diagnostic(restored)
                assert restored["currentness"] == "current", diagnostic(restored)
            finally:
                if caller_target is None:
                    env.pop("CARGO_TARGET_DIR", None)
                else:
                    env["CARGO_TARGET_DIR"] = caller_target

            # The installed CLI and MCP must use that same complete evidence through
            # durable jobs, rather than substitute a Cargo 'fresh' message for proof.
            submitted = recorded_cli("cli-submit", [sail, "verify", "--background", "--output", "json"])
            job = json.loads(submitted.stdout)
            assert job["id"] and job["state"] in ("queued", "running", "completed"), job
            deadline = time.monotonic() + 120
            while True:
                inspected = recorded_cli("cli-result", [sail, "jobs", "result", job["id"], "--output", "json"])
                response = json.loads(inspected.stdout)
                assert response["response"] == "result", response
                if response["value"]["result"] is not None:
                    complete = response["value"]["result"]
                    break
                assert response["value"]["job"]["state"] in ("queued", "running"), response
                assert time.monotonic() < deadline, "background job did not finish"
                time.sleep(.1)
            assert complete["execution"]["verifier_invocations"] == 0, complete
            assert complete["currentness"] == "current", complete
            assert next(t for t in complete["targets"] if t["requested"])["report"] == target["report"]

            # An agent's session and terminal variables are not proof inputs: the
            # MCP server below runs with different ones and must reuse the CLI's proof.
            agent_env = dict(env, CLAUDE_CODE_SESSION_ID="managed-acceptance-agent",
                             TERM_SESSION_ID="another-terminal", TERM="dumb")
            mcp = subprocess.Popen([sail, "mcp"], cwd=project, env=agent_env,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, text=True)
            def exchange(message):
                mcp.stdin.write(json.dumps(message) + "\n")
                mcp.stdin.flush()
                assert select.select([mcp.stdout], [], [], 30)[0], "MCP response timed out"
                document = json.loads(mcp.stdout.readline())
                assert document.get("id") == message["id"] and "error" not in document, document
                return document["result"]
            try:
                exchange({"jsonrpc":"2.0", "id":1, "method":"initialize", "params":{
                    "protocolVersion":"2025-11-25", "capabilities":{},
                    "clientInfo":{"name":"managed-acceptance", "version":"1"}}})
                mcp.stdin.write(json.dumps({"jsonrpc":"2.0", "method":"notifications/initialized"}) + "\n")
                started = exchange({"jsonrpc":"2.0", "id":2, "method":"tools/call", "params":{
                    "name":"start_verification", "arguments":{"project_dir":str(project)}}})
                assert not started["isError"], started
                mcp_job = started["structuredContent"]["id"]
                status = exchange({"jsonrpc":"2.0", "id":3, "method":"tools/call", "params":{
                    "name":"verification_status", "arguments":{"job_id":mcp_job}}})
                assert not status["isError"] and status["structuredContent"]["value"]["id"] == mcp_job, status
                deadline = time.monotonic() + 120
                request_id = 4
                while True:
                    result = exchange({"jsonrpc":"2.0", "id":request_id, "method":"tools/call", "params":{
                        "name":"verification_result", "arguments":{"job_id":mcp_job, "detail":"targets"}}})
                    assert not result["isError"], result
                    value = result["structuredContent"]["value"]
                    if value["result"] is not None:
                        assert value["job"]["state"] == "completed", value
                        assert value["result"]["execution"]["verifier_invocations"] == 0, value
                        assert next(t for t in value["result"]["targets"] if t["requested"])["report"] == target["report"]
                        break
                    assert time.monotonic() < deadline, "MCP job did not finish"
                    request_id += 1
                    time.sleep(.1)
                # Blocking verify reuses the same Current receipt, as the CLI does,
                # and names the run that established the reused report.
                verified = exchange({"jsonrpc":"2.0", "id":request_id + 1, "method":"tools/call", "params":{
                    "name":"verify", "arguments":{"project_dir":str(project), "detail":"targets"}}})
                assert not verified["isError"], verified
                blocking = verified["structuredContent"]["project_result"]
                assert blocking["execution"]["verifier_invocations"] == 0, blocking["execution"]
                assert next(t for t in blocking["targets"] if t["requested"])["report"] == target["report"]
                assert target["establishing_run"] in verified["content"][0]["text"], verified["content"][0]["text"]
                # The default reply is agent-sized: requested targets only, JSON once.
                compact = exchange({"jsonrpc":"2.0", "id":request_id + 2, "method":"tools/call", "params":{
                    "name":"verify", "arguments":{"project_dir":str(project)}}})
                assert not compact["isError"], compact
                assert len(compact["content"]) == 1, compact["content"]
                summary = compact["structuredContent"]["summary"]
                assert summary.startswith("VERIFIED: all "), summary
                assert len(json.dumps(compact)) < 64_000, len(json.dumps(compact))
            finally:
                mcp.stdin.close()
                try:
                    mcp.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    mcp.kill()
                    mcp.wait(timeout=10)

            # Delete only the selected receipt. Dependencies must stay warm and a fresh
            # selected proof must replace missing evidence rather than returning nothing.
            contexts = list((root / "cache/projects").glob("*/" + warm["context_sha256"]))
            assert len(contexts) == 1, contexts
            receipts = list((contexts[0] / "receipts").glob("*.json"))
            selected_receipt = next(path for path in receipts
                                    if json.loads(path.read_text())["package_id"] == reused["package_id"])
            selected_receipt.unlink()
            refill_run, refill, fresh = managed()
            assert refill_run.returncode == 0 and not fresh["reused"] and fresh["complete"], refill
            assert refill["execution"]["verifier_invocations"] == 1, refill["execution"]

            # Without --output json the same missing receipt is not re-verified:
            # Cargo finds the target up to date, and Sail prints what plain
            # Cargo-Verus prints for it, once (ADR 0046).
            selected_receipt.unlink()
            human = bounded_verification([sail, "verify"], project, env, 240)
            assert human.returncode == 0, human.stderr.decode()
            lines = human.stderr.decode().splitlines()
            assert len([l for l in lines if "Finished" in l]) == 1, lines
            assert not [l for l in lines if "Checking" in l or "Compiling" in l], lines
            assert human.stdout == b"", human.stdout
            refill_run, refill, fresh = managed()
            assert refill_run.returncode == 0 and not fresh["reused"] and fresh["complete"], refill

            # Change a proof into a failure without changing size or mtime. This verifies
            # the content/receipt binding independently of Cargo's timestamp behaviour.
            stamp = source.stat()
            source.write_text(bad)
            os.utime(source, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
            failed_run, failed, failure = managed()
            assert failed_run.returncode != 0 and not failure["reused"] and not failure["complete"], failed
            assert failure["report"]["summary"]["failed"] == 1, failure
            assert failed["execution"]["verifier_invocations"] >= 1, failed["execution"]
            oracle_thread.join()
            assert "error" not in oracle, oracle.get("error")
            plain = oracle["run"]
            assert plain.returncode == failed_run.returncode, (plain.returncode, failed_run.returncode)
            assert b"postcondition not satisfied" in plain.stderr, plain.stderr.decode()
            assert b"postcondition not satisfied" in failed_run.stderr, failed_run.stderr.decode()

            # Repeat a focused check over byte-identical source. cargo verus focus
            # builds in <target>/verus-partial with checksum freshness, so the
            # forced cleanup must clean that directory; cleaning the outer target
            # left the crate fresh, ran no verifier and failed with SAIL3107.
            source.write_text(good)
            for attempt in range(3):
                focus_run, focused, focus_target = managed("--", "--verify-root", "--verify-function", "max")
                assert focus_run.returncode == 0 and focused["exit_code"] == 0, (attempt, diagnostic(focused))
                assert focused["execution"]["verifier_invocations"] >= 1, (attempt, focused["execution"])
                assert not focus_target["reused"], (attempt, focus_target)
                source.write_text(good)

            # Plain `sail verify` asks Verus for no attribution logs (ADR 0047):
            # its output is plain Cargo-Verus's, it writes no logs, and a later
            # `--output json` run that reuses its proof says the obligation detail
            # was not recorded, with Verus's own verdicts.
            source.write_text(good + "// plain sail verify establishes this proof\n")
            logs_before = set(root.glob("cache/projects/*/*/runs/*/sidecars/*/logs"))
            # The second run reuses the first's proof when the first was Current
            # (and then, like Cargo's no-op, prints nothing), or establishes it.
            for attempt in range(2):
                human = bounded_verification([sail, "verify"], project, env, 240)
                assert human.returncode == 0, human.stderr.decode()
                if attempt == 0:
                    assert b"verification results:: 1 verified, 0 errors" in human.stdout, human.stdout.decode()
            assert set(root.glob("cache/projects/*/*/runs/*/sidecars/*/logs")) == logs_before, "plain sail verify wrote attribution logs"
            detail_run, detail, detail_target = managed()
            assert detail_run.returncode == 0 and detail_target["reused"], diagnostic(detail)
            attribution = detail_target["report"]["attribution"]
            assert attribution["status"] == "unavailable", attribution
            assert attribution["notes"][0].startswith("obligation detail not recorded"), attribution
            assert verdicts(detail_target)[0]["verified"] == 1, verdicts(detail_target)
            assert all(f["obligations"] is None for c in detail_target["report"]["crates"] for f in c["functions"])
            source.write_text(good)

            # Pruning the workspace's managed directory costs only a rebuild: the
            # next run reuses nothing, verifies again with the same report, and the
            # run after it reuses the new receipt.
            listed = json.loads(recorded_cli("projects-list", [sail, "cache", "projects", "list", "--json"]).stdout)
            mine = [p for p in listed["projects"] if p["workspace"] and Path(p["workspace"]).resolve() == workspace.resolve()]
            assert len(mine) == 1 and mine[0]["exists"] and mine[0]["bytes"] > 0, listed
            pruned = json.loads(recorded_cli("projects-prune", [sail, "cache", "projects", "prune", "--all", "--json"]).stdout)
            assert [p["key"] for p in pruned["removed"]] == [mine[0]["key"]] and not pruned["busy"], pruned
            assert not list((root / "cache/projects").glob("*/" + warm["context_sha256"])), "pruned context survived"
            rebuilt_run, rebuilt, rebuilt_target = managed()
            assert rebuilt_run.returncode == 0 and rebuilt["exit_code"] == 0, diagnostic(rebuilt)
            assert not rebuilt_target["reused"] and rebuilt_target["complete"], diagnostic(rebuilt)
            assert not vstd(rebuilt)["reused"] and vstd(rebuilt)["complete"], diagnostic(rebuilt)
            assert "dependency_store" not in vstd(rebuilt), diagnostic(rebuilt)
            assert rebuilt["execution"]["verifier_invocations"] >= 2, rebuilt["execution"]
            assert rebuilt_target["report"]["summary"] == target["report"]["summary"], diagnostic(rebuilt)
            if rebuilt["currentness"] == "historical":
                rebuilt_run, rebuilt, rebuilt_target = managed()
                assert rebuilt_run.returncode == 0 and rebuilt["currentness"] == "current", diagnostic(rebuilt)
            reread_run, reread, reread_target = managed()
            assert reread_run.returncode == 0 and reread_target["reused"] and reread_target["complete"], diagnostic(reread)
            assert reread["execution"]["verifier_invocations"] == 0, reread["execution"]
            assert reread_target["establishing_run"] == rebuilt_target["establishing_run"], reread_target
            assert reread_target["report"] == rebuilt_target["report"], "reuse must preserve the rebuilt report"

        if part in ("copies", "native"):
            # Standalone CI parts establish their own passing member project;
            # no receipt, target directory or daemon is shared with another job.
            print(f"managed acceptance: {part} donor starting", flush=True)
            source.write_text(good)
            donor_run, donor, target = managed()
            assert donor_run.returncode == donor["exit_code"] == 0, diagnostic(donor)
            assert donor["currentness"] in ("current", "historical"), diagnostic(donor)
            assert target["complete"] and not target["reused"], diagnostic(donor)
            assert target["report"]["summary"]["verified"] == 1, target
            assert vstd(donor)["complete"] and not vstd(donor)["reused"], diagnostic(donor)
            assert "dependency_store" not in vstd(donor), diagnostic(donor)
            assert donor["execution"]["verifier_invocations"] >= 2, diagnostic(donor)
            donor_verdicts = verdicts(vstd(donor)), verdicts(target)
            if donor["currentness"] == "historical":
                donor_run, donor, target = managed()
                assert donor_run.returncode == donor["exit_code"] == 0, diagnostic(donor)
                assert target["complete"] and not target["reused"], diagnostic(donor)
            assert donor["currentness"] == "current", diagnostic(donor)
            assert (verdicts(vstd(donor)), verdicts(target)) == donor_verdicts, diagnostic(donor)
            assert all(t["complete"] and "dependency_store" not in t for t in donor["targets"]), diagnostic(donor)
            stored_vstd = vstd(donor)

        if part in ("all", "copies"):
            print("managed acceptance: fresh copies starting", flush=True)
            fresh_copies(stored_vstd, target)
            print("managed acceptance: fresh copies passed", flush=True)
        if part in ("all", "native"):
            print("managed acceptance: native compilation starting", flush=True)
            native_compilation(stored_vstd, target)
            print("managed acceptance: native compilation passed", flush=True)

        if part in ("all", "scripts"):
            if part == "scripts":
                # The project as the core cases leave it: proof and vstd
                # established by a Current run.
                source.write_text(good)
                established_run, established, _ = managed()
                assert established_run.returncode == 0, diagnostic(established)
                if established["currentness"] != "current":
                    established_run, established, _ = managed()
                    assert established["currentness"] == "current", diagnostic(established)

            # Build scripts must not stop reuse. The crate gains a build script
            # that declares no inputs (Cargo then watches the whole package),
            # serde (whose build scripts generate the included OUT_DIR/private.rs)
            # and a dev-dependency with a build script that `cargo verus verify`
            # never builds.
            # #174: two versions of one scripted package, one normal and one
            # dev-only. Cleaning the unscheduled version by package name also
            # removes the scheduled sibling. The OUT_DIR marker changes on
            # every real script execution, including after a destructive clean.
            workspace_manifest = workspace / "Cargo.toml"
            workspace_manifest.write_text(workspace_manifest.read_text()
                + 'exclude = ["schedule-old", "schedule-new"]\n')
            for directory, version, label in [
                ("schedule-old", "1.0.0", "old"),
                ("schedule-new", "2.0.0", "new"),
            ]:
                package = workspace / directory
                (package / "src").mkdir(parents=True)
                (package / "Cargo.toml").write_text(
                    f'[package]\nname = "sail-schedule-fixture"\nversion = "{version}"\n'
                    'edition = "2021"\n')
                (package / "src/lib.rs").write_text("")
                (package / "build.rs").write_text(
                    'fn main() {\n'
                    '    println!("cargo::rerun-if-changed=build.rs");\n'
                    '    println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_OS");\n'
                    '    println!("cargo::rerun-if-env-changed=CARGO_FEATURE_SAIL_FIXTURE");\n'
                    '    let out = std::env::var("OUT_DIR").unwrap();\n'
                    '    let time = std::time::SystemTime::now().duration_since('
                    'std::time::UNIX_EPOCH).unwrap().as_nanos();\n'
                    f'    std::fs::write(format!("{{out}}/sail-schedule-marker"), '
                    f'format!("{label}:{{time}}")).unwrap();\n'
                    '}\n')
            manifest = project / "Cargo.toml"
            text = manifest.read_text()
            assert "[dependencies]\n" in text, text
            text = text.replace("[dependencies]\n", '[dependencies]\nserde = "=1.0.228"\n'
                'schedule-current = { package = "sail-schedule-fixture", path = "../schedule-new" }\n', 1)
            manifest.write_text(text + '\n[dev-dependencies]\nlibc = "0.2"\n'
                'schedule-old = { package = "sail-schedule-fixture", path = "../schedule-old" }\n')
            (project / "build.rs").write_text("fn main() {}\n")
            scripts_run, scripts, scripts_target = managed()
            assert scripts_run.returncode == 0 and scripts_target["complete"], diagnostic(scripts)
            # Issue #177: as for plain Cargo-Verus, the new dependencies leave
            # vstd's closure as it was, so vstd keeps its receipt and only the
            # crate is verified again.
            assert scripts["execution"]["verifier_invocations"] == 1, diagnostic(scripts)
            assert not scripts_target["reused"], diagnostic(scripts)
            assert vstd(scripts)["reused"], diagnostic(scripts)
            assert b"Compiling vstd" not in scripts_run.stderr, scripts_run.stderr.decode()
            # Issues #120 and #121: the cold run is Current (the generated include
            # and the unbuilt dev-dependency script no longer withhold it), and the
            # unchanged run after it verifies nothing.
            assert scripts["currentness"] == "current", diagnostic(scripts)
            def schedule_marker():
                markers = list((root / "cache/projects").glob(
                    "*/compiler-artifacts/*/target/**/out/sail-schedule-marker"))
                assert len(markers) == 1, markers
                marker = markers[0].read_text()
                assert marker.startswith("new:"), marker
                return marker
            script_execution = schedule_marker()
            unchanged_run, unchanged, unchanged_target = managed()
            assert unchanged_run.returncode == 0 and unchanged["currentness"] == "current", diagnostic(unchanged)
            assert unchanged["execution"]["verifier_invocations"] == 0, diagnostic(unchanged)
            assert unchanged_target["reused"], diagnostic(unchanged)
            assert schedule_marker() == script_execution, "unchanged run rebuilt scheduled sibling"
            # A second consecutive hit must also keep the same established fact.
            again_run, again, _ = managed()
            assert again_run.returncode == 0 and again["currentness"] == "current", diagnostic(again)
            assert schedule_marker() == script_execution, "second unchanged run rebuilt scheduled sibling"
            # Issue #119: a comment-only edit verifies only the edited crate; vstd
            # keeps its build-script records and its receipt.
            source.write_text(source.read_text() + "// comment-only edit\n")
            edited_run, edited, edited_target = managed()
            assert edited_run.returncode == 0 and edited["currentness"] == "current", diagnostic(edited)
            assert edited["execution"]["verifier_invocations"] == 1, diagnostic(edited)
            assert not edited_target["reused"] and edited_target["complete"], diagnostic(edited)
            assert vstd(edited)["reused"], diagnostic(edited)
            assert b"Compiling vstd" not in edited_run.stderr, edited_run.stderr.decode()
            assert schedule_marker() != script_execution, "changed source incorrectly kept old scheduling memo"

            # #294/#298: rustix probes metadata on stdout and declares
            # Cargo-generated inputs. The probe must not churn the package
            # directory or force every script and vstd to rebuild next time.
            probe_script = workspace / "schedule-new/build.rs"
            original_probe_script = probe_script.read_text()
            probe_script.write_text(
                'fn main() {\n'
                '    println!("cargo::rerun-if-changed=build.rs");\n'
                '    println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_OS");\n'
                '    println!("cargo::rerun-if-env-changed=CARGO_FEATURE_SAIL_FIXTURE");\n'
                '    let rustc = std::env::var_os("RUSTC").unwrap();\n'
                '    let probe = std::process::Command::new(rustc)\n'
                '        .args(["src/lib.rs", "--crate-type", "lib", "--emit=metadata",\n'
                '               "--crate-name", "sail_probe", "-o", "-"])\n'
                '        .output().unwrap();\n'
                '    assert!(probe.status.success(), "{:?}", probe);\n'
                '    assert!(!probe.stdout.is_empty());\n'
                '    let out = std::env::var("OUT_DIR").unwrap();\n'
                '    let time = std::time::SystemTime::now().duration_since('
                'std::time::UNIX_EPOCH).unwrap().as_nanos();\n'
                '    std::fs::write(format!("{out}/sail-schedule-marker"), '
                'format!("new:{time}")).unwrap();\n'
                '}\n')
            probe_run, probe, _ = managed()
            assert probe_run.returncode == 0 and probe["currentness"] == "current", diagnostic(probe)
            probe_execution = schedule_marker()
            for label in ("first", "second"):
                quiet_run, quiet, quiet_target = managed()
                assert quiet_run.returncode == 0 and quiet["currentness"] == "current", (label, diagnostic(quiet))
                assert quiet["execution"]["verifier_invocations"] == 0 and quiet_target["reused"], (label, diagnostic(quiet))
                assert not re.search(rb"(?m)^ *(Compiling|Checking) ", quiet_run.stderr), (label, quiet_run.stderr.decode())
                assert schedule_marker() == probe_execution, f"{label} run repeated the compiler probe"
            probe_script.write_text(original_probe_script)

            # Generated files stay bound by content (#120): the build script
            # declares an environment variable and writes its value into a file
            # the crate includes. An unchanged value reuses the proof; a changed
            # value must verify the crate again.
            (project / "build.rs").write_text(
                'fn main() {\n'
                '    println!("cargo::rerun-if-env-changed=DEMO_GENERATED_VALUE");\n'
                '    println!("cargo::rerun-if-env-changed=CARGO_TARGET_DIR");\n'
                '    assert!(std::env::var_os("CARGO_TARGET_DIR").is_none(), "declared caller target changed");\n'
                '    let value = std::env::var("DEMO_GENERATED_VALUE").unwrap_or_default();\n'
                '    let out = std::env::var("OUT_DIR").unwrap();\n'
                '    std::fs::write(format!("{out}/generated.rs"),\n'
                '        format!("pub const GENERATED: &str = {value:?};\\n")).unwrap();\n'
                '}\n')
            source.write_text('include!(concat!(env!("OUT_DIR"), "/generated.rs"));\n' + good)
            env["DEMO_GENERATED_VALUE"] = "first"
            declared_caller_target = env.pop("CARGO_TARGET_DIR", None)
            try:
                first_run, first_generated, first_target = managed()
                assert first_run.returncode == 0 and first_target["complete"], diagnostic(first_generated)
                if first_generated["currentness"] != "current":
                    first_run, first_generated, first_target = managed()
                    assert first_generated["currentness"] == "current", diagnostic(first_generated)
                same_run, same, same_target = managed()
                assert same["currentness"] == "current", diagnostic(same)
                assert same["execution"]["verifier_invocations"] == 0 and same_target["reused"], diagnostic(same)
                env["DEMO_GENERATED_VALUE"] = "second"
                changed_run, changed, changed_target = managed()
                assert changed_run.returncode == 0 and changed_target["complete"], diagnostic(changed)
                assert changed["execution"]["verifier_invocations"] >= 1, diagnostic(changed)
                assert not changed_target["reused"], diagnostic(changed)
                assert changed_target["establishing_run"] != same_target["establishing_run"], diagnostic(changed)
                env["CARGO_TARGET_DIR"] = str(root / "declared-target-change")
                # Cargo stops in the failing build script, before it creates
                # the requested artifact; no requested proof may be reported.
                declared_run, declared, _ = managed(expected_requested=0)
                assert declared_run.returncode == declared["exit_code"] == 101, diagnostic(declared)
                assert declared["currentness"] == "unknown", diagnostic(declared)
                assert b"declared caller target changed" in declared_run.stderr, declared_run.stderr.decode()
                assert declared["context_sha256"] == changed["context_sha256"], diagnostic(declared)
            finally:
                env.pop("DEMO_GENERATED_VALUE", None)
                if declared_caller_target is None:
                    env.pop("CARGO_TARGET_DIR", None)
                else:
                    env["CARGO_TARGET_DIR"] = declared_caller_target

        if part in ("all", "core"):
            # Cargo config can name a source tree as its ordinary target.
            # Managed compilation uses an owned target, so metadata must not
            # exclude that source tree (#189). This tiny no-vstd compiler
            # fixture adds no cold vstd verification to the phase.
            lock = (workspace / "Cargo.lock").read_text()
            builtin_package = next(block for block in lock.split("[[package]]")
                                   if '\nname = "verus_builtin"\n' in block)
            builtin_version = next(json.loads(line.split("=", 1)[1].strip())
                                   for line in builtin_package.splitlines() if line.startswith("version = "))
            for selection in ("src", "."):
                fixture = root / ("metadata-source-" + ("root" if selection == "." else "src"))
                (fixture / "src").mkdir(parents=True)
                (fixture / ".cargo").mkdir()
                (fixture / ".cargo/config.toml").write_text(f'[build]\ntarget-dir = "{selection}"\n')
                (fixture / "Cargo.toml").write_text(
                    '[package]\nname="metadata_source"\nversion="0.1.0"\nedition="2021"\n'
                    f'[dependencies]\nverus_builtin="={builtin_version}"\n'
                    '[package.metadata.verus]\nverify=true\nno-vstd=true\n')
                (fixture / "src/lib.rs").write_text('extern crate verus_builtin; mod leaf; pub use leaf::answer;\n')
                (fixture / "src/leaf.rs").write_text('pub fn answer() -> u32 { 42 }\n')
                fixture_env = dict(env)
                fixture_env.pop("CARGO_TARGET_DIR", None)
                def fixture_run(label):
                    result = bounded_verification([sail, "verify", "--output", "json"], fixture, fixture_env, 120)
                    (evidence / f"metadata-{fixture.name}-{label}.stdout.json").write_bytes(result.stdout)
                    (evidence / f"metadata-{fixture.name}-{label}.stderr").write_bytes(result.stderr)
                    return result, json.loads(result.stdout)
                initial_run, initial = fixture_run("initial")
                assert initial_run.returncode == 0, diagnostic(initial)
                if initial["currentness"] != "current":
                    established_run, established = fixture_run("established")
                    assert established_run.returncode == 0 and established["currentness"] == "current", diagnostic(established)
                warm_run, warm_fixture = fixture_run("warm")
                assert warm_run.returncode == 0 and warm_fixture["execution"]["verifier_invocations"] == 0, diagnostic(warm_fixture)
                assert warm_fixture["currentness"] == "current", diagnostic(warm_fixture)
                assert not (fixture / ".rustc_info.json").exists()
                assert not (fixture / "src/.rustc_info.json").exists()
                (fixture / "src/leaf").mkdir()
                (fixture / "src/leaf/mod.rs").write_text('pub fn answer() -> u32 { 43 }\n')
                ambiguous_run, ambiguous = fixture_run("ambiguous")
                assert ambiguous_run.returncode != 0 and b"E0761" in ambiguous_run.stderr, diagnostic(ambiguous)
                assert ambiguous["execution"]["verifier_invocations"] > 0, diagnostic(ambiguous)
                assert ambiguous["input_revision"] != warm_fixture["input_revision"], diagnostic(ambiguous)

            # A Sail cache is excluded from snapshots and therefore cannot
            # also contain package input. Refuse managed receipts, including
            # physical aliases and future cache suffixes; plain Cargo fallback
            # must never describe its result as Current (#189).
            for mode in ("outside", "source", "alias", "prefix", "ancestor"):
                container = root / ("cache-overlap-" + mode)
                fixture = container / "package"
                (fixture / "src").mkdir(parents=True)
                (fixture / "Cargo.toml").write_text(
                    '[package]\nname="cache_overlap"\nversion="0.1.0"\nedition="2021"\n'
                    f'[dependencies]\nverus_builtin="={builtin_version}"\n'
                    '[package.metadata.verus]\nverify=true\nno-vstd=true\n')
                (fixture / "src/lib.rs").write_text('extern crate verus_builtin; mod leaf; pub use leaf::answer;\n')
                (fixture / "src/leaf.rs").write_text('pub fn answer() -> u32 { 42 }\n')
                cache = container / "cache"
                if mode == "source":
                    cache = fixture / "src"
                elif mode == "alias":
                    cache.symlink_to(fixture / "src", target_is_directory=True)
                elif mode == "prefix":
                    cache = fixture / "src/not-created/cache"
                elif mode == "ancestor":
                    cache = container
                fixture_env = dict(env, SAIL_CACHE_DIR=str(cache),
                                   CARGO_TARGET_DIR=str(container / "caller-target"))
                def cache_fixture_run(label):
                    result = bounded_verification([sail, "verify", "--output", "json"], fixture, fixture_env, 120)
                    (evidence / f"cache-{mode}-{label}.stdout.json").write_bytes(result.stdout)
                    (evidence / f"cache-{mode}-{label}.stderr").write_bytes(result.stderr)
                    return result, json.loads(result.stdout)
                initial_run, initial = cache_fixture_run("initial")
                assert initial_run.returncode == 0, diagnostic(initial)
                if mode == "outside" and initial["currentness"] != "current":
                    established_run, established = cache_fixture_run("established")
                    assert established_run.returncode == 0 and established["currentness"] == "current", diagnostic(established)
                warm_run, warm_fixture = cache_fixture_run("warm")
                assert warm_run.returncode == 0, diagnostic(warm_fixture)
                if mode == "outside":
                    assert warm_fixture["currentness"] == "current", diagnostic(warm_fixture)
                    assert warm_fixture["execution"]["verifier_invocations"] == 0, diagnostic(warm_fixture)
                else:
                    for result, document in ((initial_run, initial), (warm_run, warm_fixture)):
                        assert document.get("currentness") != "current", diagnostic(document)
                        assert result.stderr.count(b"note[SAIL3112]") == 1, result.stderr.decode()
                        assert b"physically disjoint" in result.stderr, result.stderr.decode()
                    assert not (cache / "projects").exists(), "overlapping cache received managed proof state"
                    # Cargo itself need not detect an added alternative module
                    # in a warm target. Request a fresh ordinary target for the
                    # fallback error control, without editing any source bytes.
                    fixture_env["CARGO_TARGET_DIR"] = str(container / "fresh-caller-target")
                (fixture / "src/leaf").mkdir()
                (fixture / "src/leaf/mod.rs").write_text('pub fn answer() -> u32 { 43 }\n')
                ambiguous_run, ambiguous = cache_fixture_run("ambiguous")
                assert ambiguous_run.returncode != 0 and b"E0761" in ambiguous_run.stderr, diagnostic(ambiguous)
                if mode == "outside":
                    assert ambiguous["execution"]["verifier_invocations"] > 0, diagnostic(ambiguous)
                    assert ambiguous["input_revision"] != warm_fixture["input_revision"], diagnostic(ambiguous)
                else:
                    assert ambiguous.get("currentness") != "current", diagnostic(ambiguous)
                    assert not (cache / "projects").exists(), "overlapping cache received managed proof state"
            print("managed acceptance: failing first runs keep vstd, cold, unchanged, CLI/MCP jobs, missing receipt, restored-mtime failing edit, repeated focus, plain runs without logs and prune passed")
        if part in ("all", "scripts"):
            print("managed acceptance: build-script reuse passed")
    finally:
        stop_daemon()
