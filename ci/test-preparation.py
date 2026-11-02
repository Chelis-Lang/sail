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
    source.write_text(original)
    policy = json.loads(call("policy-original", [args.sail, "verify", "--output", "json"], project).stdout)
    disabled_env = dict(env, SAIL_BACKGROUND_PREPARE="0")
    policy_disabled = json.loads(call("policy-disabled", [args.sail, "verify", "--output", "json"],
                                      project, environment=disabled_env).stdout)
    assert policy_disabled["currentness"] == "current", policy_disabled
    assert policy_disabled["context_sha256"] == policy["context_sha256"], (policy, policy_disabled)
    assert policy_disabled["execution"]["verifier_invocations"] == 0, policy_disabled
    # The scheduling policy alone is not a compiler configuration change, but
    # an actual ghost-only read of it remains a declared proof input.
    source.write_text(original + '\nverus! { proof fn preparation_environment() {\n'
                      'let observed = option_env!("SAIL_BACKGROUND_PREPARE");\n'
                      'assert(match observed { None => true, Some(_) => false });\n} }\n')
    observed = json.loads(call("environment-original", [args.sail, "verify", "--output", "json"], project).stdout)
    assert observed["currentness"] == "current", observed
    rejected = json.loads(call("environment-changed", [args.sail, "verify", "--output", "json"],
                               project, 101, disabled_env).stdout)
    assert rejected["execution"]["verifier_invocations"] > 0, rejected
    source.write_text(original + '\npub mod sail_prepare_focus { use vstd::prelude::*;\n'
                      'verus! { pub proof fn selected() { assert(true); } } }\n')
    ready_before = status()["ready"]
    focused_args = [args.sail, "verify", "--output", "json", "--", "--verify-module", "sail_prepare_focus"]
    focused = json.loads(call("focused", focused_args, project).stdout)
    assert focused["currentness"] == "current" and focused["execution"]["verifier_invocations"] > 0, focused
    assert all(not target["complete"] for target in focused["targets"] if target["requested"]), focused
    deadline = time.monotonic() + 90
    while status()["ready"] <= ready_before:
        assert time.monotonic() < deadline, "exact focused scope was not prepared"
        time.sleep(0.1)
    focused_replay = json.loads(call("focused-prepared", focused_args, project).stdout)
    assert focused_replay["currentness"] == "current" and focused_replay["execution"]["verifier_invocations"] == 0, focused_replay
    assert all(not target["complete"] for target in focused_replay["targets"] if target["requested"]), focused_replay
    focused_runs = list((out / "cache/projects").glob("*/*/runs/" + focused_replay["run_id"]))
    assert len(focused_runs) == 1 and not (focused_runs[0] / "cargo.stdout").exists(), focused_replay
    whole = json.loads(call("whole-after-focus", [args.sail, "verify", "--output", "json"], project).stdout)
    assert whole["execution"]["verifier_invocations"] > 0, whole
    assert all(target["complete"] for target in whole["targets"] if target["requested"]), whole
    # A cold build first discovers compiler-consumed generated inputs. The
    # preparation guard must bind that complete closing revision, while proof
    # reports retain their original establishing revision and run.
    call("generated-new", [args.sail, "new", "--lib", "generated"],
         environment=dict(env, SAIL_BACKGROUND_PREPARE="0"))
    generated = out / "generated"
    script = generated / "build.rs"
    script.write_text('fn main() {\n'
                      'let output = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());\n'
                      'std::fs::write(output.join("generated.rs"), '
                      '"verus! { pub spec fn generated_value() -> int { 7 } }\\n").unwrap();\n'
                      'println!("cargo:rerun-if-changed=build.rs");\n}\n')
    lib = generated / "src/lib.rs"
    lib.write_text(lib.read_text() + '\ninclude!(concat!(env!("OUT_DIR"), "/generated.rs"));\n'
                   'verus! { proof fn generated_probe() { assert(generated_value() == 7); } }\n')
    cold = json.loads(call("generated-cold", [args.sail, "verify", "--output", "json"], generated).stdout)
    assert cold["currentness"] == "current" and cold["execution"]["verifier_invocations"] > 0, cold
    ready_before = status()["ready"]
    deadline = time.monotonic() + 90
    while True:
        progress = status()
        if progress["ready"] > ready_before and progress["queued"] == 0 and progress["running"] is None:
            break
        assert time.monotonic() < deadline, progress
        time.sleep(0.1)
    prepared = json.loads(call("generated-prepared", [args.sail, "verify", "--output", "json"], generated).stdout)
    assert prepared["currentness"] == "current" and prepared["execution"]["verifier_invocations"] == 0, prepared
    generated_runs = list((out / "cache/projects").glob("*/*/runs/" + prepared["run_id"]))
    assert len(generated_runs) == 1 and not (generated_runs[0] / "cargo.stdout").exists(), prepared
    assert [target["establishing_run"] for target in prepared["targets"]] == [target["establishing_run"] for target in cold["targets"]]
    request_path = out / "cache/results/projects/jobs/preparation/request.json"
    request = json.loads(request_path.read_bytes()[4:])["body"]
    assert request["verification"]["project_dir"] == str(generated)
    assert request["origin"]["revision"] != cold["input_revision"], "fixture did not discover generated inputs"
    script.write_text(script.read_text().replace("{ 7 }", "{ 8 }"))
    changed = json.loads(call("generated-input-changed", [args.sail, "verify", "--output", "json"], generated, 101).stdout)
    assert changed["execution"]["verifier_invocations"] > 0, changed
    (out / "records.json").write_text(json.dumps(records, indent=2) + "\n")
    print("PASS: actual preparation/replay, edit invalidation, policy/ghost-read changes, exact-focus/whole isolation and generated-input cold preparation")


if __name__ == "__main__":
    main()
