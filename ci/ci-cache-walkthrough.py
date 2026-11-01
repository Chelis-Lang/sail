#!/usr/bin/env python3
"""Hosted cache acceptance: retain all streams and refuse stale proof reuse."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("cold", "restored"))
    parser.add_argument("--mode", choices=("solver", "managed"), required=True)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--sail", type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    root.mkdir(parents=True, exist_ok=True)
    project = root / "project"
    evidence = root / (args.phase + "-" + args.mode)
    evidence.mkdir()
    env = dict(os.environ, SAIL_BACKGROUND_PREPARE="0", CARGO_TERM_COLOR="never")
    # Keep action credentials out of managed job environments and cache records.
    for name in ("GH_TOKEN", "GITHUB_TOKEN", "ACTIONS_RUNTIME_TOKEN"):
        env.pop(name, None)
    records = []

    def call(label, command, cwd=project, changes=None):
        start = time.monotonic()
        result = subprocess.run([str(item) for item in command], cwd=cwd,
                                env=dict(env, **(changes or {})), capture_output=True, timeout=600)
        (evidence / (label + ".stdout")).write_bytes(result.stdout)
        (evidence / (label + ".stderr")).write_bytes(result.stderr)
        records.append({"label": label, "exit_code": result.returncode,
                        "wall_seconds": time.monotonic() - start})
        (evidence / "records.json").write_text(json.dumps(records, indent=2) + "\n")
        return result

    def verify(label, changes=None, succeeds=True):
        result = call(label, [args.sail, "verify", "--output", "json"], changes=changes)
        document = json.loads(result.stdout)
        assert (result.returncode == 0) == succeeds, (label, result.returncode, document)
        assert document["currentness"] == "current", (label, document)
        assert (document["exit_code"] == 0) == succeeds, (label, document)
        return document

    if args.phase == "cold":
        assert not project.exists(), "cold producer must start without the fixture"
        result = call("new", [args.sail, "new", "--lib", "project"], cwd=root)
        assert result.returncode == 0, result.stderr
        source = project / "src/lib.rs"
        source.write_text(source.read_text() + "\nverus! { proof fn cache_walkthrough() { assert(true); } }\n")
        cold = verify("cold")
        assert cold["execution"]["verifier_invocations"] > 0, cold
        assert (project / "Cargo.lock").is_file()
    else:
        restored = verify("restored")
        if args.mode == "managed":
            assert restored["execution"]["verifier_invocations"] == 0, restored
        else:
            assert restored["execution"]["verifier_invocations"] > 0, restored
            status = call("restored-cache-status", [args.sail, "status", "--json"])
            assert status.returncode == 0, status.stderr
            assert json.loads(status.stdout)["stats"]["hits"] > 0, status.stdout
        source = project / "src/lib.rs"
        original = source.read_text()
        source.write_text(original.replace("proof fn cache_walkthrough() { assert(true); }",
                                           "proof fn cache_walkthrough() { assert(false); }"))
        edited = verify("edit-rejected", succeeds=False)
        assert edited["execution"]["verifier_invocations"] > 0, edited
        source.write_text(original)
        configured = verify("configuration", {"RUSTFLAGS": "--cfg sail_ci_cache_probe"})
        assert configured["execution"]["verifier_invocations"] > 0, configured
        # Tamper only with this job's installed engine; old receipts must never
        # turn modified verifier bytes into a successful Current proof.
        status = call("runtime-status", [args.sail, "engine", "status", "--output", "json"])
        assert status.returncode == 0, status.stderr
        runtime = Path(json.loads(status.stdout)["build"]) / "rust_verify"
        assert runtime.is_file() and not runtime.is_symlink(), runtime
        original_runtime = runtime.read_bytes()
        try:
            runtime.write_bytes(original_runtime + b"\nSAIL-CI-CACHE-INVALIDATION\n")
            refused = call("runtime-rejected", [args.sail, "verify", "--output", "json"])
            assert refused.returncode != 0, "changed runtime admitted a successful old proof"
        finally:
            runtime.write_bytes(original_runtime)
    print("PASS:", args.phase, args.mode)


if __name__ == "__main__":
    main()
