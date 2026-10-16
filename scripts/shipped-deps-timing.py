#!/usr/bin/env python3
"""First-run timings of a new project on a fresh machine (ADR 0048).

    python3 scripts/shipped-deps-timing.py --release DIR --stock STOCK_CARGO_VERUS [--rounds N]

DIR is a staged release tree with its shipped vstd build. Each round makes a
new `sail new` project (one verified function) under a new Cargo home and an
empty Sail cache, fetches its dependencies (untimed: the download is the same
for every arm), and times:

- shipped: Sail's first `sail verify`, with the release's shipped build;
- unshipped: the same without it (`SAIL_SHIPPED_DEPS` names an empty
  directory), what a fresh machine paid before;
- plain: plain stock `cargo verus verify` in a new target;
- rerun: Sail's second `sail verify` of the shipped project.

Prints one JSON line per round and the medians, with the load average.
"""

import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time


def timed(arguments, cwd, env):
    started = time.monotonic()
    done = subprocess.run(arguments, cwd=cwd, env=env, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, timeout=1800)
    if done.returncode != 0:
        raise RuntimeError(f"{arguments}: {done.stderr.decode()[-2000:]}")
    return round(time.monotonic() - started, 2)


def fresh(work, name, sail, extra=None):
    home = work / name / "cargo-home"
    home.mkdir(parents=True)
    env = dict(os.environ, CARGO_HOME=str(home), SAIL_CACHE_DIR=str(work / name / "cache"),
               SAIL_DEPENDENCY_STORE_ENTRIES="0")
    for variable in ("CARGO_TARGET_DIR", "SAIL_VERUS_BUILD", "SAIL_SHIPPED_DEPS",
                     "SAIL_SHIPPED_RECORD"):
        env.pop(variable, None)
    env.update(extra or {})
    subprocess.run([sail, "new", "--lib", "demo"], cwd=work / name, env=env, check=True,
                   stdout=subprocess.DEVNULL)
    project = work / name / "demo"
    subprocess.run(["cargo", "fetch", "--quiet"], cwd=project, env=env, check=True)
    return project, env


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--release", required=True)
    parser.add_argument("--stock", required=True)
    parser.add_argument("--rounds", type=int, default=3)
    args = parser.parse_args()
    sail = str(Path(args.release).resolve() / "bin/sail")
    stock = str(Path(args.stock).resolve())
    rows = []
    for round_ in range(args.rounds):
        with tempfile.TemporaryDirectory(prefix="sail-shipped-timing-") as temporary:
            work = Path(temporary)
            row = {"round": round_ + 1}
            project, env = fresh(work, "shipped", sail)
            row["shipped"] = timed([sail, "verify"], project, env)
            row["rerun"] = timed([sail, "verify"], project, env)
            (work / "empty").mkdir()
            project, env = fresh(work, "unshipped", sail, {"SAIL_SHIPPED_DEPS": str(work / "empty")})
            row["unshipped"] = timed([sail, "verify"], project, env)
            project, env = fresh(work, "plain", sail)
            row["plain"] = timed([stock, "verify"], project,
                                 dict(env, CARGO_TARGET_DIR=str(work / "plain-target")))
            row["load"] = " ".join("%.1f" % x for x in os.getloadavg())
            print(json.dumps(row), flush=True)
            rows.append(row)
    print(json.dumps({key: statistics.median(r[key] for r in rows)
                      for key in ("shipped", "rerun", "unshipped", "plain")}))


if __name__ == "__main__":
    main()
