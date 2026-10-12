#!/usr/bin/env python3
"""One complete reader acceptance run on a native Linux host.

Drives the unchanged controller in scripts/m7-reader-acceptance.py (fresh
independent baseline/active preparation, then the Python and Rust four-client,
200-sample censuses under active verification), then the retained CLI/MCP
check in scripts/m7-retained-jobs-acceptance.py against the same service.
Deadlines and thresholds are the controllers' own; nothing here changes them.
See docs/reader-acceptance.md for setup.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import subprocess
import sys
import time
from types import SimpleNamespace

HERE = Path(__file__).resolve().parent
PACKAGE = "human_eval_verus"
FUNCTION = "human_eval_verus::human_eval_013::compute_gcd"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def quantile(values, percent):
    ordered = sorted(values)
    return ordered[max(0, (percent * len(ordered) + 99) // 100 - 1)]


def report(run):
    """Print p50/p95/p99 per collector and operation (collector p95 rule)."""
    for name in ("python-census", "rust-census"):
        path = run.out / (name + ".json")
        if not path.exists():
            continue
        census = json.loads(path.read_bytes())
        for operation in ("status", "result"):
            values = [s["wall_ms"] for s in census["samples"] if s["operation"] == operation]
            print(f"{name} {operation}: n={len(values)} p50={quantile(values, 50):.2f} "
                  f"p95={quantile(values, 95):.2f} p99={quantile(values, 99):.2f} ms")


def install(target, bin_dir, probe_dir):
    """Copy built binaries into new product/probe directories with freeze records."""
    copies = {bin_dir: {"sail": target / "release/sail", "sail-z3": target / "release/sail-z3"},
              probe_dir: {"job_read_bench": target / "release/examples/job_read_bench",
                          "python-reader-probe.py": HERE / "m7-job-read-bench.py"}}
    for directory, files in copies.items():
        directory.mkdir(parents=True, exist_ok=False)
        frozen = {}
        for name, source in files.items():
            payload = source.read_bytes()
            (directory / name).write_bytes(payload)
            (directory / name).chmod(0o700)
            frozen[name] = {"sha256": hashlib.sha256(payload).hexdigest()}
        (directory / "freeze.json").write_text(json.dumps({"files": frozen}, indent=1) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("bin-dir", "probe-bin-dir", "source-workspace", "source-runtime", "stock-bin-dir", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--environment-label", required=True)
    parser.add_argument("--install-from", type=Path,
                        help="cargo target directory to install new bin/probe directories from")
    parser.add_argument("--timings", action="store_true",
                        help="diagnostic only: SAIL_JOB_RPC_TIMINGS=1 and SAIL_FRESHNESS_TRACE=1")
    parser.add_argument("--no-retained", action="store_true", help="skip the retained CLI/MCP check")
    args = parser.parse_args()
    if args.install_from is not None:
        install(args.install_from, args.bin_dir, args.probe_bin_dir)
    began = time.monotonic()
    controller = load("reader_acceptance", HERE / "m7-reader-acceptance.py")
    run = controller.Run(SimpleNamespace(
        out=args.out, bin_dir=args.bin_dir, source_workspace=args.source_workspace,
        source_runtime=args.source_runtime, probe_bin_dir=args.probe_bin_dir,
        python_probe=args.probe_bin_dir / "python-reader-probe.py", prepare_contexts=True,
        keep_daemon=not args.no_retained, setup_timeout=600, lease_inventory=None,
        package=PACKAGE, target=PACKAGE, kind="lib", function=FUNCTION,
        environment_label=args.environment_label))
    run.env.update(SAIL_CARGO_VERUS=str(args.stock_bin_dir / "cargo-verus"),
                   VERUS_Z3_PATH=str(args.stock_bin_dir / "z3"),
                   SAIL_JOB_EVENTS=str(run.out / "setup-stage-events.jsonl"))
    if args.timings:
        run.env.update(SAIL_JOB_RPC_TIMINGS="1", SAIL_FRESHNESS_TRACE="1")
    run.summary["diagnostic_timings"] = args.timings
    status = 0
    try:
        run.run()
        if not args.no_retained:
            job = run.summary["proofs"]["baseline"]["job"]
            run.command("retained-integration", [
                sys.executable, str(HERE / "m7-retained-jobs-acceptance.py"),
                "--sail", str(run.bin / "sail"), "--project", str(run.out / "baseline"),
                "--cache", str(run.cache), "--socket", str(run.socket),
                "--out", str(run.out / "retained-raw"), "--job", job,
                "--package", PACKAGE, "--target", PACKAGE, "--kind", "lib",
                "--function", FUNCTION, "--environment-label", args.environment_label,
            ], run.out / "baseline", timeout=145, json_output=False)
            retained = json.loads((run.out / "retained-raw/summary.json").read_bytes())
            run.summary["retained_passed"] = retained["passed"] is True
            if not run.summary["retained_passed"]:
                raise AssertionError("retained CLI/MCP check failed")
    except BaseException as error:
        run.summary["passed"] = False
        run.summary["error"] = {"type": type(error).__name__, "message": str(error)}
        status = 1
    finally:
        daemon = run.daemon
        if daemon is not None and daemon.poll() is None:
            daemon.send_signal(signal.SIGINT)
            try:
                daemon.wait(timeout=10)
            except subprocess.TimeoutExpired:
                daemon.kill()
                daemon.wait(timeout=5)
        run.summary["wall_s"] = time.monotonic() - began
        run.save()
    report(run)
    print("passed", run.summary.get("passed"), "retained", run.summary.get("retained_passed"),
          "error", run.summary.get("error"))
    return status


if __name__ == "__main__":
    sys.exit(main())
