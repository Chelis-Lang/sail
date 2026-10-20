#!/usr/bin/env python3
"""Local acceptance of the dependency store (ADR 0045) against plain stock Verus.

    python3 scripts/dependency-store-check.py --sail SAIL --stock STOCK_CARGO_VERUS --out DIR

SAIL is the `sail` to test (its Verus selected as usual, for example by
SAIL_VERUS_BUILD); STOCK_CARGO_VERUS is the pinned stock `cargo-verus`, the
oracle. Every project is new, made as README's First verification says, with
a proof that fails a vstd precondition and one failing postcondition, so that
diagnostics point into vstd's source. Each case compares Sail's first managed
run with plain stock `cargo verus verify` on the same project in a new target:
exit code, stderr without Cargo's progress lines (and without Sail's one
SAIL3131 warning), and the `verification results::` lines plain prints (vstd's,
then the crate's), rendered from Sail's reports.

1. donor: with an empty store, Sail verifies vstd and stores it.
2. seeded: a second project's first run takes vstd from the store (one
   verifier run).
3. twin1, twin2: two more projects at once, both seeded.
4. mismatch: vstd with other features is another key: verified, no warning.
5. corrupt: a damaged entry prints warning[SAIL3131] and vstd is verified.

Timings are printed with the load average. Exit status 0 means every check
held. This is a local check (ADR 0013); CI runs the cheaper seeding checks in
ci/test-managed.py.
"""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time
import tomllib

PINS = Path(__file__).resolve().parent.parent / "corpus/toolchain.toml"
VSTD = "=" + tomllib.loads(PINS.read_text())["verus"]["vstd"]
SOURCE = """use vstd::prelude::*;

verus! {

pub fn first(v: &Vec<u64>) -> u64 {
    v[0]
}

pub fn max(a: u32, b: u32) -> (r: u32)
    ensures r >= a, r >= b, r == a || r == b,
{ if a > b { a } else { b } }

pub fn bad(x: u32) -> (r: u32)
    ensures r > x,
{ x }

} // verus!
"""
PROGRESS = re.compile(
    rb"^ *(Compiling|Checking|Finished|Blocking|Locking|Downloaded|Downloading|Updating|Adding) ")


def load():
    return " ".join("%.1f" % x for x in os.getloadavg())


def project(work, name, dependency, lock):
    path = work / name
    subprocess.run(["cargo", "new", "--lib", "--vcs", "none", "-q", name], cwd=work, check=True)
    with open(path / "Cargo.toml", "a") as manifest:
        manifest.write(f"vstd = {dependency}\n\n[package.metadata.verus]\nverify = true\n")
    if lock is not None:
        # The donor's resolution, as for another worktree of one project.
        (path / "Cargo.lock").write_text(lock.replace('name = "donor"', f'name = "{name}"'))
    (path / "src/lib.rs").write_text(SOURCE)
    return path


def run(arguments, cwd, env):
    started = time.monotonic()
    done = subprocess.run(arguments, cwd=cwd, env=env, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, timeout=1800)
    return done, time.monotonic() - started


def managed(sail, path, env):
    return run([sail, "verify", "--incremental", "--output", "json"], path, env)


def comparable(stderr):
    return b"".join(line for line in stderr.splitlines(keepends=True)
                    if not PROGRESS.match(line) and not line.startswith(b"warning[SAIL3131]"))


def results(document):
    """The `verification results::` lines plain prints, from Sail's reports:
    vstd's first (Cargo finishes it before the crate), then the crate's."""
    lines = []
    for requested in (False, True):
        for target in document["targets"]:
            if target["requested"] == requested:
                verus = target["report"]["crates"][0]["verus"]
                lines.append("verification results:: %d verified, %d errors"
                             % (verus["verified"], verus["errors"]))
    return lines


