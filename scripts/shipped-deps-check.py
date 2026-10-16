#!/usr/bin/env python3
"""Local acceptance of the shipped vstd build (ADR 0048) against plain stock Verus.

    python3 scripts/shipped-deps-check.py --release DIR --stock STOCK_CARGO_VERUS --out OUT

DIR is a staged release tree (`scripts/release.sh stage`: bin/sail, engine/
and deps/); STOCK_CARGO_VERUS is the pinned stock `cargo-verus`, the oracle.
Each case starts as a fresh machine would: an empty Sail cache and its own
new Cargo home (each at a different path, so vstd's sources lie elsewhere
than when the release recorded them). Projects are made with `sail new` and
hold a proof that fails a vstd precondition and one failing postcondition,
so diagnostics point into vstd's source. Each case compares Sail's first run
(`sail verify`, as a user runs it) with plain stock `cargo verus verify` on
the same project, under the same Cargo home, in a new target: exit code,
stdout and stderr without Cargo's progress lines (and without Sail's one
SAIL3133 warning).

1. new: a new library project; vstd must be served (the run's `shipped`).
2. other-home: the same under a Cargo home with a longer path.
3. other-cargo: another default Rust toolchain (`RUSTUP_TOOLCHAIN`);
   a different Cargo argument set may safely miss, but output must match.
4. package-dir: a package of a two-member workspace, run from the package's
   directory.
5. rerun: case 1 again; vstd is reused and no verifier runs for it.
6. mismatch: vstd with other features is another key: verified, no warning.
7. corrupt: a copy of the release whose stored vstd metadata is damaged
   prints warning[SAIL3133] and verifies vstd.

Timings are printed with the load average. Exit status 0 means every check
held. This is a local check (ADR 0013).
"""

import argparse
import glob
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time

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


def run(arguments, cwd, env):
    started = time.monotonic()
    done = subprocess.run(arguments, cwd=cwd, env=env, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, timeout=1800)
    return done, time.monotonic() - started


def comparable(stderr):
    return b"".join(line for line in stderr.splitlines(keepends=True)
                    if not PROGRESS.match(line) and not line.startswith(b"warning[SAIL3133]"))


def environment(work, name, release):
    home = work / name / ("cargo-home" if name != "other-home" else
                          "a-considerably-longer-cargo-home-directory-name/nested")
    env = dict(os.environ, CARGO_HOME=str(home), SAIL_CACHE_DIR=str(work / name / "cache"),
               SAIL_DEPENDENCY_STORE_ENTRIES="0")
    # Apple may set SDKROOT when launching Python. Test the default user
    # environment explicitly; SDK/linker overrides are bound and may miss.
    for variable in ("CARGO_TARGET_DIR", "SAIL_VERUS_BUILD", "SAIL_SHIPPED_DEPS",
                     "SAIL_SHIPPED_RECORD", "SDKROOT", "MACOSX_DEPLOYMENT_TARGET"):
        env.pop(variable, None)
    home.mkdir(parents=True, exist_ok=True)
    return env


