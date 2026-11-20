#!/usr/bin/env python3
"""Compare installed native CFG rechecking with fresh checks and test its miss paths."""
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time


def run(sail, root, width=200000):
    cache = root / "cache"
    socket = root / "s.sock"
    env = dict(os.environ, SAIL_CACHE_DIR=str(cache))
    original = root / "previous.json"
    current = root / "current.json"
    source = f"""---- MODULE M ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == IF x = 0 THEN x' \\in 1..{width} ELSE IF x = {width} THEN x' = {width+1} ELSE x' = x
Inv == x /= {width+1}
Aux == x >= 0
====
"""
    config = "INIT Init\nNEXT Next\nINVARIANT Inv\nINVARIANT Aux\n"

    def job(path, module=source, cfg=config, max_states=210000):
        doc = {"version": 1, "input": {"kind": "temporal", "job": {
            "obligation_id": path.stem, "requirement_ids": [], "wall_time_ms": 30000,
            "request": {"version": 1, "model": {"root": "M.tla", "config": "M.cfg",
                "files": {"M.tla": module, "M.cfg": cfg}},
                "operation": {"kind": "check", "max_states": max_states, "max_depth": 100}}}}}
        path.write_text(json.dumps(doc))

    def call(args):
        start = time.monotonic()
        result = subprocess.run(args, env=env, capture_output=True, timeout=40)
        if result.returncode not in (0, 1) or not result.stdout:
            raise RuntimeError(result.stderr.decode())
        return json.loads(result.stdout), (time.monotonic() - start) * 1000

    def check(path, *options):
        report, ms = call([sail, "runtime", "execute", "--file", path, *options])
        return report["output"]["result"], ms

    def recheck():
        return call([sail, "runtime", "recheck", "--previous", original, "--file", current])

    with subprocess.Popen([sail, "serve", "--socket", socket], env=env,
                          stdout=subprocess.DEVNULL, stderr=subprocess.PIPE) as daemon:
        try:
            for _ in range(200):
                if daemon.poll() is not None:
                    raise RuntimeError(daemon.stderr.read().decode())
                if subprocess.run([sail, "runtime", "capabilities", "--socket", socket],
                                  env=env, capture_output=True, timeout=3).returncode == 0:
                    break
                time.sleep(0.05)
            job(original)
            # A missing basis falls through to the current check. No old result is created.
            job(current, cfg=config + "\\* cold basis\n")
            cold, _ = recheck()
            assert cold["route"] == "current-check"
            basis, _ = check(original, "--socket", socket)
            assert not basis["cache_hit"] and basis["response"]["outcome"] == "refuted"
            expected_itf = basis["response"]["itf_traces"]
            assert len(expected_itf[0]["states"]) == 3

            fast, full = [], []
            for sample in range(5):
                job(current, cfg=config + f"\\* new explanation {sample}\n")
                report, ms = recheck()
                assert report["route"] == "equivalent-cfg-rechecked-refutation"
                assert report["outcome"] == "refuted"
                assert report["obligation_id"] == current.stem
                assert report["request_sha256"] != report["previous_request_sha256"]
                assert len(report["previous_receipt_key"]) == 64
                assert report["validation"]["output"]["result"]["response"]["outcome"] == "valid"
                assert report["validation"]["output"]["result"]["response"]["itf_traces"] == expected_itf
                fast.append(round(ms, 2))
                fresh, ms = check(current)
                assert not fresh["cache_hit"] and fresh["response"]["outcome"] == "refuted"
                assert fresh["response"]["itf_traces"] == expected_itf
                full.append(round(ms, 2))

            # The last edited request now has its own exact receipt. Rechecking
            # with a different previous request must return the current evidence.
            exact, _ = recheck()
            assert exact["route"] == "current-check"
            result = exact["response"]["output"]["result"]
            assert result["cache_hit"] and result["response"] == fresh["response"]
            assert result["request_sha256"] == report["request_sha256"]

            for mutation, intended in [
                ({"cfg": config.replace("INVARIANT Aux\n", "")}, "refuted"),
                ({"module": source.replace(f"Inv == x /= {width+1}", f"Inv == IF x = 1 THEN 1 ELSE x /= {width+1}")}, "error"),
                ({"module": source.replace(f"Inv == x /= {width+1}", f"Inv == x <= {width+1}")}, "holds"),
                ({"max_states": 4}, "stopped"),
            ]:
                job(current, **mutation)
                result, _ = recheck()
                assert result["route"] == "current-check"
                assert result["response"]["output"]["result"]["response"]["outcome"] == intended
            witness = json.dumps(expected_itf, sort_keys=True, separators=(",", ":")).encode()
            print(json.dumps({"width": width, "fresh_ms": full, "guarded_recheck_ms": fast,
                              "median_speedup": round(statistics.median(full) / statistics.median(fast), 2),
                              "states": basis["response"]["native_report"]["statistics"]["distinct"],
                              "basis_request_sha256": basis["request_sha256"],
                              "witness_sha256": hashlib.sha256(witness).hexdigest(),
                              "sail_sha256": hashlib.sha256(sail.read_bytes()).hexdigest(),
                              "worker_sha256": basis["worker_sha256"]}))
        finally:
            daemon.terminate()
            daemon.wait(timeout=15)


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] != "--smoke"):
        raise SystemExit("usage: benchmark-guarded-refutation.py STAGED_SAIL [--smoke]")
    with tempfile.TemporaryDirectory(prefix="sail-wit-", dir="/tmp") as temp:
        run(Path(sys.argv[1]).resolve(), Path(temp), 20 if "--smoke" in sys.argv else 200000)
