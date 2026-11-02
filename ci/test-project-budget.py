#!/usr/bin/env python3
"""Installed project-budget controls over two genuinely verified workspaces."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("sail", "runtime", "out"):
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args()
    out = Path(args.out).resolve()
    out.mkdir(parents=True)
    cache = out / "cache"
    env = dict(os.environ, SAIL_CACHE_DIR=str(cache), SAIL_VERUS_BUILD=args.runtime,
               SAIL_BACKGROUND_PREPARE="0")
    env.pop("SAIL_DAEMON_SOCKET", None)
    records = []

    def call(label, command, cwd=out, code=0):
        start = time.monotonic()
        result = subprocess.run([str(item) for item in command], cwd=cwd, env=env,
                                capture_output=True, timeout=180)
        (out / (label + ".stdout")).write_bytes(result.stdout)
        (out / (label + ".stderr")).write_bytes(result.stderr)
        records.append({"label": label, "exit_code": result.returncode,
                        "wall_seconds": time.monotonic() - start})
        (out / "records.json").write_text(json.dumps(records, indent=2) + "\n")
        assert result.returncode == code, (label, result.returncode, result.stderr)
        return result

    def projects(label):
        return json.loads(call(label, [args.sail, "cache", "projects", "list", "--json"]).stdout)["projects"]

    def budget(label, *options):
        return json.loads(call(label, [args.sail, "cache", "projects", "budget", *options, "--json"]).stdout)

    def stop_daemon(child):
        if child is not None:
            if child.poll() is None:
                child.terminate()
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
                raise

    def start_daemon(label):
        with (out / (label + "-daemon.stdout")).open("wb") as stdout, \
                (out / (label + "-daemon.stderr")).open("wb") as stderr:
            child = subprocess.Popen([args.sail, "serve"], cwd=out, env=env,
                                     stdout=stdout, stderr=stderr)
        try:
            deadline = time.monotonic() + 10
            while True:
                assert child.poll() is None, (label, child.returncode)
                status = json.loads(call(label + "-ready-" + str(len(records)),
                                         [args.sail, "status", "--json"]).stdout)
                if status["running"]:
                    return child
                assert time.monotonic() < deadline, label
                time.sleep(.05)
        except BaseException:
            stop_daemon(child)
            raise

    daemon = None
    try:
        daemon = start_daemon("manual")
        assert budget("default")["enabled"] is False
        for name in ("older", "newer"):
            call("new-" + name, [args.sail, "new", "--lib", name])
            document = json.loads(call("verify-" + name, [args.sail, "verify", "--output", "json"], out / name).stdout)
            assert document["currentness"] == "current" and document["exit_code"] == 0, document
            assert document["execution"]["verifier_invocations"] > 0, document
        initial = projects("initial")
        assert len(initial) == 2, initial
        keys = {Path(item["workspace"]).name: item["key"] for item in initial}
        dry = json.loads(call("dry-run", [args.sail, "cache", "projects", "prune", "--to-bytes", "1", "--dry-run", "--json"]).stdout)
        assert dry["protected"] == keys["newer"] and [item["key"] for item in dry["removed"]] == [keys["older"]], dry
        assert len(projects("after-dry-run")) == 2
        trimmed = json.loads(call("trim", [args.sail, "cache", "projects", "prune", "--to-bytes", "1", "--json"]).stdout)
        assert trimmed["protected"] == keys["newer"] and trimmed["over_budget"], trimmed
        assert [item["key"] for item in trimmed["removed"]] == [keys["older"]], trimmed
        assert [item["key"] for item in projects("after-trim")] == [keys["newer"]]
        enabled = budget("enable", "--max-bytes", "1")
        assert enabled["enabled"] and enabled["max_bytes"] == 1 and enabled["over_budget"], enabled
        assert budget("read-back") == enabled
        assert budget("disable", "--disable")["enabled"] is False
        call("conflicting-selector", [args.sail, "cache", "projects", "prune", "--to-bytes", "1", "--all"], code=2)
        rebuilt = json.loads(call("rebuild-evicted", [args.sail, "verify", "--output", "json"], out / "older").stdout)
        assert rebuilt["currentness"] == "current" and rebuilt["exit_code"] == 0, rebuilt
        assert rebuilt["execution"]["verifier_invocations"] > 0, rebuilt
        stop_daemon(daemon)
        daemon = None
        assert len(projects("before-automatic")) == 2
        assert budget("enable-automatic", "--max-bytes", "1")["enabled"]
        daemon = start_daemon("automatic")
        deadline = time.monotonic() + 15
        while True:
            remaining = projects("automatic-" + str(len(records)))
            if [item["key"] for item in remaining] == [keys["older"]]:
                break
            assert time.monotonic() < deadline, ("idle daemon did not enforce the managed cache budget", remaining)
            time.sleep(.1)
        policy = budget("automatic-policy")
        assert policy["enabled"] and policy["over_budget"], policy
        assert budget("disable-automatic", "--disable")["enabled"] is False
        print("PASS: installed budget defaults, dry-run, LRU/newest protection, actual idle-daemon eviction and genuine re-verification")
    finally:
        stop_daemon(daemon)



if __name__ == "__main__":
    main()
