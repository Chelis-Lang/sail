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
import signal
import subprocess
import time

CASE = "managed::freshness::tests::unchanged_metadata_publication_keeps_same_continuous_freshness_owner"
ASSERTION = "Arc::ptr_eq(&owner, &fixture.runtime_owner(&index))"
COMPILE_SECONDS = 600
TEST_SECONDS = 180


class OwnedCommands:
    """Forced cleanup cannot prove compiler descendants retired."""

    def __init__(self):
        self.active = None
        self.spawning = False
        self.reasons = []
        self.starts = []

    def uncertain(self, reason):
        if reason not in self.reasons:
            self.reasons.append(reason)

    def require_closed(self):
        if self.reasons or self.active is not None or self.spawning:
            raise RuntimeError("owned descendant closure uncertain; external holder audit required")

    def wait(self, child, limit, kill_group=os.killpg):
        try:
            return child.wait(timeout=limit)
        except BaseException:
            self.uncertain("wait interrupted or timed out")
            raise
        finally:
            self.cleanup(child, kill_group)

    def cleanup(self, child, kill_group=os.killpg):
        if child.poll() is None:
            self.uncertain("process-group termination required")
            kill_group(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                kill_group(child.pid, signal.SIGKILL)
                child.wait(timeout=2)

    def execute(self, argv, source, environment, log, limit):
        self.require_closed()
        self.spawning = True
        try:
            self.active = subprocess.Popen(argv, cwd=source, env=environment,
                                           stdout=log, stderr=subprocess.STDOUT,
                                           start_new_session=True)
            self.spawning = False
            self.starts.append({"pid": self.active.pid, "pgid": self.active.pid,
                                "launched_unix_ns": time.time_ns()})
            return self.wait(self.active, limit)
        except BaseException:
            # Spawn interruption can lose the new child's handle. Never restore
            # sources or start another command from this uncertain state.
            self.uncertain("command interrupted or spawn failed")
            raise
        finally:
            if self.active is not None:
                # Also cover interruption after Popen returned but before wait.
                # Failed forced cleanup remains for the lead, without retries.
                if "process-group termination required" not in self.reasons:
                    self.cleanup(self.active)
                if self.active.poll() is not None:
                    self.active = None

    def restore(self, path, original):
        self.require_closed()
        path.write_bytes(original)


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for data in iter(lambda: stream.read(65536), b""):
            digest.update(data)
    return digest.hexdigest()


def expected_negative(code, text):
    # --nocapture interleaves the panic between the case prefix and FAILED.
    # Require the same exact single-case failure and unchanged assertion.
    case = re.escape(CASE)
    failed_case = re.search(
        r"(?ms)^test " + case + r" \.\.\. (?:FAILED$|\nthread '" + case
        + r"' [^\n]* panicked at [^\n]*\n.*?^FAILED$)", text)
    return (code == 101 and failed_case is not None
            and re.search(r"(?m)^running 1 test$", text) is not None
            and re.search(r"(?m)^assertion failed: " + re.escape(ASSERTION) + r"$", text) is not None
            and re.search(r"(?m)^    " + case + r"$", text) is not None
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
    command = ["cargo", "test", "--locked", "-p", "sail-verus", "--release", "--lib",
               "--no-run", "--message-format=json-render-diagnostics"]
    strict = ["cargo", "clippy", "--locked", "-p", "sail-verus", "--release",
              "--all-targets", "--", "-D", "warnings"]
    if not arguments.execute:
        print(json.dumps({"status": "prepare-only", "source_equal": True,
                          "compile_command": command, "strict_command": strict,
                          "test_filters": ["metadata", "managed::freshness::tests::"],
                          "compile_limit_seconds": COMPILE_SECONDS,
                          "test_limit_seconds": TEST_SECONDS,
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
    environment["CARGO_INCREMENTAL"] = "0"
    ownership = OwnedCommands()

    def interrupt(number, _frame):
        ownership.uncertain("controller signal; command or patch closure requires audit")
        for value in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(value, signal.SIG_IGN)
        raise InterruptedError("controller signal " + str(number))

    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, interrupt)
    results = {"schema": 1, "status": "RUNNING", "source": record["sources"],
               "lock_sha256": lock_sha, "phases": [], "actor_closure_certified": False,
               "verification_or_acceptance_credit": False}

    def save():
        results["owned_group_starts"] = list(ownership.starts)
        results["active_leader_pid"] = ownership.active.pid if ownership.active else None
        results["spawn_closure_uncertain"] = ownership.spawning
        (output / "result.json").write_text(json.dumps(results, indent=2) + "\n")

    def phase(name, argv, limit, negative=False, compile_image=False, test=False):
        path = output / (name + ".log")
        started = time.monotonic()
        row = {"name": name, "argv": argv, "limit_seconds": limit,
               "exit_code": None}
        results["phases"].append(row)
        try:
            with path.open("xb") as log:
                row["exit_code"] = ownership.execute(argv, source, environment, log, limit)
        finally:
            row["elapsed_seconds"] = time.monotonic() - started
            row["log_sha256"] = sha(path) if path.is_file() else None
            save()
        save()
        selected = []
        if compile_image:
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
        row["pass"] = expected_negative(row["exit_code"], text) if negative else row["exit_code"] == 0
        if compile_image and row["pass"]:
            row["pass"] = len(selected) == 1
        if test and row["pass"]:
            row["pass"] = re.search(r"running [1-9][0-9]* tests?", text) is not None
        save()
        if not row["pass"]:
            raise ValueError("unexpected phase outcome: " + name)
        return retained / (name + "-" + selected[0].name) if compile_image else None

    def case(name, image, selected, negative=False, exact=False):
        argv = [str(image), selected]
        if exact:
            argv.append("--exact")
        argv += ["--nocapture", "--test-threads=1"]
        phase(name, argv, TEST_SECONDS, negative=negative, test=True)

    original = (source / "crates/sail-verus/src/managed/files.rs").read_bytes()
    try:
        image = phase("positive-build", command, COMPILE_SECONDS, compile_image=True)
        case("metadata", image, "metadata")
        case("freshness", image, "managed::freshness::tests::")
        phase("strict", strict, COMPILE_SECONDS)
        try:
            ownership.require_closed()
            subprocess.run(["git", "apply", str(patch)], cwd=source, check=True)
            image = phase("removed-noop-build", command, COMPILE_SECONDS, compile_image=True)
            case("removed-noop", image, CASE, negative=True, exact=True)
        finally:
            ownership.restore(source / "crates/sail-verus/src/managed/files.rs", original)
        if {name: sha(source / name) for name in before} != before:
            raise ValueError("positive source restoration mismatch")
        image = phase("restored-positive-build", command, COMPILE_SECONDS, compile_image=True)
        case("restored-positive", image, CASE, exact=True)
        results["status"] = "PASS"
    except BaseException as error:
        results["status"] = "FAIL"
        results["error_kind"] = type(error).__name__
    finally:
        results["closure_uncertainty_reasons"] = list(ownership.reasons)
        results["process_group_closure_uncertain"] = bool(ownership.reasons)
        if ownership.reasons or ownership.active is not None or ownership.spawning:
            results["source_equal"] = None
            results["lock_equal"] = None
        else:
            results["source_equal"] = {name: sha(source / name) for name in before} == before
            results["lock_equal"] = sha(source / "Cargo.lock") == lock_sha
            if not results["source_equal"] or not results["lock_equal"]:
                results["status"] = "FAIL"
        save()
    raise SystemExit(0 if results["status"] == "PASS" else 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--target", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--execute", action="store_true")
    run(parser.parse_args())


if __name__ == "__main__":
    main()
