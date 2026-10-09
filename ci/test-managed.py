#!/usr/bin/env python3
"""Bounded real-tool acceptance for managed proof receipts; never a benchmark."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

sail = str(Path(sys.argv[1]).resolve())
cargo_verus = os.environ.get("SAIL_CARGO_VERUS") or shutil.which("cargo-verus")
if not cargo_verus:
    raise SystemExit("managed acceptance requires the pinned cargo-verus")

with tempfile.TemporaryDirectory(prefix="sail-managed-acceptance-") as temporary:
    root = Path(temporary)
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

        def managed(*args):
            run = subprocess.run([sail, "verify", "--incremental", "--output", "json", *args],
                                 cwd=project, env=env, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, timeout=240)
            result = json.loads(run.stdout)
            assert result["kind"] == "sail-project-result", result
            assert result["error"] is None, result
            selected = [target for target in result["targets"] if target["requested"]]
            assert len(selected) == 1, selected
            return run, result, selected[0]

        cold_run, cold, target = managed()
        assert cold_run.returncode == 0 and cold["exit_code"] == 0, cold_run.stderr.decode()
        assert target["complete"] and not target["reused"], target
        assert cold["currentness"] == "current", cold
        assert target["report"]["summary"]["verified"] == 1, target

        warm_run, warm, reused = managed()
        assert warm_run.returncode == 0 and reused["reused"] and reused["complete"], warm
        assert warm["execution"]["verifier_invocations"] == 0, warm["execution"]
        assert reused["establishing_run"] == target["establishing_run"], reused
        assert reused["report"] == target["report"], "reuse must preserve establishing report"

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
        plain = subprocess.run([cargo_verus, "verus", "verify", "--target-dir",
                                str(contexts[0] / "target")], cwd=project, env=env,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
        assert plain.returncode == failed_run.returncode, (plain.returncode, failed_run.returncode)
        assert b"postcondition not satisfied" in plain.stderr, plain.stderr.decode()
        assert b"postcondition not satisfied" in failed_run.stderr, failed_run.stderr.decode()

        print("managed acceptance: cold, unchanged, missing receipt, and restored-mtime failing edit passed")
    finally:
        stop_daemon()