def new_project(sail, parent, name, env, manifest_extra=None):
    done = subprocess.run([sail, "new", "--lib", name], cwd=parent, env=env,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if done.returncode != 0:
        raise RuntimeError(f"sail new {name}: {done.stderr.decode()}")
    path = parent / name
    (path / "src/lib.rs").write_text(SOURCE)
    if manifest_extra:
        manifest = (path / "Cargo.toml").read_text()
        manifest = re.sub(r'^vstd = .*$', manifest_extra, manifest, flags=re.M)
        (path / "Cargo.toml").write_text(manifest)
        (path / "Cargo.lock").unlink(missing_ok=True)
    return path


def results(cache):
    """The newest run result in a Sail cache."""
    files = sorted(glob.glob(str(cache / "projects/*/*/runs/*/result.json")),
                   key=os.path.getmtime)
    return json.loads(Path(files[-1]).read_text()) if files else {}


def compare(label, sail, stock, path, env, out, expect_shipped, expect_warning=False):
    done, seconds = run([sail, "verify"], path, env)
    document = results(Path(env["SAIL_CACHE_DIR"]))
    (out / f"{label}.project-result.json").write_text(json.dumps(document, indent=2) + "\n")
    target = path.parent / (path.name + "-plain-target")
    plain, plain_seconds = run([stock, "verify"], path, dict(env, CARGO_TARGET_DIR=str(target)))
    vstd = [t for t in document.get("targets", []) if t["target"] == "vstd"]
    row = {
        "case": label,
        "load": load(),
        "sail_seconds": round(seconds, 2),
        "plain_seconds": round(plain_seconds, 2),
        "verifier_invocations": document.get("execution", {}).get("verifier_invocations"),
        "vstd_shipped": bool(vstd and vstd[0].get("shipped")),
        "vstd_reused": bool(vstd and vstd[0].get("reused")),
        "vstd_establishing_run": vstd[0].get("establishing_run") if vstd else None,
        "dependency_store": vstd[0].get("dependency_store") if vstd else None,
        "shipped_origin_valid": not (vstd and vstd[0].get("shipped")) or
            vstd[0].get("establishing_run") == "shipped:" + vstd[0]["shipped"],
        "warned": b"warning[SAIL3133]" in done.stderr,
        "exit_identical": done.returncode == plain.returncode,
        "stdout_identical": done.stdout == plain.stdout,
        "stderr_identical": comparable(done.stderr) == comparable(plain.stderr),
        "vstd_diagnostic": str(Path(env["CARGO_HOME"]) / "registry/src").encode() in done.stderr,
    }
    for name, data in (("sail.stdout", done.stdout), ("sail.stderr", done.stderr),
                       ("plain.stdout", plain.stdout), ("plain.stderr", plain.stderr)):
        (out / f"{label}.{name}").write_bytes(data)
    print(json.dumps(row), flush=True)
    failures = []
    for check in ("exit_identical", "stdout_identical", "stderr_identical", "vstd_diagnostic",
                  "shipped_origin_valid"):
        if not row[check]:
            failures.append(f"{label}: {check} is false")
    if expect_shipped is not None and row["vstd_shipped"] != expect_shipped:
        failures.append(f"{label}: vstd_shipped is {row['vstd_shipped']}")
    if row["warned"] != expect_warning:
        failures.append(f"{label}: warned is {row['warned']}")
    return row, failures


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--release", required=True)
    parser.add_argument("--stock", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    release = Path(args.release).resolve()
    sail = str(release / "bin/sail")
    stock = str(Path(args.stock).resolve())
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    failures = []
    with tempfile.TemporaryDirectory(prefix="sail-shipped-check-") as temporary:
        work = Path(temporary)

        env = environment(work, "new", release)
        project = new_project(sail, work / "new", "proj", env)
        if not (project / "Cargo.lock").is_file():
            failures.append("new: sail new wrote no Cargo.lock from the shipped build")
        failures += compare("new", sail, stock, project, env, out, True)[1]

        env2 = environment(work, "other-home", release)
        other = new_project(sail, work / "other-home", "proj", env2)
        failures += compare("other-home", sail, stock, other, env2, out, True)[1]

        # The launcher fixes the compiler toolchain, but Cargo may emit
        # different flags for the same package. Keep those flags keyed and
        # require diagnostic identity whether this version hits or misses.
        another_env = dict(environment(work, "other-cargo", release),
                           RUSTUP_TOOLCHAIN="1.98.1")
        another = new_project(sail, work / "other-cargo", "proj", another_env)
        failures += compare("other-cargo", sail, stock, another, another_env, out, None)[1]

        env3 = environment(work, "package-dir", release)
        workspace = work / "package-dir" / "ws"
        workspace.mkdir(parents=True)
        package = new_project(sail, workspace, "pkg", env3)
        shutil.move(str(package / "Cargo.lock"), str(workspace / "Cargo.lock"))
        new_project(sail, workspace, "other", env3).joinpath("Cargo.lock").unlink()
        (workspace / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["pkg", "other"]\nresolver = "2"\n')
        failures += compare("package-dir", sail, stock, package, env3, out, True)[1]

        row, more = compare("rerun", sail, stock, project, env, out, True)
        failures += more
        if not row["vstd_reused"] or row["verifier_invocations"] != 1:
            failures.append("rerun: vstd was not reused, or a verifier ran for it")

        # A shipped proof keeps its nonlocal origin when copied through the
        # machine-local dependency store into another project's new target.
        store_env = dict(environment(work, "store", release), SAIL_DEPENDENCY_STORE_ENTRIES="4")
        store_rows = []
        for label in ("store-first", "store-copy"):
            parent = work / "store" / label
            parent.mkdir(parents=True)
            stored_project = new_project(sail, parent, "proj", store_env)
            store_row, more = compare(label, sail, stock, stored_project, store_env, out, True)
            failures += more
            store_rows.append(store_row)
        if not store_rows[1]["dependency_store"] or not store_rows[1]["vstd_reused"]:
            failures.append("store-copy: vstd was not reused from the dependency store")
        if store_rows[0]["vstd_establishing_run"] != store_rows[1]["vstd_establishing_run"]:
            failures.append("store-copy: shipped proof origin changed")

        env5 = environment(work, "mismatch", release)
        mismatch = new_project(
            sail, work / "mismatch", "proj", env5,
            'vstd = { version = "=0.0.0-2026-10-04-0306", default-features = false, '
            'features = ["alloc"] }')
        failures += compare("mismatch", sail, stock, mismatch, env5, out, False)[1]

        damaged = work / "damaged-release"
        shutil.copytree(release, damaged, symlinks=True)
        metadata = sorted(damaged.glob("deps/*/units/*/files/libvstd-*.rmeta"))
        for stored in metadata:
            stored.write_bytes(stored.read_bytes() + b"corrupt")
        env6 = environment(work, "corrupt", release)
        corrupt = new_project(sail, work / "corrupt", "proj", env6)
        if not metadata:
            failures.append("corrupt: no stored vstd metadata to damage")
        failures += compare("corrupt", str(damaged / "bin/sail"), stock, corrupt, env6, out,
                            False, expect_warning=True)[1]
    for failure in failures:
        print("FAIL", failure, file=sys.stderr)
    print("shipped dependency check:", "passed" if not failures else f"{len(failures)} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
