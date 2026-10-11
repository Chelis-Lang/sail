#!/usr/bin/env python3
"""Run the frozen metadata controls in a lead-granted private source/target.

This controller does not install sources, acquire a compiler slot or certify
whole-process closure. The lead must supply an exclusive private tree and retain
its independent actor/holder audit. No verifier proof or acceptance run occurs.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import time

CASE = "managed::freshness::tests::unchanged_metadata_publication_keeps_same_continuous_freshness_owner"
ASSERTION = "Arc::ptr_eq(&owner, &fixture.runtime_owner(&index))"


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for data in iter(lambda: stream.read(65536), b""):
            digest.update(data)
    return digest.hexdigest()


def expected_negative(code, text):
    return (code == 101 and CASE + " ... FAILED" in text
            and ASSERTION in text
            and re.search(r"test result: FAILED\. 0 passed; 1 failed;", text) is not None)


def images(log, target):
    selected = set()
    for line in log.read_text(errors="replace").splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if (row.get("reason") == "compiler-artifact"
                and row.get("target", {}).get("name") == "sail_verus"
                and row.get("profile", {}).get("test") is True
                and row.get("executable")):
            path = Path(row["executable"]).resolve(strict=True)
            if not path.is_relative_to(target):
                raise ValueError("tested image outside private target")
            selected.add(path)
    return sorted(selected)


def run(arguments):
    source = arguments.source.resolve(strict=True)
    target = arguments.target.resolve(strict=True)
    bundle = Path(__file__).resolve().parent
    record = json.loads((bundle / "m7-preserve-freshness-metadata-source.json").read_text())
    before = {name: sha(source / name) for name in record["sources"]}
    if before != record["sources"]:
        raise ValueError("frozen source identity mismatch before target mutation")
    lock_sha = sha(source / "Cargo.lock")
    patch = bundle / "m7-preserve-freshness-metadata-counterfactual.patch"
    subprocess.run(["git", "apply", "--check", str(patch)], cwd=source, check=True)
    command = ["cargo", "test", "-p", "sail-verus", "--release", "--lib", "--message-format=json-render-diagnostics"]
    phases = [
        ("metadata", command + ["metadata", "--", "--test-threads=1"]),
        ("freshness", command + ["managed::freshness::tests::", "--", "--test-threads=1"]),
        ("strict", ["cargo", "clippy", "-p", "sail-verus", "--release", "--all-targets", "--", "-D", "warnings"]),
    ]
    if not arguments.execute:
        print(json.dumps({"status": "prepare-only", "source_equal": True,
                          "commands": [argv for _, argv in phases],
                          "sensitivity_case": CASE, "compiler_actor_started": False}))
        return
    output = arguments.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    retained = output / "tested-binaries"
    retained.mkdir()
    environment = dict(os.environ)
    for name in list(environment):
        if name.startswith("SAIL_ENGINE"):
            del environment[name]
    environment["CARGO_TARGET_DIR"] = str(target)
    environment["CARGO_BUILD_JOBS"] = "1"
    results = {"schema": 1, "status": "RUNNING", "source": record["sources"],
               "lock_sha256": lock_sha, "phases": [], "actor_closure_certified": False,
               "verification_or_acceptance_credit": False}

    def save():
        (output / "result.json").write_text(json.dumps(results, indent=2) + "\n")

    def phase(name, argv, negative=False):
        path = output / (name + ".log")
        started = time.monotonic()
        with path.open("xb") as log:
            process = subprocess.run(argv, cwd=source, env=environment, stdout=log, stderr=subprocess.STDOUT)
        row = {"name": name, "argv": argv, "exit_code": process.returncode,
               "elapsed_seconds": time.monotonic() - started, "log_sha256": sha(path)}
        results["phases"].append(row)
        save()
        if name != "strict":
            row["tested_images"] = []
            selected = images(path, target)
            for executable in selected:
                copy = retained / (name + "-" + executable.name)
                shutil.copy2(executable, copy)
                if sha(copy) != sha(executable):
                    raise ValueError("tested executable retention mismatch")
                row["tested_images"].append({"name": copy.name, "sha256": sha(copy), "bytes": copy.stat().st_size})
                dependency = executable.with_suffix(".d")
                if dependency.is_file():
                    shutil.copy2(dependency, retained / (name + "-" + dependency.name))
        text = path.read_text(errors="replace")
        row["pass"] = expected_negative(process.returncode, text) if negative else process.returncode == 0
        if name != "strict" and row["pass"]:
            row["pass"] = bool(selected) and "running 0 tests" not in text
        save()
        if not row["pass"]:
            raise ValueError("unexpected phase outcome: " + name)

    original = (source / "crates/sail-verus/src/managed/files.rs").read_bytes()
    try:
        for name, argv in phases:
            phase(name, argv)
        try:
            subprocess.run(["git", "apply", str(patch)], cwd=source, check=True)
            phase("removed-noop", command + [CASE, "--", "--exact", "--test-threads=1"], negative=True)
        finally:
            (source / "crates/sail-verus/src/managed/files.rs").write_bytes(original)
        if {name: sha(source / name) for name in before} != before:
            raise ValueError("positive source restoration mismatch")
        phase("restored-positive", command + [CASE, "--", "--exact", "--test-threads=1"])
        results["status"] = "PASS"
    except BaseException as error:
        results["status"] = "FAIL"
        results["error_kind"] = type(error).__name__
        raise
    finally:
        results["source_equal"] = {name: sha(source / name) for name in before} == before
        results["lock_equal"] = sha(source / "Cargo.lock") == lock_sha
        save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--target", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--execute", action="store_true")
    run(parser.parse_args())


if __name__ == "__main__":
    main()
