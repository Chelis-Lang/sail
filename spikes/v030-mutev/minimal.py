#!/usr/bin/env python3
"""Spike A1: runs the minimal cases through stock Verus and through Sail.

    minimal.py --sail SAIL --shim SAIL_Z3 --z3 PINNED_Z3 --engine DIR --out DIR [--runs 5]

Run from the repository root, inside the CI image (stock `cargo verus` on
PATH). Per case, in its own `cargo verus new --lib` project:

- `stock-traced-K`: `cargo verus verify -- --output-json --time`, the solver
  being the pass-through shim in front of the pinned Z3 with the tap on
  (raw solver replies kept under `trace/`); the package is cleaned first so
  every run verifies again.
- `stock-plain`: the same command once with no shim (stock's bundled Z3).
- `sail-K`: `sail verify --output json` with an empty Sail cache per run and
  the package cleaned from the shared target directory first.

Everything is kept under OUT; `analyze.py` reads it.
"""
import argparse
import json
import os
import shutil
import subprocess
from pathlib import Path

FLAGS = ["--output-json", "--time", "--no-report-long-running"]


def run(cmd, cwd, env, log):
    log.mkdir(parents=True, exist_ok=True)
    with open(log / "stdout.txt", "wb") as o, open(log / "stderr.txt", "wb") as e:
        p = subprocess.run(cmd, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=o, stderr=e,
                           check=False)
    (log / "exit.txt").write_text(f"{p.returncode}\n")
    (log / "command.json").write_text(json.dumps({"cmd": [str(c) for c in cmd], "cwd": str(cwd)}))
    return p.returncode


def main():
    ap = argparse.ArgumentParser()
    for a in ("sail", "shim", "z3", "engine", "out"):
        ap.add_argument(f"--{a}", required=True, type=Path)
    ap.add_argument("--runs", type=int, default=5)
    ap.add_argument("--only", default="", help="comma-separated case names")
    a = ap.parse_args()
    out = a.out.resolve()
    sail, shim, z3, engine = (p.resolve() for p in (a.sail, a.shim, a.z3, a.engine))
    only = set(filter(None, a.only.split(",")))
    cases = [c for c in sorted(Path("spikes/v030-mutev/minimal").glob("*.rs"))
             if not only or c.stem in only]
    base = {k: v for k, v in os.environ.items()
            if not k.startswith(("SAIL_", "VERUS_")) and k not in ("RUSTC_WRAPPER", "CARGO_TARGET_DIR")}
    base.update(CARGO_TERM_COLOR="never", CARGO_TERM_QUIET="true")
    toolchain = subprocess.run(
        ["python3", "ci/stock-toolchain.py", str(sail)], check=True, capture_output=True, text=True,
        env={**base, "SAIL_VERUS_BUILD": str(engine)}).stdout.strip()
    (out / "proj").mkdir(parents=True, exist_ok=True)
    (out / "identity.json").write_text(json.dumps({"stock_toolchain": toolchain}))
    stock_env = {**base, "RUSTUP_TOOLCHAIN": toolchain, "CARGO_TARGET_DIR": str(out / "target-stock")}
    for case in cases:
        name = case.stem
        proj = out / "proj" / name
        if not proj.exists():
            subprocess.run(["cargo", "verus", "new", "--lib", name], cwd=out / "proj", env=stock_env,
                           check=True, capture_output=True)
        shutil.copyfile(case, proj / "src/lib.rs")
        logs = out / "logs" / name
        verify = ["cargo", "verus", "verify", "--", *FLAGS]
        for k in range(a.runs):
            subprocess.run(["cargo", "clean", "-p", name], cwd=proj, env=stock_env, check=True,
                           capture_output=True)
            log = logs / f"stock-traced-{k}"
            (log / "trace").mkdir(parents=True, exist_ok=True)
            code = run(verify, proj, {**stock_env, "VERUS_Z3_PATH": str(shim), "SAIL_Z3_PATH": str(z3),
                                      "SAIL_TRACE_DIR": str(log / "trace")}, log)
            print(name, f"stock-traced-{k}", code, flush=True)
        subprocess.run(["cargo", "clean", "-p", name], cwd=proj, env=stock_env, check=True,
                       capture_output=True)
        print(name, "stock-plain", run(verify, proj, stock_env, logs / "stock-plain"), flush=True)
        for k in range(a.runs):
            cache = out / "cache" / f"{name}-{k}"
            env = {**base, "SAIL_CACHE_DIR": str(cache), "SAIL_VERUS_BUILD": str(engine),
                   "CARGO_TARGET_DIR": str(out / "target-sail")}
            subprocess.run(["cargo", "clean", "-p", name], cwd=proj, check=False, capture_output=True,
                           env={**stock_env, "CARGO_TARGET_DIR": str(out / "target-sail")})
            code = run([sail, "verify", "--output", "json"], proj, env, logs / f"sail-{k}")
            status = subprocess.run([sail, "status", "--json"], env=env, capture_output=True, text=True,
                                    check=False).stdout
            try:
                pid = json.loads(status).get("info", {}).get("pid")
            except ValueError:
                pid = None
            if pid:
                subprocess.run(["kill", str(pid)], check=False)
            subprocess.run(["pkill", "-f", "--", str(cache)], check=False)
            print(name, f"sail-{k}", code, flush=True)


main()
