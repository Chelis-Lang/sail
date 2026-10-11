#!/usr/bin/env python3
"""One root-granted, private Linux HTTP control/sensitivity/comparison scope."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import time


CASE = "shared::http_tests::concurrent_readers_coalesce_one_http_lookup"
SENSITIVE = "remote::refresh_tests::small_exact_body_avoids_eof_read_with_only_existing_fixed_capacity"
REMOTE = "crates/sail-daemon/src/remote.rs"
SHARED = "crates/sail-daemon/src/shared.rs"
PRIVATE_TARGET = Path("/tmp/sail-m7-shared-trust-fix/target")


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def seal(path):
    stat = path.stat()
    return {"path": str(path), "bytes": stat.st_size,
            "mode": stat.st_mode & 0o7777, "sha256": digest(path)}


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def pid1_birth():
    # Linux stat field 22, parsed after the final comm parenthesis.
    raw = Path("/proc/1/stat").read_text()
    return int(raw.rsplit(")", 1)[1].split()[19])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--target", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--grant", type=Path, required=True)
    args = parser.parse_args()
    payload = Path(__file__).resolve().parent
    inputs = json.loads((payload / "inputs.json").read_text())
    grant = json.loads(args.grant.read_text())
    assert grant["exclusive_compile_slot"] is True
    assert grant["controller_sha256"] == digest(Path(__file__))
    assert grant["inputs_sha256"] == digest(payload / "inputs.json")
    assert args.target.resolve() == PRIVATE_TARGET
    assert Path(grant["target"]).resolve() == args.target.resolve()
    assert Path(grant["source"]).resolve() == args.source.resolve()
    assert grant["container_pid1_start_ticks"] == pid1_birth()
    assert grant["dual_container_target_holders_closed"] is True
    assert digest(Path(grant["target_handoff_record"])) == grant["target_handoff_sha256"]
    for name, expected in inputs["payloads"].items():
        assert digest(payload / name) == expected, name
    for name, expected in inputs["candidate_sources"].items():
        assert digest(args.source / name) == expected, name
    # The lead supplies the whole Git-archive source inventory and protected
    # product/runtime/tested-image inventory after the live target handoff.
    inventory_path = Path(grant["source_inventory"])
    assert digest(inventory_path) == grant["source_inventory_sha256"]
    inventory = json.loads(inventory_path.read_text())
    assert inventory["git_commit"] == inputs["candidate_commit"]
    protected_path = Path(grant["protected_inventory"])
    assert digest(protected_path) == grant["protected_inventory_sha256"]
    protected = json.loads(protected_path.read_text())["files"]

    def source_check():
        for name, expected in inventory["files"].items():
            path = args.source / name
            assert not path.is_symlink() and digest(path) == expected, name
        for name, target in inventory["symlinks"].items():
            path = args.source / name
            assert path.is_symlink() and os.readlink(path) == target, name

    def protected_check():
        for row in protected:
            path = Path(row["path"])
            assert not path.is_symlink() and seal(path) == row, row["path"]

    source_check()
    protected_check()
    args.output.mkdir(exist_ok=False)
    images = args.output / "tested-images"
    images.mkdir()
    before = {name: (args.source / name).read_bytes() for name in (REMOTE, SHARED)}
    env = os.environ.copy()
    env.update(CARGO_BUILD_JOBS="1", CARGO_INCREMENTAL="0")
    phases = []
    active = None
    spawning = False
    started = time.monotonic()
    result = {"status": "RUNNING", "scope": "affected HTTP controls, isolated sensitivity and one matched comparison; unselected",
              "candidate_commit": inputs["candidate_commit"], "grant": seal(args.grant),
              "controller": seal(Path(__file__)), "inputs": seal(payload / "inputs.json"),
              "target": str(args.target), "profile": "release", "jobs": 1,
              "warm_target_claim": False, "phases": phases}
    write(args.output / "execution.json", result)

    def interrupt(number, _frame):
        for value in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(value, signal.SIG_IGN)
        raise InterruptedError("controller signal " + str(number))

    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, interrupt)

    def run(command, label, limit):
        nonlocal active, spawning
        row = {"name": label, "command": command, "limit_seconds": limit}
        begin = time.monotonic()
        phases.append(row)
        try:
            with (args.output / (label + ".stdout")).open("wb") as stdout, (args.output / (label + ".stderr")).open("wb") as stderr:
                spawning = True
                active = subprocess.Popen(command, cwd=args.source, env=env,
                                          stdout=stdout, stderr=stderr, start_new_session=True)
                spawning = False
                row.update(pid=active.pid, pgid=active.pid,
                           launched_unix_ns=time.time_ns())
                write(args.output / "progress.json", result)
                try:
                    active.wait(timeout=limit)
                finally:
                    if active.poll() is None:
                        os.killpg(active.pid, signal.SIGTERM)
                        try:
                            active.wait(timeout=2)
                        except subprocess.TimeoutExpired:
                            os.killpg(active.pid, signal.SIGKILL)
                            active.wait(timeout=2)
                    row["exit_code"] = active.returncode
                    active = None
        finally:
            row["elapsed_seconds"] = time.monotonic() - begin
            row["raw"] = [seal(args.output / (label + suffix)) for suffix in (".stdout", ".stderr")]
            write(args.output / "progress.json", result)
        return row

    def build(label, integration=False):
        command = ["cargo", "test", "--locked", "--release", "--target-dir", str(args.target),
                   "-p", "sail-daemon", "--no-run", "--message-format=json"]
        command += ["--test", "http_shared"] if integration else ["--lib"]
        row = run(command, label, 600)
        assert row["exit_code"] == 0, label
        artifacts = []
        dependency_features = []
        for line in (args.output / (label + ".stdout")).read_text().splitlines():
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                continue
            if record.get("reason") == "compiler-artifact" and record.get("executable"):
                if record["target"]["name"] == ("http_shared" if integration else "sail_daemon"):
                    artifacts.append(record)
            if record.get("reason") == "compiler-artifact" and record["target"]["name"] == "reqwest":
                dependency_features.append(record["features"])
        assert len(artifacts) == 1, (label, artifacts)
        artifact = artifacts[0]
        image = Path(artifact["executable"])
        retained = images / label
        shutil.copy2(image, retained)
        assert digest(image) == digest(retained)
        row["cargo_fresh"] = artifact["fresh"]
        assert len(dependency_features) == 1, dependency_features
        features = dependency_features[0]
        assert "blocking" in features and "rustls-no-provider" in features
        assert not set(features) & {"default", "gzip", "brotli", "zstd", "deflate", "http2"}
        row["reqwest_features"] = features
        row["tested_image"] = seal(retained)
        write(args.output / "progress.json", result)
        return retained

    def case(image, label, name=None, expected=0, limit=180):
        command = [str(image)]
        if name:
            command += [name, "--exact"]
        command += ["--nocapture", "--test-threads=1"]
        row = run(command, label, limit)
        text = (args.output / (label + ".stdout")).read_text()
        assert row["exit_code"] == expected, label
        assert re.search(r"running [1-9][0-9]* tests?", text), label
        return row

    def patch(name):
        subprocess.run(["git", "apply", "--check", str(payload / name)], cwd=args.source, check=True)
        subprocess.run(["git", "apply", str(payload / name)], cwd=args.source, check=True)

    def restore():
        for name, data in before.items():
            (args.source / name).write_bytes(data)

    try:
        version = run(["rustc", "--version", "--verbose"], "compiler-identity", 10)
        assert version["exit_code"] == 0
        assert "release: 1.99.0" in (args.output / "compiler-identity.stdout").read_text()
        unit = build("candidate-unit-build")
        # Execute the original library HTTP set and private refresh controls,
        # including the two new real Read-count/capacity controls.
        for label, name in [("shared-http", "shared::http_tests::"), ("remote-refresh", "remote::refresh_tests::")]:
            row = run([str(unit), name, "--nocapture", "--test-threads=1"], label, 180)
            assert row["exit_code"] == 0, label
            assert re.search(r"running [1-9][0-9]* tests?", (args.output / (label + ".stdout")).read_text()), label
        integration = build("candidate-integration-build", True)
        case(integration, "complete-http-integration")
        strict = run(["cargo", "clippy", "--locked", "--release", "--target-dir", str(args.target),
                      "-p", "sail-daemon", "--all-targets", "--", "-D", "warnings"], "strict-all-targets", 600)
        assert strict["exit_code"] == 0, "strict-all-targets"
        patch("original-stream-sensitivity.patch")
        negative = build("original-stream-sensitive-build")
        case(negative, "original-stream-count-refusal", SENSITIVE, 101)
        failure = (args.output / "original-stream-count-refusal.stderr").read_text()
        assert "assertion `left == right` failed" in failure
        assert re.search(r"left:\s*1\s*right:\s*0", failure), failure
        restore()
        positive = build("restored-positive-build")
        case(positive, "restored-positive-count", SENSITIVE)
        # Each comparison arm is invoked ONCE. A failing baseline is retained
        # while the predetermined candidate observation still runs; neither is
        # counted as passing if its unchanged fixture assertions fail.
        comparison = []
        for arm in ("baseline", "candidate"):
            restore()
            if arm == "baseline":
                (args.source / REMOTE).write_bytes((payload / "baseline-remote.rs").read_bytes())
            patch(arm + "-observation.patch")
            image = build(arm + "-matched-build")
            row = run([str(image), CASE, "--exact", "--nocapture", "--test-threads=1"], arm + "-matched-lookup", 30)
            text = (args.output / (arm + "-matched-lookup.stdout")).read_text()
            assert "running 1 test" in text
            comparison.append(row["exit_code"])
        assert comparison == [0, 0], comparison
        result["status"] = "PASS"
    except BaseException as error:
        result.update(status="FAIL", error_type=type(error).__name__, error=str(error))
    finally:
        # Preserve the primary scope before independent source/protected closure.
        result["elapsed_seconds"] = time.monotonic() - started
        write(args.output / "primary.json", result)
        try:
            assert active is None and not spawning, "child ownership/retirement unavailable; leave applied source untouched"
            restore()
            source_check()
            protected_check()
            assert pid1_birth() == grant["container_pid1_start_ticks"]
            result["closure"] = {"source_restored": True, "protected_files_equal": True,
                                 "container_birth_equal": True, "owned_child_leaders_reaped": active is None and not spawning,
                                 "independent_dual_container_holder_audit": "required by lead after controller exit"}
        except BaseException as error:
            result["closure"] = {"status": "unavailable", "error": str(error)}
            result["status"] = "FAIL"
        write(args.output / "result.json", result)
    raise SystemExit(0 if result["status"] == "PASS" else 1)


if __name__ == "__main__":
    main()
