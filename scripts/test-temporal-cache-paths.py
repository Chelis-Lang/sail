#!/usr/bin/env python3
"""Check receipt sharing across the installed native CLI and its default daemon."""
from contextlib import contextmanager
from copy import deepcopy
import json
import os
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def check_projection(sail, base, doc):
    """Prove a subset across CLI processes and verify the mandatory fallback cases."""
    previous = deepcopy(doc)
    old_job = previous["input"]["job"]
    model = old_job["request"]["model"]
    model["files"]["Warm.tla"] = """---- MODULE Warm ----
VARIABLE x
Init == x = <<0, 1>>
Next == x' = x
Inv == x[1] = 0
Second == x[2] = 1
FalseInv == x[1] = 1
====
"""
    model["files"]["Unused.tla"] = "---- MODULE Unused ----\nV == 1\n====\n"
    model["files"]["Warm.cfg"] = "INIT Init\nNEXT Next\nINVARIANT Inv Second\n"
    old_file = base / "previous.json"
    old_file.write_text(json.dumps(previous))
    store = base / "projection-store"

    def invoke(action, current, *args):
        result = subprocess.run(
            [sail, "runtime", action, "--file", "-", "--cache", store, *args],
            input=json.dumps(current), text=True, capture_output=True, check=True, timeout=20,
        )
        return json.loads(result.stdout)

    current = deepcopy(previous)
    job = current["input"]["job"]
    job["obligation_id"] = "edited-inventory"
    job["request"]["model"]["files"]["Warm.cfg"] = "INIT Init\nNEXT Next\nINVARIANT Inv\n"

    def recheck(envelope):
        return invoke("recheck", envelope, "--previous", old_file)

    basis = invoke("execute", previous)["output"]["result"]
    assert basis["response"]["outcome"] == "holds" and not basis["cache_hit"]
    retained = invoke("execute", previous)["output"]["result"]
    assert retained["cache_hit"]
    projected = recheck(current)
    assert projected["route"] == "established-invariant-subset" and projected["outcome"] == "holds"
    assert projected["obligation_id"] == "edited-inventory" and projected["invariants"] == ["Inv"]
    assert projected["previous_receipt_key"] == retained["cache_key"] and projected["worker_sha256"] == basis["worker_sha256"]

    for change, expected in [("unused-source", "holds"), ("limits", "holds"), ("added-obligation", "refuted")]:
        other = deepcopy(current)
        req = other["input"]["job"]["request"]
        if change == "unused-source":
            req["model"]["files"]["Unused.tla"] += "\n"
        elif change == "limits":
            req["operation"]["max_states"] += 1
        else:
            req["model"]["files"]["Warm.cfg"] += "INVARIANT FalseInv\n"
        reply = recheck(other)
        assert reply["route"] == "current-check", change
        assert reply["response"]["output"]["result"]["response"]["outcome"] == expected, change

    exact = invoke("execute", current)["output"]["result"]
    assert not exact["cache_hit"] and exact["request_sha256"] == projected["request_sha256"]
    reply = recheck(current)
    assert reply["route"] == "current-check" and reply["response"]["output"]["result"]["cache_hit"]



