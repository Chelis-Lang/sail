#!/usr/bin/env python3
"""Actual Cargo engine forwarding oracle; real launcher and byte-exact full SMT."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import tempfile
import time


def sha(value):
    return hashlib.sha256(value).hexdigest()


def execute(command, directory, environment, timeout, output, label):
    started = time.monotonic()
    child = subprocess.Popen(command, cwd=directory, env=environment,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             start_new_session=True)
    try:
        stdout, stderr = child.communicate(timeout=timeout)
    except BaseException as failure:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        stdout, stderr = child.communicate(timeout=10)
        (output / (label + ".stdout")).write_bytes(stdout)
        (output / (label + ".stderr")).write_bytes(stderr)
        raise failure
    (output / (label + ".stdout")).write_bytes(stdout)
    (output / (label + ".stderr")).write_bytes(stderr)
    return child.returncode, stdout, stderr, time.monotonic() - started


def stop_owned(child):
    if child.poll() is None:
        child.terminate()
    try:
        child.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(child.pid, signal.SIGKILL)
        child.wait(timeout=10)


def pipe(path):
    return b"".join(part.split(b";;;<<<\n", 1)[0] + b'\n(echo "<<DONE>>")\n'
                    for part in path.read_bytes().split(b";;;>>> QUERY\n")[1:])


def run(args):
    sail = Path(args.sail).resolve()
    source = Path(args.source).resolve()
    output = Path(args.out).resolve()
    output.mkdir(parents=True, exist_ok=True)
    summary = {"version": 1, "kind": "actual-cargo-no-vstd-forwarding-oracle",
               "machine": platform.platform(), "environment_label": args.environment_label,
               "sail_sha256": sha(sail.read_bytes()), "passed": False,
               "invocation_bound_s": args.timeout}
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    with tempfile.TemporaryDirectory(prefix="snv-", dir="/tmp") as temporary:
        root = Path(temporary)
        project = root / "project"
        (project / "src").mkdir(parents=True)
        manifest = '[package]\nname="repair_demo"\nversion="0.1.0"\nedition="2021"\n[package.metadata.verus]\nverify=true\nno-vstd=true\n[dependencies]\n'
        for name, directory in [("verus_builtin", "builtin"), ("verus_builtin_macros", "builtin_macros")]:
            manifest += f'{name}={{path={json.dumps(str(source / "source" / directory))}}}\n'
        (project / "Cargo.toml").write_text(manifest)
        body = 'use verus_builtin::*;\nuse verus_builtin_macros::*;\nverus! {\nproof fn selected(x: bool) ensures x || !x { }\nproof fn other(x: bool) ensures x == x { }\n}\n'
        (project / "src/lib.rs").write_text(body)
        summary["source_sha256"] = sha(body.encode())
        environment = dict(os.environ, SAIL_CACHE_DIR=str(root / "cache"),
                           SAIL_DAEMON_SOCKET=str(root / "d.sock"))
        environment.pop("VSTD_KIND", None)
        code, raw, _, _ = execute([str(sail), "engine", "install", "--from", args.engine_build,
                                   "--output", "json"], project, environment, 30, output, "engine-install")
        assert code == 0, "actual engine installation failed"
        status = json.loads(raw)
        environment["RUSTUP_TOOLCHAIN"] = status["rust_toolchain"]
        launcher = Path(status["verus"])
        summary.update(engine_manifest_sha256=status["manifest_sha256"],
                       engine_patch_sha256=status["patch_sha256"],
                       engine_launcher_sha256=sha(launcher.read_bytes()),
                       rust_toolchain=status["rust_toolchain"])
        code, _, _, _ = execute(["cargo", "fetch"], project, environment, 60, output, "cargo-fetch")
        assert code == 0, "Cargo fixture dependency setup failed"
        with (output / "daemon.stderr").open("wb") as errors:
            daemon = subprocess.Popen([str(sail), "serve", "--cache", environment["SAIL_CACHE_DIR"],
                                       "--socket", environment["SAIL_DAEMON_SOCKET"], "--idle-timeout", "0"],
                                      env=environment, stdout=subprocess.DEVNULL, stderr=errors,
                                      start_new_session=True)
            try:
                code, raw, _, elapsed = execute([str(sail), "repair-check", "--engine", "interactive", "--function", "repair_demo::selected",
                                                 "--package", "repair_demo", "--lib", "--output", "json"],
                                                project, environment, args.timeout, output, "actual-cargo")
                summary.update(actual_cargo_exit_code=code, actual_cargo_elapsed_s=elapsed)
                assert raw, "actual Cargo repair returned no result; see preserved stderr"
                document = json.loads(raw)
                full = document["batch"]["full"]
                assert code in [0, 1] and full["exit_code"] == 0 and full["error"] is None
                targets = [target for target in full["targets"] if target["requested"]]
                assert len(targets) == 1 and targets[0]["complete"], "actual full Cargo proof is incomplete"
                assert document["batch"]["batched"] and document["batch"]["focus"] is not None
                options = list((root / "cache/projects").glob("*/*/runs/" + full["run_id"] + "/sidecars/*/launcher-options.json"))
                assert len(options) == 1, "adaptation did not preserve one exact target provenance record"
                sidecar = options[0].parent
                record = json.loads(options[0].read_text())
                assert record["forwarded"] == ["--no-vstd"] and record["engine_only"]
                assert record["run_id"] == full["run_id"] and record["context_sha256"] == full["context_sha256"]
                assert record["target"] == "repair_demo" and record["kind"] == "lib"
                direct_environment = dict(environment, **record["cargo_identity"])
                for key in list(direct_environment):
                    if key.startswith("SAIL_ENGINE") or key.startswith("SAIL_MANAGED"):
                        del direct_environment[key]
                direct_environment.update(RUSTC_BOOTSTRAP="1", __VERUS_DRIVER_VIA_CARGO__="1",
                                          VERUS_Z3_PATH=str(launcher.with_name("z3")))
                direct_environment["__VERUS_DRIVER_VERIFY_" + record["package_id"]] = "1"
                direct_environment.update(record["deferred_environment"])
                direct_args = [os.fsdecode(bytes(value)) for value in record["launcher_arguments"]]
                direct_logs = root / "direct-logs"
                direct_code, direct_raw, direct_errors, direct_elapsed = execute(
                    [str(launcher), *direct_args, "--output-json", "--time", "--log", "air",
                     "--log", "smt-transcript", "--log-dir", str(direct_logs)],
                    project, direct_environment, args.timeout, output, "direct-same-argv")
                actual_raw = (sidecar / "verus.stdout").read_bytes()
                actual_errors = (sidecar / "verus.stderr").read_bytes()
                (output / "actual-target.stdout").write_bytes(actual_raw)
                (output / "actual-target.stderr").write_bytes(actual_errors)
                actual, direct = json.loads(actual_raw), json.loads(direct_raw)
                assert direct_code == full["exit_code"]
                assert direct_errors == actual_errors, "full diagnostics differ from direct same-argv --no-vstd"
                for key in ["verification-results", "func-details"]:
                    assert actual[key] == direct[key], ("full Verus results differ", key)
                def verdicts(report):
                    return sorted((entry["function"], entry["success"])
                                  for module in report.get("times-ms", {}).get("smt", {}).get("smt-run-module-times", [])
                                  for entry in module.get("function-breakdown", []))
                assert verdicts(actual) == verdicts(direct) and len(verdicts(actual)) == 2
                actual_logs = sorted((sidecar / "logs").glob("*.smt_transcript"))
                baseline_logs = sorted(direct_logs.glob("*.smt_transcript"))
                assert [path.name for path in actual_logs] == [path.name for path in baseline_logs] and actual_logs
                sessions = []
                for actual_path, baseline_path in zip(actual_logs, baseline_logs):
                    actual_bytes, baseline_bytes = pipe(actual_path), pipe(baseline_path)
                    assert actual_bytes == baseline_bytes, "whole full-target SMT input differs"
                    sessions.append({"name": actual_path.name, "bytes": len(actual_bytes), "input_sha256": sha(actual_bytes)})
                duplicate_environment = dict(direct_environment,
                                             SAIL_MANAGED_WRAPPER=str(launcher),
                                             SAIL_MANAGED_RUNTIME=str(sidecar.parents[3] / "runtime.json"))
                duplicate_environment[" __VERUS_DRIVER_ARGS__"] += "__VERUS_DRIVER_ARGS_SEP__--no-vstd"
                duplicate_environment[" __VERUS_DRIVER_ARGS_FOR_" + record["package_id"]] = "__VERUS_DRIVER_ARGS_SEP__--no-vstd"
                adapter = root / "sail-verus-driver"
                adapter.symlink_to(sail)
                original_args = [os.fsdecode(bytes(value)) for value in record["original_arguments"]]
                duplicate_code, duplicate_raw, duplicate_errors, _ = execute(
                    [str(adapter), *original_args], project, duplicate_environment, 30, output, "duplicate-effective")
                assert duplicate_code == 2 and not duplicate_raw
                assert b"SAIL3121" in duplicate_errors and b"duplicate effective" in duplicate_errors
                # Persist only the owned fixture's exact invocation/allowlisted
                # identity, never the initiating process's general environment.
                (output / "launcher-options.json").write_text(json.dumps(record, indent=2) + "\n")
                summary.update(passed=True, actual_cargo_elapsed_s=elapsed, direct_elapsed_s=direct_elapsed,
                               actual_currentness=full["currentness"], currentness_is_not_oracle_acceptance=True,
                               full_verified=actual["verification-results"], full_function_verdicts=verdicts(actual),
                               full_sessions=sessions, diagnostics_sha256=sha(actual_errors),
                               duplicate_effective_refused=True)
            finally:
                stop_owned(daemon)
                (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ["sail", "engine-build", "source", "out"]:
        parser.add_argument("--" + option, required=True)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--environment-label", default="local")
    arguments = parser.parse_args()
    assert arguments.timeout > 0
    assert os.environ.get("CI", "").lower() != "true" or arguments.timeout <= 120
    run(arguments)
