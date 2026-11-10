#!/usr/bin/env python3
"""Hosted cache acceptance: retain all streams and refuse stale proof reuse.

The workflow (.github/workflows/ci-cache-walkthrough.yml) runs the GitHub
Action (action/action.yml) for the cold and restored verifications, so their
documents arrive through --result; this driver then checks them and runs the
invalidation probes itself.

  prepare   write the fixture (needs no Sail)
  cold      check the action's cold document
  restored  check the action's restored document, then refuse stale reuse
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

FIXTURE = Path(__file__).resolve().parent / "github-action-fixture"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("prepare", "cold", "restored"))
    parser.add_argument("--mode", choices=("solver", "managed"), required=True)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--sail", type=Path)
    parser.add_argument("--result", type=Path, help="the action's result document")
    args = parser.parse_args()
    root = args.root.resolve()
    root.mkdir(parents=True, exist_ok=True)
    project = root / "project"
    if args.phase == "prepare":
        assert not project.exists(), "cold producer must start without the fixture"
        shutil.copytree(FIXTURE, project)
        source = project / "src/lib.rs"
        source.write_text(source.read_text() + "\nverus! { proof fn cache_walkthrough() { assert(true); } }\n")
        print("PASS: prepare", args.mode)
        return
    assert args.sail and args.result, "cold and restored need --sail and --result"
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

    def action_result(label):
        """The action's document, retained with this phase's evidence."""
        shutil.copyfile(args.result, evidence / (label + ".stdout"))
        document = json.loads(args.result.read_bytes())
        records.append({"label": label, "exit_code": document["exit_code"],
                        "wall_seconds": document["execution"]["wall_ms"] / 1000, "via": "action"})
        (evidence / "records.json").write_text(json.dumps(records, indent=2) + "\n")
        assert document["exit_code"] == 0, (label, document)
        assert document["currentness"] == "current", (label, document)
        return document

    def walkthrough_provenance(document):
        functions = [function for target in document["targets"] if target["requested"]
                     for crate in target["report"]["crates"] for function in crate["functions"]
                     if function["function"].endswith("::cache_walkthrough")]
        assert len(functions) == 1 and functions[0]["verdict"] == "verified", document
        return functions[0]["provenance"]

    if args.phase == "cold":
        cold = action_result("cold")
        assert cold["execution"]["verifier_invocations"] > 0, cold
        assert walkthrough_provenance(cold)["checks_solved"] > 0, cold
        assert (project / "Cargo.lock").is_file()
    else:
        restored = action_result("restored")
        if args.mode == "managed":
            assert restored["execution"]["verifier_invocations"] == 0, restored
        else:
            assert restored["execution"]["verifier_invocations"] > 0, restored
            # The fixture's own query must be served, rather than counting a
            # shipped dependency hit as proof that restoration helped it.
            assert walkthrough_provenance(restored)["checks_served"] > 0, restored
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
        installed = json.loads(status.stdout)
        runtime = Path(installed["verus"]).with_name("rust_verify")
        assert Path(installed["build"]).resolve() in runtime.resolve().parents, installed
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