def check_first_temporal_claim(sail, base, template):
    """A cross-process result can cite only an established ordinary native receipt."""
    original = deepcopy(template)
    request = original["input"]["job"]["request"]
    request["operation"].update(max_states=64, max_depth=30)
    files = request["model"]["files"]
    files["Warm.tla"] = r"""---- MODULE Warm ----
CONSTANTS A,B
Values == [tag: {"a"}, v:A] \cup [tag: {"b"}, v:B]
VARIABLE x
Init == x \in Values
Next == x'=x
P == (x \in Values) ~> (x \in Values)
Q == []<>(x \in Values)
Bad == []<>(x.tag = "z")
====
"""
    files["Warm.cfg"] = "INIT Init\nNEXT Next\nCONSTANTS A={0,1} B={1}\nPROPERTY P Q\n"
    current = deepcopy(original)
    job = current["input"]["job"]
    job["obligation_id"] = "check-first-claim"
    job["requirement_ids"] = ["keep-p"]
    job["request"]["model"]["files"]["Warm.cfg"] = files["Warm.cfg"].replace("P Q", "P")
    old_file = base / "original-temporal.json"
    old_file.write_text(json.dumps(original))
    store = base / "first-claim-store"

    def invoke(action, value, *args):
        run = subprocess.run([sail, "runtime", action, "--cache", store, "--file", "-", *args],
                             input=json.dumps(value), text=True, capture_output=True, timeout=20,
                             check=True)
        return json.loads(run.stdout)

    def recheck(value, previous=old_file):
        return invoke("recheck", value, "--previous", previous)

    previous = invoke("execute", original)["output"]["result"]
    retained = invoke("execute", original)["output"]["result"]
    assert not previous["cache_hit"] and retained["cache_hit"]
    # The CLI parent can have an ambient Dive policy that the isolated worker clears.
    parent_env = dict(os.environ, DIVE_CORE_OPT="none", DIVE_NATIVE_ATOM_AUDIT="1")
    batch = {"version": 1, "wall_time_ms": 10000, "checks": [request, job["request"]]}
    session = subprocess.run([sail, "runtime", "edit-session", "--file", "-"],
                             input=json.dumps(batch), capture_output=True, text=True, check=True,
                             timeout=20, env=parent_env)
    entries = json.loads(session.stdout)["entries"]
    assert entries[0]["evidence"]["route"] == "fresh"
    assert entries[1]["evidence"]["route"] == "established-first-temporal-claim"
    first = recheck(current)
    assert first["route"] == "established-first-temporal-claim" and first["property"] == "P", first
    assert first["outcome"] == "holds" and first["obligation_id"] == "check-first-claim"
    assert first["requirement_ids"] == ["keep-p"]
    assert first["basis_counts"]["distinct"] == 3 and first["previous_receipt_key"] == retained["cache_key"]
    assert first["worker_sha256"] == previous["worker_sha256"]
    assert first["previous_request_sha256"] == previous["request_sha256"]
    # A successful projection is not a new native receipt and cannot establish the next edit.
    current_file = base / "projected-temporal.json"
    current_file.write_text(json.dumps(current))
    third = deepcopy(current)
    third["input"]["job"]["request"]["model"]["files"]["Warm.cfg"] += "\n"
    ordinary = recheck(third, current_file)
    assert ordinary["route"] == "current-check" and not ordinary["response"]["output"]["result"]["cache_hit"]
    for change, expected in [("first-property", "holds"), ("added", "refuted"), ("reorder", "holds"),
                             ("scope", "holds"), ("binding", "holds"), ("limits", "holds")]:
        edited = deepcopy(current)
        req = edited["input"]["job"]["request"]
        model = req["model"]["files"]
        if change == "scope":
            model["Unused.tla"] = "---- MODULE Unused ----\n===="
        elif change == "binding":
            model["Warm.cfg"] = model["Warm.cfg"].replace("A={0,1}", "A={0}")
        elif change == "limits":
            req["operation"]["max_depth"] += 1
        else:
            name = {"first-property": "Q", "added": "P Bad", "reorder": "Q P"}[change]
            model["Warm.cfg"] = model["Warm.cfg"].replace("PROPERTY P", f"PROPERTY {name}")
        ordinary = recheck(edited)
        assert ordinary["route"] == "current-check", change
        evidence = ordinary["response"]["output"]["result"]["response"]
        assert evidence["outcome"] == expected, (change, evidence)
        if expected == "refuted":
            assert evidence["validations"][0]["result"] == "valid"
            # A genuine previous liveness refutation cannot certify a surviving true sibling.
            refuted_file = base / "refuted-temporal.json"
            refuted_file.write_text(json.dumps(edited))
            surviving = deepcopy(current)
            surviving["input"]["job"]["request"]["model"]["files"]["Warm.cfg"] += "\n\n"
            checked = recheck(surviving, refuted_file)
            assert checked["route"] == "current-check"
            assert checked["response"]["output"]["result"]["response"]["outcome"] == "holds"
    exact = invoke("execute", current)["output"]["result"]
    assert not exact["cache_hit"] and first["request_sha256"] == exact["request_sha256"]
    replay = recheck(current)
    assert replay["route"] == "current-check" and replay["response"]["output"]["result"]["cache_hit"]

