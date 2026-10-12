#!/usr/bin/env python3
"""Bounded real-tool acceptance for managed proof receipts; never a benchmark."""

import json
import os
from pathlib import Path
import shutil
import select
import signal
import subprocess
import sys
import tempfile
import time

sail = str(Path(sys.argv[1]).resolve())
cargo_verus = os.environ.get("SAIL_CARGO_VERUS") or shutil.which("cargo-verus")
if not cargo_verus:
    raise SystemExit("managed acceptance requires the pinned cargo-verus")

def bounded_verification(arguments, cwd, env, timeout):
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
        raise
    return subprocess.CompletedProcess(arguments, process.returncode, stdout, stderr)

with tempfile.TemporaryDirectory(prefix="sail-managed-acceptance-") as temporary:
    root = Path(temporary)
    evidence = Path(os.environ.get("SAIL_MANAGED_ACCEPTANCE_OUT", str(root / "evidence"))).resolve()
    evidence.mkdir(mode=0o700, parents=True, exist_ok=True)
    managed_calls = []
    env = dict(os.environ, SAIL_CACHE_DIR=str(root / "cache"))
    subprocess.run([cargo_verus, "verus", "new", "--lib", "demo"], cwd=root,
                   env=env, check=True, stdout=subprocess.DEVNULL)
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
        project = root / "demo"
        source = project / "src/lib.rs"
        good = """use vstd::prelude::*;
    verus! {
    pub fn max(a: u32, b: u32) -> (r: u32)
        ensures r >= a, r >= b,
    { if a > b { a } else { b } }
    }
    """
        source.write_text(good)

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

        def managed(*args):
            label = f"managed-{len(managed_calls) + 1:02d}"
            managed_calls.append(label)
            try:
                run = bounded_verification([sail, "verify", "--incremental", "--output", "json", *args],
                                           project, env, 240)
            except subprocess.TimeoutExpired as failure:
                (evidence / (label + ".stdout.json")).write_bytes(failure.stdout or b"")
                (evidence / (label + ".stderr")).write_bytes(failure.stderr or b"")
                raise
            (evidence / (label + ".stdout.json")).write_bytes(run.stdout)
            (evidence / (label + ".stderr")).write_bytes(run.stderr)
            result = json.loads(run.stdout)
            assert result["kind"] == "sail-project-result", result
            assert result["error"] is None, result
            selected = [target for target in result["targets"] if target["requested"]]
            assert len(selected) == 1, selected
            return run, result, selected[0]

        cold_run, cold, target = managed()
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
        assert reused["establishing_run"] == target["establishing_run"], reused
        assert reused["report"] == target["report"], "reuse must preserve establishing report"

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

        mcp = subprocess.Popen([sail, "mcp"], cwd=project, env=env,
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
                    "name":"verification_result", "arguments":{"job_id":mcp_job}}})
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

        # Change a proof into a failure without changing size or mtime. This verifies
        # the content/receipt binding independently of Cargo's timestamp behaviour.
        stamp = source.stat()
        bad = good.replace("r >= b", "r >  b")
        assert len(bad) == len(good)
        source.write_text(bad)
        os.utime(source, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
        failed_run, failed, failure = managed()
        assert failed_run.returncode != 0 and not failure["reused"] and not failure["complete"], failed
        assert failure["report"]["summary"]["failed"] == 1, failure
        assert failed["execution"]["verifier_invocations"] >= 1, failed["execution"]
        plain = bounded_verification([cargo_verus, "verus", "verify", "--target-dir",
                                      str(contexts[0] / "target")], project, env, 120)
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

        print("managed acceptance: cold, unchanged, CLI/MCP jobs, missing receipt, restored-mtime failing edit and repeated focus passed")
    finally:
        stop_daemon()
