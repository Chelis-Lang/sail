#!/usr/bin/env python3
"""Spike driver (#558): first unchanged call at several delays after a verifying run.

    spike-firstcall.py PROJECT PRISTINE OUT ROUNDS --stock DIR --arm NAME=BIN_DIR...

PRISTINE is a prepared project copy (scripts/perf-real.sh's `pristine`).
Each Sail arm and stock get one copy and, for Sail, one cache and daemon.
After one cold run per arm, each round runs every delay (rotated) on every
arm (rotated). One cycle on a Sail arm is: a unique comment-only edit, a
verifying run (timed: `verify`), a sleep of the delay, the first unchanged
call (`first`), three seconds, a second unchanged call (`second`).
Stock runs one verifying edit per round, then one unchanged call after each
delay in the round's order (stock keeps no first-call state).
Every sample goes to OUT/samples.jsonl; nothing is dropped.
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

PROJECTS = {
    "ironkv": dict(pkg="ironsht", cargs=[], vargs=[], noop="ironsht/src/host_impl_v.rs", toolchain="1.99.1"),
    "vest": dict(pkg="vest_lib", cargs=[], vargs=["--rlimit", "100"], noop="vest_lib/src/asn1/der_ord.rs",
                 toolchain="1.99.1"),
    "capybarakv": dict(pkg="capybaraKV/capybarakv", cargs=["--no-default-features"], vargs=[],
                       noop="capybaraKV/capybarakv/src/kv2/rwkv_v.rs", toolchain="1.99.1"),
    "anvil": dict(pkg=".", cargs=["--lib"], vargs=["--verify-module", "vdeployment_controller"],
                  noop="src/controllers/vdeployment_controller/proof/liveness/resource_match.rs", toolchain=None,
                  partial=True),
}
DELAYS = [0.0, 0.5, 2.0, 5.0]


def base_env(toolchain):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("SAIL_", "VERUS_")) and k != "CARGO_TARGET_DIR"}
    if toolchain:
        env["RUSTUP_TOOLCHAIN"] = toolchain
    return env


class Arm:
    def __init__(self, name, kind, path_prefix, out, pristine, cfg):
        self.name, self.kind, self.cfg = name, kind, cfg
        self.dir = out / name / "proj"
        self.cache = out / name / "c"
        self.logs = out / name / "logs"
        shutil.rmtree(out / name, ignore_errors=True)
        self.logs.mkdir(parents=True)
        shutil.copytree(pristine, self.dir, symlinks=True)
        self.env = base_env(cfg["toolchain"])
        self.env["PATH"] = path_prefix + os.pathsep + self.env["PATH"]
        partial = cfg.get("partial", False)
        if kind == "sail":
            self.cache.mkdir()
            self.env["SAIL_CACHE_DIR"] = str(self.cache)
            self.cmd = ["sail", "verify"] + (["--output", "json"] if partial else [])
        else:
            self.cmd = ["cargo", "verus", "focus" if partial else "verify"]
        self.cmd += cfg["cargs"] + ["--"] + cfg["vargs"]
        self.noop = self.dir / cfg["noop"]
        self.original = self.noop.read_bytes()
        self.counter = 0

    def edit(self):
        self.counter += 1
        temporary = self.noop.with_suffix(".tmp-spike")
        temporary.write_bytes(self.original + f"// sail-spike noop {self.name} {self.counter}\n".encode())
        temporary.replace(self.noop)

    def run(self, label):
        log = self.logs / f"{label}.log"
        with open(log, "wb") as f:
            t0 = time.perf_counter()
            p = subprocess.run(self.cmd, cwd=self.dir / self.cfg["pkg"], env=self.env, stdout=f,
                               stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
            wall = time.perf_counter() - t0
        return wall, p.returncode

    def stop(self, script):
        if self.kind == "sail":
            subprocess.run(["bash", str(script)], env=self.env, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("project")
    ap.add_argument("pristine")
    ap.add_argument("out")
    ap.add_argument("rounds", type=int)
    ap.add_argument("--stock", required=True)
    ap.add_argument("--arm", action="append", default=[])
    ap.add_argument("--sail-env", action="append", default=[], help="K=V for every Sail arm (diagnosis)")
    args = ap.parse_args()
    cfg = PROJECTS[args.project]
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    samples = out / "samples.jsonl"

    def record(**s):
        s["project"] = args.project
        with open(samples, "a") as f:
            f.write(json.dumps(s, sort_keys=True) + "\n")
        print(f"  r{s['round']} {s['arm']:6} d={s['delay']} {s['phase']:7} {s['wall']:8.3f} s exit {s['exit']}",
              flush=True)

    pristine = Path(args.pristine)
    sail_arms = []
    for spec in args.arm:
        name, bin_dir = spec.split("=", 1)
        sail_arms.append(Arm(name, "sail", bin_dir + os.pathsep + args.stock, out, pristine, cfg))
        for pair in args.sail_env:
            k, v = pair.split("=", 1)
            sail_arms[-1].env[k] = v.replace("{arm}", str(out / name))
    stock = Arm("stock", "stock", args.stock, out, pristine, cfg)
    stop = Path(__file__).resolve().parent / "sail-stop-daemon.sh"
    try:
        for arm in [stock] + sail_arms:
            wall, code = arm.run("cold")
            record(round=0, arm=arm.name, delay=None, phase="cold", wall=wall, exit=code)
            time.sleep(5)
            wall, code = arm.run("cold-unchanged")
            record(round=0, arm=arm.name, delay=None, phase="warmup", wall=wall, exit=code)
            time.sleep(3)
        for rnd in range(1, args.rounds + 1):
            delays = DELAYS[(rnd - 1) % 4:] + DELAYS[:(rnd - 1) % 4]
            for index, delay in enumerate(delays):
                order = sail_arms[(rnd + index) % len(sail_arms):] + sail_arms[:(rnd + index) % len(sail_arms)]
                for arm in order:
                    time.sleep(3)
                    arm.edit()
                    wall, code = arm.run(f"r{rnd}-d{delay}-verify")
                    record(round=rnd, arm=arm.name, delay=delay, phase="verify", wall=wall, exit=code)
                    time.sleep(delay)
                    wall, code = arm.run(f"r{rnd}-d{delay}-first")
                    record(round=rnd, arm=arm.name, delay=delay, phase="first", wall=wall, exit=code)
                    time.sleep(3)
                    wall, code = arm.run(f"r{rnd}-d{delay}-second")
                    record(round=rnd, arm=arm.name, delay=delay, phase="second", wall=wall, exit=code)
            time.sleep(3)
            stock.edit()
            wall, code = stock.run(f"r{rnd}-verify")
            record(round=rnd, arm="stock", delay=None, phase="verify", wall=wall, exit=code)
            for delay in delays:
                time.sleep(delay)
                wall, code = stock.run(f"r{rnd}-d{delay}-unchanged")
                record(round=rnd, arm="stock", delay=delay, phase="first", wall=wall, exit=code)
    finally:
        for arm in sail_arms:
            for log in arm.cache.rglob("adopt.log"):
                shutil.copy(log, out / f"{arm.name}.adopt.log")
            arm.stop(stop)
    report(samples)


def report(samples):
    rows = [json.loads(line) for line in open(samples)]
    groups = {}
    for r in rows:
        if r["round"] == 0:
            continue
        groups.setdefault((r["arm"], r["phase"], r["delay"]), []).append(r["wall"])
    print("\narm     phase    delay  n  median    min      max")
    for (arm, phase, delay), walls in sorted(groups.items(), key=lambda kv: (kv[0][1], str(kv[0][2]), kv[0][0])):
        print(f"{arm:7} {phase:8} {delay!s:5} {len(walls):2} {statistics.median(walls):8.3f} "
              f"{min(walls):8.3f} {max(walls):8.3f}")
    bad = [r for r in rows if r["exit"] != 0]
    print(f"\nnonzero exits: {len(bad)}")


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "report":
        report(sys.argv[2])
    else:
        main()
