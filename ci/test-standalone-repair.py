#!/usr/bin/env python3
"""Installed standalone repair against the actual pinned runtime, never a stub."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("sail", "stock", "runtime", "out"):
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args()
    out = Path(args.out).absolute()
    out.mkdir(parents=True)
    work = out / "sources"
    work.mkdir()
    source = work / "repair_case.rs"
    env = dict(os.environ, SAIL_CACHE_DIR=str(out / "cache"),
               SAIL_VERUS_BUILD=args.runtime, SAIL_BACKGROUND_PREPARE="0")
    records = []

    def call(label, command, code=0):
        result = subprocess.run(command, cwd=work, env=env, capture_output=True, timeout=180)
        (out / (label + ".stdout")).write_bytes(result.stdout)
        (out / (label + ".stderr")).write_bytes(result.stderr)
        records.append({"label": label, "command": command, "exit_code": result.returncode})
        assert result.returncode == code, (label, result.returncode, result.stderr.decode(errors="replace"))
        return result

    def repair(label, function, code=0):
        result = call(label, [args.sail, "repair-check", source.name, "--function", function,
                             "--engine", "interactive", "--output", "json"], code)
        return json.loads(result.stdout)

    passing = "use vstd::prelude::*;\nverus! { proof fn selected() { assert(true); } proof fn other() { assert(true); } }\nfn main() {}\n"
    source.write_text(passing)
    initial = json.loads(call("initial", [args.sail, "verify", source.name, "--output", "json"]).stdout)
    functions = [item["function"] for target in initial["targets"]
                 for crate in target["report"]["crates"] for item in crate["functions"]
                 if item["function"].endswith("::selected")]
    assert len(functions) == 1, initial
    function = functions[0]
    positive = repair("positive", function)
    assert positive["accepted"] and positive["batch"]["batched"], positive
    assert positive["batch"]["compiler_invocations"] == 1, positive
    full = positive["batch"]["full"]
    assert full["currentness"] == "current" and full["targets"][0]["complete"], full
    focus = positive["batch"]["focus"]
    assert not any(target["complete"] for target in focus["targets"]), focus
    assert all(item["verdict"] == "provisional" for target in focus["targets"]
               for crate in target["report"]["crates"] for item in crate["functions"]), focus

    source.write_text(passing.replace("proof fn other() { assert(true); }", "proof fn other() { assert(false); }"))
    call("stock-other-fails", [args.stock, source.name], 1)
    negative = repair("other-fails", function, 1)
    assert not negative["accepted"], negative
    assert negative["batch"]["full"]["exit_code"] == 1, negative
    assert negative["batch"]["focus"] is not None, negative
    source.write_text(passing)
    missing = repair("missing-selector", function + "_absent", 1)
    assert not missing["accepted"] and missing["batch"]["selection_error"], missing

    # A retained full-phase job is independently reread after source changes.
    submitted = json.loads(call("submit", [args.sail, "repair-check", source.name,
                                          "--function", function, "--engine", "interactive",
                                          "--background", "--output", "json"]).stdout)
    deadline = time.monotonic() + 180
    while True:
        retained = json.loads(call("job-result", [args.sail, "jobs", "result", submitted["id"], "--output", "json"]).stdout)["value"]
        if retained["job"]["state"] == "completed":
            break
        assert time.monotonic() < deadline, retained
        time.sleep(0.1)
    assert retained["repair"]["accepted"], retained
    source.write_text(passing.replace("assert(true)", "assert(false)", 1))
    stale = json.loads(call("historical", [args.sail, "jobs", "result", submitted["id"], "--output", "json"]).stdout)["value"]
    assert stale["job"]["currentness"] != "current", stale
    assert not stale["repair"]["accepted"], stale
    assert not (work / "Cargo.toml").exists() and not (work / "Cargo.lock").exists()
    (out / "records.json").write_text(json.dumps(records, indent=2) + "\n")
    print("PASS: actual standalone repair focus/full, failure, selector, job and stale-evidence controls")


if __name__ == "__main__":
    main()
