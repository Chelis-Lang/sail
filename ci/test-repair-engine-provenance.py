#!/usr/bin/env python3
"""Real prebuilt-engine provenance controls (ADR 0027).

Runs beside the unchanged repair acceptance fixture (ci/test-repair.py) and
reuses its process helpers. It checks, against the actual engine, that engine
results name the engine and its manifest digest in project, report and
function provenance, with mode still `default`, and that the batch carries no
prepared-lowering telemetry (the engine has none).
"""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import time

spec = importlib.util.spec_from_file_location("repair_acceptance", Path(__file__).with_name("test-repair.py"))
acceptance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(acceptance)


def verdicts(target):
    return sorted((f["function"], f["verdict"]) for c in target["report"]["crates"] for f in c["functions"])


def run(args):
    assert args.timeout > 0, "control timeout must be positive"
    assert os.environ.get("CI", "").lower() != "true" or args.timeout <= 120, "CI proof/tool budget cannot exceed 120 seconds"
    sail = str(Path(args.sail).resolve())
    engine = Path(args.engine_build).resolve()
    source_tree = Path(args.verus_source).resolve()
    evidence = Path(args.out).resolve()
    evidence.mkdir(parents=True, exist_ok=True, mode=0o700)
    builtin = source_tree / "source/builtin"
    macros = source_tree / "source/builtin_macros"
    assert (builtin / "Cargo.toml").is_file() and (macros / "Cargo.toml").is_file()
    with tempfile.TemporaryDirectory(prefix="sp-", dir="/tmp") as temporary:
        root = Path(temporary)
        project = root / "project"
        (project / "src").mkdir(parents=True)
        (project / "Cargo.toml").write_text(
            '[package]\nname="repair_demo"\nversion="0.1.0"\nedition="2021"\n'
            '[package.metadata.verus]\nverify=true\nno-vstd=true\n[dependencies]\n'
            f'verus_builtin={{path={json.dumps(str(builtin))}}}\n'
            f'verus_builtin_macros={{path={json.dumps(str(macros))}}}\n')
        (project / "src/lib.rs").write_text(
            "use verus_builtin::*;\nuse verus_builtin_macros::*;\nverus! {\n" + acceptance.GOOD + "\n}\n")
        env = dict(os.environ, SAIL_CACHE_DIR=str(root / "cache"), SAIL_DAEMON_SOCKET=str(root / "d.sock"))
        env.pop("VSTD_KIND", None)
        setup = acceptance.execute([sail, "engine", "install", "--from", str(engine), "--output", "json"], project, env, 30)
        (evidence / "engine-install.stdout.json").write_bytes(setup.stdout)
        setup.check_returncode()
        status = json.loads(setup.stdout)
        env["RUSTUP_TOOLCHAIN"] = status["rust_toolchain"]
        setup = acceptance.execute(["cargo", "+" + status["rust_toolchain"], "fetch"], project, env, 60)
        setup.check_returncode()
        daemon_errors = (evidence / "daemon.stderr").open("wb")
        daemon = subprocess.Popen([sail, "serve", "--cache", env["SAIL_CACHE_DIR"], "--socket",
                                   env["SAIL_DAEMON_SOCKET"], "--idle-timeout", "0"], env=env,
                                  stdout=subprocess.DEVNULL, stderr=daemon_errors, start_new_session=True)
        started = time.monotonic()
        deadline = started + args.timeout
        summary = {"version": 1, "kind": "actual-prebuilt-engine-provenance-controls",
                   "machine": platform.platform(), "environment_label": args.environment_label,
                   "engine_manifest_sha256": status["manifest_sha256"],
                   "sail_sha256": hashlib.sha256(Path(sail).read_bytes()).hexdigest(), "passed": False}
        operations = []

        def remaining(maximum=60):
            value = min(maximum, deadline - time.monotonic())
            assert value > 0, f"engine provenance controls exceeded {args.timeout:g} seconds"
            return value

        def cli(label, arguments):
            before = time.monotonic()
            result = acceptance.execute([sail, *arguments], project, env, remaining())
            (evidence / (label + ".stdout.json")).write_bytes(result.stdout)
            (evidence / (label + ".stderr")).write_bytes(result.stderr)
            operations.append({"operation": label, "elapsed_s": time.monotonic() - before,
                               "exit_code": result.returncode})
            assert result.returncode == 0, (label, result.returncode, result.stderr.decode(errors="replace"))
            return json.loads(result.stdout)

        def submit(label):
            job = cli(label + "-submit", ["repair-check", "--engine", "interactive", "--package", "repair_demo",
                                          "--lib", "--function", "repair_demo::selected", "--background",
                                          "--output", "json"])["id"]
            attempt = 0
            while True:
                value = cli(f"{label}-result-{attempt}", ["jobs", "result", job, "--output", "json"])["value"]
                if value["job"]["state"] == "completed":
                    document = value["repair"]
                    full = document["batch"]["full"]
                    targets = [t for t in full["targets"] if t["requested"]]
                    assert len(targets) == 1 and targets[0]["target"] == "repair_demo", targets
                    return document, full, targets[0]
                assert value["job"]["state"] in ("queued", "running"), value
                remaining()
                attempt += 1
                time.sleep(.25)

        def provenance(full, target):
            identity = {"name": "interactive", "manifest_sha256": status["manifest_sha256"]}
            assert full["engine"] == identity, full
            engine = dict(identity, established_by="engine")
            assert target["report"]["engine"] == engine, target
            assert target["report"]["mode"] == "default", target
            functions = [f for c in target["report"]["crates"] for f in c["functions"]]
            assert functions and all(f["provenance"]["engine"] == engine and f["provenance"]["mode"] == "default"
                                     for f in functions), target

        try:
            while not Path(env["SAIL_DAEMON_SOCKET"]).exists():
                assert daemon.poll() is None, "owned daemon failed before the controls"
                remaining(5)
                time.sleep(.02)
            document, default_full, default_target = submit("default")
            assert "preparation" not in document["batch"], document
            assert "engine_fault" not in document["batch"], document
            assert default_full["exit_code"] == 0 and default_target["complete"], document
            provenance(default_full, default_target)
            summary["passed"] = True
        finally:
            acceptance.stop_owned(daemon, 10)
            daemon_errors.close()
            summary.update(elapsed_s=time.monotonic() - started, operations=operations)
            (evidence / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print("Engine provenance controls: project, report and function provenance", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("sail", "engine-build", "verus-source", "out"):
        parser.add_argument("--" + flag, required=True)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--environment-label", default="unspecified")
    run(parser.parse_args())
