#!/usr/bin/env python3
"""Build the frozen measurement-only observer in a new owned native checkout.

No host provisioning, Sail runtime manifest/pin changes or publication. Uses
the caller's dedicated toolchain/download volumes. Every failure is retained.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import time

SOURCE = "80a19b1aef80e2344baa2101fe7077b45aebba2c"
BASE = "8a373f81a507b7ef4f144767ccd237d3d3de6c48"
BASE_BUNDLE = "62e4a9453c73dc00c2a6147396d0a1d3a2d8b2313d77d672ea62a9e6fa629e6d"
OBSERVER_BUNDLE = "17626ea5e625206d12ced9c3b7b14dcbee7e3ff1e14ab7044bc81a36fe2d64ff"
LOCK = "80c71fdd00ef5073cd93edf6b5e37a635f7b4114fba8f6553ba2125eecb1395e"
Z3 = "e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765"
RUNTIME_FILES = ["verus", "rust_verify", "cargo-verus", "z3", "verus-root",
                 "libverus_builtin.rlib", "libverus_builtin_macros.so",
                 "libverus_state_machines_macros.so", "libvstd.rlib", "vstd.vir"]
RUNTIME_DIRS = ["builtin", "builtin_macros", "state_machines_macros", "vstd"]


def sha(path):
    result = hashlib.sha256()
    with path.open("rb") as source:
        while block := source.read(65536):
            result.update(block)
    return result.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["base-bundle", "observer-bundle", "z3", "work"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--install-toolchain", action="store_true")
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("native x86-64 Linux only")
    for path, expected in [(args.base_bundle, BASE_BUNDLE),
                           (args.observer_bundle, OBSERVER_BUNDLE), (args.z3, Z3)]:
        if sha(path) != expected:
            parser.error("immutable input digest mismatch: " + str(path))
    args.base_bundle = args.base_bundle.resolve(strict=True)
    args.observer_bundle = args.observer_bundle.resolve(strict=True)
    args.z3 = args.z3.resolve(strict=True)
    work = args.work.absolute()
    work.mkdir(parents=True, exist_ok=False)
    out, checkout = work / "build-results", work / "verus"
    out.mkdir()
    manifest = {"source": SOURCE, "base": BASE, "source_lock_sha256": LOCK,
                "base_bundle_sha256": BASE_BUNDLE, "observer_bundle_sha256": OBSERVER_BUNDLE,
                "solver_sha256": Z3, "platform": platform.platform(),
                "scope": "direct compiler research runtime; no Sail runtime/corpus qualification",
                "complete": False, "candidate_gate": False, "steps": [], "errors": []}
    env = dict(os.environ, RUSTUP_TOOLCHAIN="1.98.1", CARGO_BUILD_JOBS="8", CARGO_INCREMENTAL="0",
               VERUS_Z3_PATH=str(args.z3))
    for key in list(env):
        if key.startswith(("SAIL_", "__VERUS_")) or key in [
                "CARGO", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_BUILD_DIR",
                "RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_BOOTSTRAP", "VERUS_IN_VARGO",
                "VARGO_TARGET_DIR", "LD_PRELOAD", "DYLD_INSERT_LIBRARIES"]:
            env.pop(key, None)

    def save():
        temporary = out / "manifest.tmp"
        temporary.write_text(json.dumps(manifest, indent=2) + "\n")
        temporary.replace(out / "manifest.json")

    def run(label, command, cwd, timeout=120, environment=None):
        actual_env = environment if environment is not None else env
        row = {"step": label, "command": list(map(str, command)), "cwd": str(cwd),
               "timeout_seconds": timeout,
               "environment_sha256": hashlib.sha256(json.dumps(sorted(actual_env.items())).encode()).hexdigest()}
        manifest["steps"].append(row)
        save()
        start = time.monotonic()
        with (out / (label + ".log")).open("wb") as log:
            child = subprocess.Popen(command, cwd=cwd, env=actual_env, stdout=log,
                                     stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
                                     start_new_session=True)
            try:
                child.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                row["timeout"] = True
            finally:
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGKILL)
                row["exit"] = child.wait(timeout=10)
                row["wall_seconds"] = time.monotonic() - start
                row["log_sha256"] = sha(out / (label + ".log"))
                save()
        if row["exit"] != 0:
            raise ValueError(label + " failed; retained log")

    def interrupted(number, _frame):
        raise KeyboardInterrupt("signal " + str(number))

    signal.signal(signal.SIGTERM, interrupted)
    try:
        run("clone", ["git", "clone", "--no-checkout", str(args.base_bundle), str(checkout)], work)
        run("fetch-observer", ["git", "fetch", str(args.observer_bundle),
                               "refs/heads/m11-8/raw-key-evidence"], checkout)
        run("checkout", ["git", "checkout", "--detach", SOURCE], checkout)
        if sha(checkout / "source/Cargo.lock") != LOCK:
            raise ValueError("locked source dependencies differ")
        run("source-before", ["git", "status", "--porcelain"], checkout)
        if subprocess.check_output(["git", "status", "--porcelain"], cwd=checkout):
            raise ValueError("unexpected source changes before build")
        if args.install_toolchain:
            run("toolchain-install", ["rustup", "toolchain", "install", "1.98.1", "--profile", "minimal",
                                      "--component", "rustc-dev,llvm-tools,rustfmt",
                                      "--target", "x86_64-unknown-none"], checkout, 1200)
        run("rustc-identity", ["rustc", "-Vv"], checkout)
        run("toolchain-components", ["rustup", "component", "list", "--installed",
                                     "--toolchain", "1.98.1"], checkout)
        run("vargo-build", ["cargo", "build", "--release", "--locked"], checkout / "tools/vargo", 900)
        source = checkout / "source"
        vargo = checkout / "tools/vargo/target/release/vargo"
        manifest["vargo_sha256"] = sha(vargo)
        run("toolchain-manifest", ["cargo", "run", "--locked", "-p", "cargo-verus-toolchains",
                                    "--bin", "create-manifest", "--", "--write-to-dir",
                                    "cargo-verus/toolchain-manifests"], source, 900)
        run("runtime-build", [str(vargo), "build", "--release"], source, 2100)
        runtime = source / "target-verus/release"
        run("release-vstd", [str(runtime / "cargo-verus"), "build", "--release",
                              "--manifest-path", "vstd/Cargo.toml"], source, 600)
        # vargo test accepts only air/rust_verify_test. Use its exact documented
        # compiler cfgs for the two small observer unit tests after the full build.
        test_env = dict(env, RUSTC_BOOTSTRAP="1", VERUS_IN_VARGO="1",
                        VARGO_TARGET_DIR=str(runtime), RUST_MIN_STACK=str(20 * 1024 * 1024),
                        RUSTFLAGS="--cfg proc_macro_span --cfg verus_keep_ghost --cfg span_locations --cfg procmacro2_semver_exempt")
        run("observer-unit-tests", ["cargo", "test", "--release", "--locked", "-p", "rust_verify",
                                     "--lib", "sail_m8_evidence::tests", "--", "--nocapture"],
            source, 600, test_env)
        destination = out / "runtime"
        destination.mkdir()
        for name in RUNTIME_FILES:
            shutil.copy2(runtime / name, destination / name)
        for name in RUNTIME_DIRS:
            shutil.copytree(runtime / name, destination / name)
        if (runtime / ".vstd-fingerprint").is_file():
            shutil.copy2(runtime / ".vstd-fingerprint", destination / ".vstd-fingerprint")
        if sha(destination / "z3") != Z3:
            raise ValueError("built runtime solver differs")
        manifest["runtime_sha256"] = {str(p.relative_to(destination)): sha(p)
                                      for p in sorted(destination.rglob("*")) if p.is_file()}
        (out / "runtime-files.json").write_text(json.dumps(manifest["runtime_sha256"], indent=2) + "\n")
        manifest["complete"] = True
    except BaseException as error:
        manifest["errors"].append(type(error).__name__ + ": " + str(error))
    finally:
        try:
            if checkout.is_dir():
                head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout).decode().strip()
                manifest["source_head_after"] = head
                if head != SOURCE:
                    raise ValueError("source HEAD moved during build")
                status = subprocess.check_output(["git", "status", "--porcelain",
                                                   "--untracked-files=no"], cwd=checkout).decode()
                untracked = subprocess.check_output(["git", "ls-files", "--others", "--exclude-standard"],
                                                    cwd=checkout).decode().splitlines()
                # create-manifest produces a version-specific, untracked build
                # input. Preserve its exact bytes/identity; no other new source.
                if any(not p.startswith("source/cargo-verus/toolchain-manifests/")
                       or not p.endswith(".toml") for p in untracked):
                    raise ValueError("unexpected untracked build input")
                manifest["generated_toolchain_manifests"] = {p: sha(checkout / p) for p in untracked}
                manifest["source_status_after"] = status
                if status or sha(checkout / "source/Cargo.lock") != LOCK:
                    raise ValueError("source/lock changed during build")
            if sha(args.z3) != Z3 or sha(args.base_bundle) != BASE_BUNDLE or sha(args.observer_bundle) != OBSERVER_BUNDLE:
                raise ValueError("immutable build input changed")
        except BaseException as error:
            manifest["errors"].append("final identity: " + str(error))
            manifest["complete"] = False
        save()
    return 0 if manifest["complete"] and not manifest["errors"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