def run(sail, base):
    cache = base / "cache"
    assert not cache.exists()
    env = dict(os.environ, SAIL_CACHE_DIR=str(cache))
    fixture = Path(__file__).resolve().parents[1] / "crates/sail-cli/src/temporal_warmup.json"
    doc = json.loads(fixture.read_bytes())
    doc["input"]["job"]["wall_time_ms"] = 10_000
    request = base / "job.json"

    def check(action, *args):
        output = subprocess.run(
            [sail, "runtime", action, "--file", request, *args],
            env=env, capture_output=True, check=True, timeout=20,
        )
        result = json.loads(output.stdout)["output"]["result"]
        assert result["outcome"] == "completed" and result["response"]["outcome"] == "holds", result
        return result

    @contextmanager
    def running(args, endpoint=None):
        with subprocess.Popen(
            [sail, "serve", *args], env=env, cwd=base, umask=0o002,
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
        ) as daemon:
            try:
                deadline = time.monotonic() + 15
                while time.monotonic() < deadline:
                    if daemon.poll() is not None:
                        raise RuntimeError(daemon.stderr.read().decode())
                    sockets = [endpoint] if endpoint else list(cache.glob("daemon-v*-workspace-v2-proof-v1.sock"))
                    if sockets:
                        [socket] = sockets
                        ready = subprocess.run(
                            [sail, "runtime", "capabilities", "--socket", socket],
                            env=env, capture_output=True, timeout=5,
                        )
                        if ready.returncode == 0:
                            assert json.loads(ready.stdout)["temporal"]["installed"], ready.stdout
                            yield socket
                            return
                    time.sleep(0.05)
                raise RuntimeError("native runtime endpoint did not start")
            finally:
                daemon.terminate()
                try:
                    daemon.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    daemon.kill()
                    daemon.wait()

    with running([]) as socket:
        assert cache.stat().st_mode & 0o777 == 0o700
        request.write_text(json.dumps(doc))
        assert not check("execute", "--socket", socket)["cache_hit"]
        embedded = check("execute")
        assert embedded["cache_hit"]
        key = embedded["cache_key"]
        assert (cache / "runtime/temporal-v1/v2" / key[:2] / key).is_file()

        # A new byte identity, prepared through the embedded installer path.
        doc["input"]["job"]["request"]["model"]["files"]["Warm.cfg"] += "\n"
        request.write_text(json.dumps(doc))
        assert not check("prepare-temporal")["cache_hit"]
        served = check("execute", "--socket", socket)
        assert served["cache_hit"] and served["cache_key"] != key
        assert not (cache / "results/runtime/temporal-v1").exists()

        custom = base / "explicit"
        assert not check("execute", "--cache", custom)["cache_hit"]
        assert check("execute", "--cache", custom)["cache_hit"]
        assert (custom / "temporal-v1").is_dir()

    # The regular solver cache is eligible for unsigned CI restore. Alternate
    # spellings of it select the same trusted temporal store on both interfaces.
    (base / "replies").symlink_to(cache / "results", target_is_directory=True)
    custom_socket = base / "alias.sock"
    with running(["--socket", custom_socket, "--cache", "./replies"], custom_socket):
        assert check("execute", "--socket", custom_socket)["cache_hit"]
    assert check("execute", "--cache", base / "replies")["cache_hit"]
    alternate = base / "replies" / "nested"
    assert not check("execute", "--cache", alternate)["cache_hit"]
    assert check("execute", "--cache", alternate)["cache_hit"]
    assert not list((cache / "results").rglob("temporal-v1"))
    check_projection(sail, base, doc)
    check_first_temporal_claim(sail, base, doc)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="sail-rt-", dir="/tmp") as directory:
        base = Path(directory)
        if len(sys.argv) == 2:
            sail = Path(sys.argv[1]).resolve()
        elif len(sys.argv) == 4 and sys.argv[1] == "--stage":
            install = base / "bin"
            install.mkdir()
            sail = install / "sail"
            shutil.copy2(sys.argv[2], sail)
            stage = Path(__file__).with_name("stage-temporal-worker.py")
            subprocess.run([sys.executable, stage, sys.argv[3], install], check=True)
        else:
            raise SystemExit("usage: test-temporal-cache-paths.py STAGED_SAIL | --stage SAIL WORKER")
        run(sail, base)
        subprocess.run([sys.executable, Path(__file__).with_name("benchmark-guarded-refutation.py"),
                        sail, "--smoke"], check=True)
        subprocess.run([sys.executable, Path(__file__).with_name("benchmark-temporal-edit-session.py"),
                        sail, "--width", "20", "--samples", "1"], check=True, stdout=subprocess.DEVNULL)
    print("native cache sharing, guarded rechecks and edit sessions passed")
