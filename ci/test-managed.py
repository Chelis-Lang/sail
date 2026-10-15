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

        def managed(*args):
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
            assert len(selected) == 1, selected
            return run, result, selected[0]

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
        assert reused["establishing_run"] == target["establishing_run"], reused
        assert reused["report"] == target["report"], "reuse must preserve establishing report"

        stored_vstd = vstd(warm)

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
        plain = bounded_verification([cargo_verus, "verus", "verify", "--target-dir",
                                      str(contexts[0] / "target")], project, env, 120)
        assert plain.returncode == failed_run.returncode, (plain.returncode, failed_run.returncode)
        assert b"postcondition not satisfied" in plain.stderr, plain.stderr.decode()
        assert b"postcondition not satisfied" in failed_run.stderr, failed_run.stderr.decode()

        # A process-loader variable (ADR 0046): the run is verified as plain
        # Cargo-Verus would verify it, with one note line, and reuses and
        # records no proof receipts. It is never refused.
        loader_env = dict(env, LD_LIBRARY_PATH=str(root / "no-libraries"))
        note = b"note[SAIL3112]: LD_LIBRARY_PATH is set"
        loader_json = bounded_verification([sail, "verify", "--output", "json"],
                                           project, loader_env, 240)
        assert note in loader_json.stderr, loader_json.stderr.decode()
        assert loader_json.returncode == failed_run.returncode, loader_json.stderr.decode()
        loader_doc = json.loads(loader_json.stdout)
        assert loader_doc["error"] is None and loader_doc["currentness"] == "unknown", diagnostic(loader_doc)
        assert any(n.startswith("SAIL3112:") for n in loader_doc["notes"]), loader_doc["notes"]
        assert not any(t["reused"] for t in loader_doc["targets"]), diagnostic(loader_doc)
        loader_contexts = list((root / "cache/projects").glob("*/" + loader_doc["context_sha256"]))
        assert len(loader_contexts) == 1 and loader_contexts[0] != contexts[0], loader_contexts
        assert not list((loader_contexts[0] / "receipts").glob("*.json")), "no receipts without reuse"
        # Its human output is plain Cargo-Verus's, apart from the note: the
        # same failing edit, each in its own target already built above.
        plain = bounded_verification([cargo_verus, "verus", "verify", "--target-dir",
                                      str(contexts[0] / "target")], project, env, 120)
        loader_human = bounded_verification([sail, "verify"], project, loader_env, 240)
        notes = [l for l in loader_human.stderr.splitlines(True) if l.startswith(b"note[SAIL3112]")]
        assert len(notes) == 1, loader_human.stderr.decode()
        assert loader_human.returncode == plain.returncode, loader_human.stderr.decode()
        assert loader_human.stdout == plain.stdout, (loader_human.stdout, plain.stdout)
        assert b"".join(l for l in loader_human.stderr.splitlines(True) if l not in notes) == plain.stderr, (
            loader_human.stderr.decode(), plain.stderr.decode())
        assert not list((loader_contexts[0] / "receipts").glob("*.json")), "no receipts without reuse"

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

        # Pruning the workspace's managed directory costs only a rebuild: the
        # next run reuses nothing, verifies again with the same report, and the
        # run after it reuses the new receipt.
        listed = json.loads(recorded_cli("projects-list", [sail, "cache", "projects", "list", "--json"]).stdout)
        mine = [p for p in listed["projects"] if p["workspace"] and Path(p["workspace"]).resolve() == project.resolve()]
        assert len(mine) == 1 and mine[0]["exists"] and mine[0]["bytes"] > 0, listed
        pruned = json.loads(recorded_cli("projects-prune", [sail, "cache", "projects", "prune", "--all", "--json"]).stdout)
        assert [p["key"] for p in pruned["removed"]] == [mine[0]["key"]] and not pruned["busy"], pruned
        assert not list((root / "cache/projects").glob("*/" + warm["context_sha256"])), "pruned context survived"
        rebuilt_run, rebuilt, rebuilt_target = managed()
        assert rebuilt_run.returncode == 0 and rebuilt["exit_code"] == 0, diagnostic(rebuilt)
        assert not rebuilt_target["reused"] and rebuilt_target["complete"], diagnostic(rebuilt)
        assert vstd(rebuilt)["dependency_store"], diagnostic(rebuilt)  # ADR 0045
        assert rebuilt["execution"]["verifier_invocations"] >= 1, rebuilt["execution"]
        assert rebuilt_target["report"]["summary"] == target["report"]["summary"], diagnostic(rebuilt)
        if rebuilt["currentness"] == "historical":
            rebuilt_run, rebuilt, rebuilt_target = managed()
            assert rebuilt_run.returncode == 0 and rebuilt["currentness"] == "current", diagnostic(rebuilt)
        reread_run, reread, reread_target = managed()
        assert reread_run.returncode == 0 and reread_target["reused"] and reread_target["complete"], diagnostic(reread)
        assert reread["execution"]["verifier_invocations"] == 0, reread["execution"]
        assert reread_target["establishing_run"] == rebuilt_target["establishing_run"], reread_target
        assert reread_target["report"] == rebuilt_target["report"], "reuse must preserve the rebuilt report"

        # ADR 0045: new projects (or worktrees) with the same vstd start from
        # the machine's dependency store, two at once. Neither verifies vstd;
        # each reports the vstd verified above, marked as placed from the store.
        store = root / "cache/dependency-store/v1"
        entries = [p for p in store.iterdir() if not p.name.startswith(".")]
        assert len(entries) == 1, entries
        copies = []
        for name in ("worktree-1", "worktree-2"):
            shutil.copytree(project, root / name)
            copies.append(root / name)
        seeding = [subprocess.Popen([sail, "verify", "--incremental", "--output", "json"],
                                    cwd=copy, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                   for copy in copies]
        for copy, process in zip(copies, seeding):
            out, err = process.communicate(timeout=240)
            (evidence / (copy.name + ".stdout.json")).write_bytes(out)
            (evidence / (copy.name + ".stderr")).write_bytes(err)
            seeded = json.loads(out)
            assert process.returncode == 0 and seeded["exit_code"] == 0, err.decode()
            assert b"SAIL3131" not in err, err.decode()
            seeded_vstd = vstd(seeded)
            assert seeded_vstd["reused"] and seeded_vstd["dependency_store"] == entries[0].name, diagnostic(seeded)
            assert seeded_vstd["establishing_run"] == stored_vstd["establishing_run"], diagnostic(seeded)
            assert seeded_vstd["report"] == stored_vstd["report"], "the store must preserve vstd's report"
            assert seeded["execution"]["verifier_invocations"] == 1, diagnostic(seeded)
            assert any(note.startswith("SAIL3130:") for note in seeded["notes"]), seeded["notes"]
            own = [t for t in seeded["targets"] if t["requested"]]
            assert len(own) == 1 and not own[0]["reused"] and own[0]["complete"], diagnostic(seeded)
            assert verdicts(own[0]) == verdicts(target), (verdicts(own[0]), verdicts(target))
        # The next run of a seeded project reuses its own receipts, which a
        # Current seeded run adopts: nothing is verified or placed again.
        again = bounded_verification([sail, "verify", "--incremental", "--output", "json"],
                                     copies[0], env, 240)
        repeated = json.loads(again.stdout)
        assert again.returncode == 0 and repeated["execution"]["verifier_invocations"] == 0, diagnostic(repeated)
        assert all(t["reused"] and "dependency_store" not in t for t in repeated["targets"]), diagnostic(repeated)
        assert vstd(repeated)["establishing_run"] == stored_vstd["establishing_run"], diagnostic(repeated)

        print("managed acceptance: failing first runs keep vstd, cold, unchanged, dependency store seeding, CLI/MCP jobs, missing receipt, restored-mtime failing edit, repeated focus and prune passed")
    finally:
        stop_daemon()
