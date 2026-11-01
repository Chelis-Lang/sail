#!/usr/bin/env python3
"""Actual daemon preparation and next-call replay; native acceptance only."""
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
    out = Path(args.out).absolute()
    out.mkdir(parents=True)
    env = dict(os.environ, SAIL_CACHE_DIR=str(out / "cache"), SAIL_VERUS_BUILD=args.runtime)
    env.pop("SAIL_BACKGROUND_PREPARE", None)
    records = []

    def call(label, command, cwd=out, code=0, environment=None):
        start = time.monotonic()
        result = subprocess.run(command, cwd=cwd, env=environment or env,
                                capture_output=True, timeout=180)
        (out / (label + ".stdout")).write_bytes(result.stdout)
        (out / (label + ".stderr")).write_bytes(result.stderr)
        records.append({"label": label, "command": command, "exit_code": result.returncode,
                        "wall_seconds": time.monotonic() - start})
        assert result.returncode == code, (label, result.returncode, result.stderr.decode(errors="replace"))
        return result

    def status():
        return json.loads(call("status", [args.sail, "status", "--json"]).stdout)["preparation"]

    call("new", [args.sail, "new", "--lib", "demo"])
    project = out / "demo"
    # This is the first explicit user call, with no waiting for preparation.
    first = json.loads(call("immediate", [args.sail, "verify", "--output", "json"], project).stdout)
    assert first["exit_code"] == 0, first
    source = project / "src/lib.rs"
    original = source.read_text()
    source.write_text(original + "\nverus! { proof fn sail_preparation_probe() { assert(true); } }\n")
    changed = json.loads(call("changed", [args.sail, "verify", "--output", "json"], project).stdout)
    assert changed["currentness"] == "current" and changed["execution"]["verifier_invocations"] > 0, changed
    deadline = time.monotonic() + 90
    while True:
        progress = status()
        if progress["ready"] > 0:
            break
        assert time.monotonic() < deadline, progress
        time.sleep(0.1)
    replay = json.loads(call("prepared", [args.sail, "verify", "--output", "json"], project).stdout)
    assert replay["currentness"] == "current" and replay["execution"]["verifier_invocations"] == 0, replay
    # Distinguish a genuine unchanged-call replay from another zero-verifier Cargo run.
    runs = list((out / "cache/projects").glob("*/*/runs/" + replay["run_id"]))
    assert len(runs) == 1 and (runs[0] / "result.json").is_file(), replay
    assert not (runs[0] / "cargo.stdout").exists(), "prepared call still ran Cargo"
    assert [target["establishing_run"] for target in replay["targets"]] == [target["establishing_run"] for target in changed["targets"]]
    source.write_text(source.read_text().replace("proof fn sail_preparation_probe() { assert(true); }", "proof fn sail_preparation_probe() { assert(false); }"))
    failed = json.loads(call("failure", [args.sail, "verify", "--output", "json"], project, 101).stdout)
    assert failed["execution"]["verifier_invocations"] > 0, failed
    (out / "records.json").write_text(json.dumps(records, indent=2) + "\n")
    print("PASS: actual new-project preparation, immediate foreground call, prepared replay and edit invalidation")


if __name__ == "__main__":
    main()