def compare(label, sail_run, stock, path, env, out):
    done, seconds = sail_run
    document = json.loads(done.stdout)
    target = path.parent / (path.name + "-plain-target")
    plain, plain_seconds = run([stock, "verify"], path, dict(env, CARGO_TARGET_DIR=str(target)))
    plain_lines = [line for line in plain.stdout.decode().splitlines()
                   if line.startswith("verification results::")]
    vstd = [t for t in document["targets"] if t["target"] == "vstd"]
    row = {
        "case": label,
        "load": load(),
        "sail_seconds": round(seconds, 2),
        "plain_seconds": round(plain_seconds, 2),
        "verifier_invocations": document["execution"]["verifier_invocations"],
        "vstd_from_store": bool(vstd and vstd[0].get("dependency_store")),
        "currentness": document["currentness"],
        "warned": b"warning[SAIL3131]" in done.stderr,
        "exit_identical": done.returncode == plain.returncode,
        "results_identical": results(document) == plain_lines,
        "stderr_identical": comparable(done.stderr) == comparable(plain.stderr),
    }
    for name, data in (("sail.stdout.json", done.stdout), ("sail.stderr", done.stderr),
                       ("plain.stdout", plain.stdout), ("plain.stderr", plain.stderr)):
        (out / f"{label}.{name}").write_bytes(data)
    print(json.dumps(row), flush=True)
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--sail", required=True)
    parser.add_argument("--stock", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    sail = str(Path(args.sail).resolve())
    stock = str(Path(args.stock).resolve())
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    failures = []
    rows = []
    with tempfile.TemporaryDirectory(prefix="sail-dependency-store-") as temporary:
        work = Path(temporary)
        env = dict(os.environ, SAIL_CACHE_DIR=str(work / "cache"))
        env.pop("CARGO_TARGET_DIR", None)
        dependency = f'"{VSTD}"'

        donor = project(work, "donor", dependency, None)
        rows.append(compare("donor", managed(sail, donor, env), stock, donor, env, out))
        if rows[-1]["vstd_from_store"]:
            failures.append("donor: an empty store supplied vstd")
        store = work / "cache/dependency-store/v1"
        entries = [p for p in store.iterdir() if not p.name.startswith(".")] \
            if store.is_dir() else []
        if len(entries) != 1:
            failures.append(f"donor: expected one store entry, found {len(entries)}")
        lock = (donor / "Cargo.lock").read_text()

        seeded = project(work, "seeded", dependency, lock)
        rows.append(compare("seeded", managed(sail, seeded, env), stock, seeded, env, out))
        if not (rows[-1]["vstd_from_store"] and rows[-1]["verifier_invocations"] == 1):
            failures.append("seeded: vstd was not taken from the store")

        twins = [project(work, f"twin{i}", dependency, lock) for i in (1, 2)]
        started = time.monotonic()
        processes = [subprocess.Popen([sail, "verify", "--incremental", "--output", "json"],
                                      cwd=twin, env=env, stdout=subprocess.PIPE,
                                      stderr=subprocess.PIPE) for twin in twins]
        for twin, process in zip(twins, processes):
            stdout, stderr = process.communicate(timeout=1800)
            document = json.loads(stdout)
            vstd = [t for t in document["targets"] if t["target"] == "vstd"]
            ok = bool(vstd and vstd[0].get("dependency_store")) and b"SAIL3131" not in stderr
            print(json.dumps({"case": twin.name, "load": load(),
                              "sail_seconds": round(time.monotonic() - started, 2),
                              "vstd_from_store": ok}), flush=True)
            if not ok:
                failures.append(f"{twin.name}: concurrent seeding failed")

        other = project(work, "mismatch",
                        f'{{ version = "{VSTD}", default-features = false, features = ["alloc"] }}',
                        None)
        rows.append(compare("mismatch", managed(sail, other, env), stock, other, env, out))
        if rows[-1]["vstd_from_store"] or rows[-1]["warned"]:
            failures.append("mismatch: another key was served, or a warning printed")

        damaged = sorted((entries[0] / "files").rglob("*.rmeta"))[:1] if entries else []
        for stored in damaged:
            stored.write_bytes(stored.read_bytes() + b"corrupt")
        corrupt = project(work, "corrupt", dependency, lock)
        rows.append(compare("corrupt", managed(sail, corrupt, env), stock, corrupt, env, out))
        if not damaged or rows[-1]["vstd_from_store"] or not rows[-1]["warned"]:
            failures.append("corrupt: the damaged entry was used or not reported")
    for row in rows:
        for check in ("exit_identical", "results_identical", "stderr_identical"):
            if not row[check]:
                failures.append(f"{row['case']}: {check} is false")
    for failure in failures:
        print("FAIL", failure, file=sys.stderr)
    print("dependency store check:", "passed" if not failures else f"{len(failures)} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
