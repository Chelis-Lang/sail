#!/usr/bin/env python3
"""Spike driver (#560): the tiny project's unchanged, edit and revert rows with switches.

    spike-tiny.py OUT ROUNDS --stock DIR --sail BIN_DIR [--variant NAME=K=V,K=V]...

Arms: `stock`, `sail` (defaults) and one Sail arm per --variant with the given
environment on every call. Each round gives each arm a fresh project, Cargo
home and Sail cache, in rotated order, and runs, with a five-second settle
before each timed call (scripts/perf-matrix.py's small-project recipe):
cold-first (new + verify), unchanged, small-edit, revert, then full-path
(an unchanged call with SAIL_MANAGED_NOOP=0). A variant whose environment
names SAIL_JOB_EVENTS is a diagnostic arm: its managed-setup-phase rows are
collected per scenario.
"""
import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

MIN_FN = """pub fn min(a: u64, b: u64) -> (r: u64)
    ensures
        r <= a && r <= b,
        r == a || r == b,
{
    if a <= b { a } else { b }
}

"""
SCENARIOS = ["cold-first", "unchanged", "small-edit", "revert", "full-path"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("rounds", type=int)
    ap.add_argument("--stock", required=True)
    ap.add_argument("--sail", required=True)
    ap.add_argument("--variant", action="append", default=[])
    ap.add_argument("--settle", type=float, default=5.0)
    args = ap.parse_args()
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    samples = out / "samples.jsonl"
    stop = Path(__file__).resolve().parent / "sail-stop-daemon.sh"
    arms = [("stock", {}), ("sail", {})]
    for spec in args.variant:
        name, pairs = spec.split("=", 1)
        arms.append((name, dict(pair.split("=", 1) for pair in pairs.split(",") if pair)))

    for rnd in range(1, args.rounds + 1):
        order = arms[(rnd - 1) % len(arms):] + arms[:(rnd - 1) % len(arms)]
        for name, extra in order:
            base = out / f"r{rnd}" / name
            shutil.rmtree(base, ignore_errors=True)
            base.mkdir(parents=True)
            env = {k: v for k, v in os.environ.items()
                   if not k.startswith(("SAIL_", "VERUS_")) and k != "CARGO_TARGET_DIR"}
            path = args.stock + os.pathsep + env["PATH"]
            if name != "stock":
                path = args.sail + os.pathsep + path
                env["SAIL_CACHE_DIR"] = str(base / "c")
            env["PATH"] = path
            env["CARGO_HOME"] = str(base / "cargo-home")
            events = None
            for k, v in extra.items():
                env[k] = v.replace("{base}", str(base))
                if k == "SAIL_JOB_EVENTS":
                    events = Path(env[k])
                    events.parent.mkdir(parents=True, exist_ok=True)
            sail = name != "stock"
            new = ["sail", "new", "--lib", "demo"] if sail else ["cargo", "verus", "new", "--lib", "demo"]
            verify = ["sail", "verify"] if sail else ["cargo", "verus", "verify"]
            demo = base / "demo"
            lib = demo / "src" / "lib.rs"
            template = None
            for scenario in SCENARIOS:
                call_env = dict(env)
                if scenario == "small-edit":
                    template = lib.read_text()
                    marker = "} // verus!"
                    assert template.count(marker) == 1
                    lib.write_text(template.replace(marker, MIN_FN + marker))
                elif scenario == "revert":
                    lib.write_text(template)
                elif scenario == "full-path":
                    call_env["SAIL_MANAGED_NOOP"] = "0"
                steps = [(new, base), (verify, demo)] if scenario == "cold-first" else [(verify, demo)]
                time.sleep(args.settle)
                code = 0
                with open(base / f"{scenario}.log", "wb") as f:
                    t0 = time.perf_counter()
                    for cmd, cwd in steps:
                        code = subprocess.run(cmd, cwd=cwd, env=call_env, stdout=f, stderr=subprocess.STDOUT,
                                              stdin=subprocess.DEVNULL).returncode
                        if code:
                            break
                    wall = time.perf_counter() - t0
                phases = {}
                if events is not None:
                    time.sleep(0.2)
                    for file in sorted(events.parent.glob("*.setup-*.jsonl")):
                        for line in file.read_text().splitlines():
                            row = json.loads(line)
                            if row.get("kind") == "managed-setup-phase":
                                phases[row["phase"]] = phases.get(row["phase"], 0.0) + row["elapsed_ms"]
                        file.unlink()
                sample = dict(round=rnd, arm=name, scenario=scenario, wall=wall, exit=code, phases=phases)
                with open(samples, "a") as f:
                    f.write(json.dumps(sample, sort_keys=True) + "\n")
                print(f"  r{rnd} {name:10} {scenario:11} {wall:8.3f} s exit {code} "
                      + " ".join(f"{k}={v:.0f}" for k, v in phases.items()), flush=True)
            if sail:
                subprocess.run(["bash", str(stop)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                time.sleep(1)
            shutil.rmtree(base / "demo" / "target", ignore_errors=True)
            shutil.rmtree(base / "c", ignore_errors=True)
            shutil.rmtree(base / "cargo-home", ignore_errors=True)
    report(samples)


def report(samples):
    rows = [json.loads(line) for line in open(samples)]
    groups, phases = {}, {}
    for r in rows:
        groups.setdefault((r["scenario"], r["arm"]), []).append(r["wall"])
        for k, v in r.get("phases", {}).items():
            phases.setdefault((r["scenario"], r["arm"], k), []).append(v)
    print("\nscenario     arm         n  median    min      max")
    for (scenario, arm), walls in sorted(groups.items(), key=lambda kv: (SCENARIOS.index(kv[0][0]), kv[0][1])):
        print(f"{scenario:12} {arm:10} {len(walls):2} {statistics.median(walls):8.3f} {min(walls):8.3f} "
              f"{max(walls):8.3f}")
    if phases:
        print("\nscenario     arm        phase            n  median_ms   min     max")
        for (scenario, arm, phase), values in sorted(phases.items(),
                                                     key=lambda kv: (SCENARIOS.index(kv[0][0]), kv[0][1], kv[0][2])):
            print(f"{scenario:12} {arm:10} {phase:16} {len(values):2} {statistics.median(values):8.1f} "
                  f"{min(values):7.1f} {max(values):7.1f}")
    print(f"\nnonzero exits: {len([r for r in rows if r['exit'] != 0])}")


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "report":
        report(sys.argv[2])
    else:
        main()
