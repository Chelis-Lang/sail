#!/usr/bin/env python3
"""Native Linux control of a private, genuine stock source-suite build (#202).

Requires Python 3.11+, a matching built conformance binary and xtask binary,
and an initial `build-verus-source --keep-intermediates` build. This deliberately
mutates one named test executable and its manifest, then rebuilds the stock
suite. Never point it at a shared build or a fork/engine runtime. Raw results,
including refusals and interrupted attempts, remain under --out.
"""

import argparse
import copy
import hashlib
import gzip
import json
import os
from pathlib import Path
import platform
import signal
import shutil
import subprocess
import time
import tomllib


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def checked_path(build, name):
    relative = Path(name)
    if not name or relative.is_absolute() or any(p in (".", "..") for p in name.split("/")):
        raise ValueError("unsafe source-manifest path")
    path = build / relative
    if not path.resolve(strict=True).is_relative_to(build):
        raise ValueError("source-manifest path escapes owned build")
    if not path.is_file() or path.is_symlink():
        raise ValueError("source-manifest entry is not an ordinary file")
    return path


def inventory(build, pins):
    manifest_file = build / "sail-build.json"
    manifest = json.loads(manifest_file.read_text())
    if (manifest["schema_version"] != 1 or manifest["features"] != []
            or manifest["commit"] != pins["verus"]["commit"]
            or manifest["rust_toolchain"] != pins["verus"]["rust_toolchain"]
            or manifest["z3_sha256"] != pins["assets"]["x86_64-unknown-linux-gnu"]["z3_sha256"]):
        raise ValueError("control requires a genuine active-pin stock suite, never a fork/engine variant")
    tests = manifest["test_binaries"]
    if (not tests or not manifest["artifacts"] or any(not b["name"] for b in tests)
            or len({b["name"] for b in tests}) != len(tests)
            or len({b["path"] for b in tests}) != len(tests)):
        raise ValueError("incomplete or duplicate source-suite inventory")
    runtime = build / "verus/source/target-verus/release"
    actual_runtime = {str(p.relative_to(build)) for p in runtime.iterdir() if p.is_file()}
    if actual_runtime != set(manifest["artifacts"]):
        raise ValueError("runtime inventory has unbound or missing files")
    hashes = {}
    for name, expected in list(manifest["artifacts"].items()) + [(b["path"], b["sha256"]) for b in tests]:
        actual = digest(checked_path(build, name))
        if actual != expected:
            raise ValueError("source-suite digest mismatch: " + name)
        hashes[name] = actual
    return manifest, dict(manifest_sha256=digest(manifest_file), files=hashes)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--xtask", type=Path, help="built xtask; defaults to --binary's sibling")
    parser.add_argument("--z3", type=Path, help="release Z3; defaults to SAIL_TEST_Z3")
    parser.add_argument("--baseline-stock", action="store_true", help="for a harness with explicit --baseline support")
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise SystemExit("native Linux x86-64 only; never run under local emulation")
    root = Path(__file__).resolve().parent.parent
    pins = tomllib.loads((root / "corpus/toolchain.toml").read_text())
    build, binary = args.build.resolve(strict=True), args.binary.resolve(strict=True)
    xtask = (args.xtask or binary.parent / "xtask").resolve(strict=True)
    z3 = (args.z3 or Path(os.environ["SAIL_TEST_Z3"])).resolve(strict=True)
    if digest(z3) != pins["assets"]["x86_64-unknown-linux-gnu"]["z3_sha256"]:
        raise SystemExit("release Z3 does not match active solver pin")
    if (build.name != pins["verus"]["commit"] or build.parent.name != "verus-src-build"
            or args.out.resolve().is_relative_to(build)):
        raise SystemExit("expected an owned default stock-build path and external evidence directory")
    checkout = build / "verus"
    if checkout.is_symlink() or checkout.resolve(strict=True) != checkout:
        raise SystemExit("redirected checkout refused")
    args.out.mkdir(parents=True, exist_ok=False)
    out = args.out.resolve()
    source_head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip()
    if source_head != pins["verus"]["commit"]:
        raise SystemExit("source checkout does not match stock pin")
    state = dict(scope="genuine stock-suite integrity and rebuild control only", status="running",
                 source_commit=source_head, binary_sha256=digest(binary), xtask_sha256=digest(xtask),
                 solver_sha256=digest(z3), steps=[])
    original_manifest = (build / "sail-build.json").read_bytes()
    (out / "original-manifest.json").write_bytes(original_manifest)
    env = {k: v for k, v in os.environ.items() if not k.startswith("SAIL_")}
    for key in ("CI", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_BUILD_DIR",
                "RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS", "RUSTUP_TOOLCHAIN"):
        env.pop(key, None)
    # SAIL_VERUS_HOME may select the image's read-only pinned release install.
    if "SAIL_VERUS_HOME" in os.environ:
        env["SAIL_VERUS_HOME"] = os.environ["SAIL_VERUS_HOME"]
    env["SAIL_CACHE_DIR"] = str(build.parent.parent)
    env["SAIL_BUILD_JOBS"] = os.environ.get("SAIL_BUILD_JOBS", "8")
    suite_env = env | {"SAIL_VERUS_SOURCE_BUILD": str(build)}
    deadline = time.monotonic() + 4500
    changed = None
    rebuild_started = False
    manifest_changed = False

    def save():
        write_json(out / "result.json", state)

    def run(label, command, child_env, expected_error=None, seconds=120, cwd=root):
        row = dict(name=label, command=[str(x) for x in command], started_unix=time.time(), passed=False)
        state["steps"].append(row)
        save()
        prefix = out / label
        proc = None
        try:
            with prefix.with_suffix(".stdout").open("wb") as stdout, prefix.with_suffix(".stderr").open("wb") as stderr:
                proc = subprocess.Popen(command, cwd=cwd, env=child_env, stdin=subprocess.DEVNULL,
                                        stdout=stdout, stderr=stderr, start_new_session=True)
                try:
                    proc.wait(timeout=min(seconds, max(1, deadline - time.monotonic())))
                finally:
                    if proc.poll() is None:
                        os.killpg(proc.pid, signal.SIGTERM)
                        try:
                            proc.wait(timeout=10)
                        except subprocess.TimeoutExpired:
                            os.killpg(proc.pid, signal.SIGKILL)
                            proc.wait(timeout=10)
                row["exit"] = proc.returncode
            row["stdout_sha256"] = digest(prefix.with_suffix(".stdout"))
            row["stderr_sha256"] = digest(prefix.with_suffix(".stderr"))
            row["seconds"] = time.time() - row["started_unix"]
            if expected_error is None:
                assert row["exit"] == 0, label + " failed"
            else:
                assert row["exit"] != 0 and expected_error in prefix.with_suffix(".stderr").read_text(), label + " failed to refuse"
                assert prefix.with_suffix(".stdout").stat().st_size == 0, "refused suite produced a test list"
            row["passed"] = True
            return prefix
        except BaseException as error:
            row["error"] = repr(error)
            raise
        finally:
            if proc is not None:
                row["exit"] = proc.returncode
            row["seconds"] = time.time() - row["started_unix"]
            for stream in ("stdout", "stderr"):
                log = prefix.with_suffix("." + stream)
                if log.exists():
                    row[stream + "_sha256"] = digest(log)
            save()

    def list_suite(label, error=None):
        command = [str(binary), "rust-verify-test", "--list"]
        if args.baseline_stock:
            command += ["--baseline", "stock"]
        return run(label, command, suite_env, error)

    def assert_runtime(before):
        after = {name: digest(checked_path(build, name)) for name in before}
        assert before == after, "control changed runtime bytes"

    def backup_executable(path, label):
        metadata = path.stat()
        assert 0 < metadata.st_size <= 64 * 1024**2, "selected test backup exceeds 64 MiB raw cap; refusing before mutation"
        target = out / (label + ".gz")
        hasher = hashlib.sha256()
        total = 0
        with path.open("rb") as source, gzip.open(target, "wb", compresslevel=3) as packed:
            while data := source.read(65536):
                total += len(data)
                assert total <= 64 * 1024**2, "selected test grew beyond raw backup cap"
                hasher.update(data)
                packed.write(data)
        assert total == metadata.st_size and digest(path) == hasher.hexdigest(), "selected test changed during backup"
        return dict(path=target.name, archive_sha256=digest(target), raw_sha256=hasher.hexdigest(),
                    raw_bytes=total, mode=metadata.st_mode, device=metadata.st_dev, inode=metadata.st_ino,
                    mtime_ns=metadata.st_mtime_ns, ctime_ns=metadata.st_ctime_ns)

    def mutate(path):
        nonlocal changed
        metadata = path.stat()
        assert metadata.st_size > 0
        with path.open("r+b") as stream:
            original = stream.read(1)
            stream.seek(0)
            stream.write(bytes([original[0] ^ 1]))
            stream.flush()
            os.fsync(stream.fileno())
        os.utime(path, ns=(metadata.st_atime_ns, metadata.st_mtime_ns))
        changed = dict(path=path, byte=original, corrupted=digest(path), stat=metadata)
        return dict(path=str(path.relative_to(build)), original_byte=original.hex(),
                    corrupted_sha256=changed["corrupted"], mtime_ns=metadata.st_mtime_ns,
                    mtime_restored=path.stat().st_mtime_ns == metadata.st_mtime_ns)

    def restore_mutation():
        nonlocal changed
        if changed is not None:
            path = changed["path"]
            if path.is_file() and digest(path) == changed["corrupted"]:
                with path.open("r+b") as stream:
                    stream.write(changed["byte"])
                    stream.flush()
                    os.fsync(stream.fileno())
                metadata = changed["stat"]
                os.utime(path, ns=(metadata.st_atime_ns, metadata.st_mtime_ns))
            changed = None

    def interrupted(number, _frame):
        raise KeyboardInterrupt("signal " + str(number))

    for number in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(number, interrupted)
    try:
        run("stock-source-before", ["git", "diff", "--no-ext-diff", "--exit-code", "HEAD", "--"], env, cwd=checkout)
        manifest, before = inventory(build, pins)
        write_json(out / "before.json", before)
        runtimes = {name: before["files"][name] for name in manifest["artifacts"]}
        first = list_suite("list-before")
        assert first.with_suffix(".stdout").stat().st_size > 0, "genuine suite lists no tests"
        selected = min(manifest["test_binaries"], key=lambda b: checked_path(build, b["path"]).stat().st_size)
        path = checked_path(build, selected["path"])
        assert selected["path"] not in runtimes, "control must alter only a separate test executable"
        selected_stat = path.stat()
        state["selected_test"] = dict(name=selected["name"], path=selected["path"], sha256=selected["sha256"],
                                      bytes=selected_stat.st_size, mode=selected_stat.st_mode,
                                      mtime_ns=selected_stat.st_mtime_ns, ctime_ns=selected_stat.st_ctime_ns)
        save()
        state["selected_test_backup"] = backup_executable(path, "selected-test-original")
        assert state["selected_test_backup"]["raw_sha256"] == before["files"][selected["path"]]
        state["mutation"] = mutate(path)
        assert state["mutation"]["mtime_restored"]
        assert digest(path) != before["files"][selected["path"]]
        assert_runtime(runtimes)
        list_suite("list-corrupt", "error[SAIL3040]")
        restore_mutation()
        inventory(build, pins)
        list_suite("list-restored")

        legacy = copy.deepcopy(manifest)
        for entry in legacy["test_binaries"]:
            entry.pop("sha256")
        (build / "sail-build.json").write_text(json.dumps(legacy, indent=2) + "\n")
        manifest_changed = True
        state["legacy_mutation"] = mutate(path)
        assert_runtime(runtimes)
        list_suite("list-legacy-corrupt", "error[SAIL3040]")
        (out / "legacy-corrupt-manifest.json").write_bytes((build / "sail-build.json").read_bytes())

        vargo = checkout / "tools/vargo/target/release/vargo"
        assert vargo.is_file(), "initial source build must retain intermediates"
        vargo_env = env | {"RUSTUP_TOOLCHAIN": manifest["rust_toolchain"], "VERUS_Z3_PATH": str(z3),
                           "CARGO_BUILD_JOBS": env["SAIL_BUILD_JOBS"], "CARGO_TERM_PROGRESS_WHEN": "never",
                           "PATH": str(vargo.parent) + os.pathsep + env["PATH"]}
        observation = run("cargo-freshness-before-reset", [str(vargo), "test", "-p", "rust_verify_test", "--release",
                          "--no-run", "--message-format=json"], vargo_env, seconds=1500, cwd=checkout / "source")
        matching = []
        for line in observation.with_suffix(".stdout").read_text().splitlines():
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue
            if message.get("reason") == "compiler-artifact" and message.get("executable") == str(path):
                matching.append(message)
        still_corrupt = path.is_file() and digest(path) == changed["corrupted"]
        state["cargo_freshness_observation"] = dict(matching_artifacts=matching,
            altered_bytes_remained=still_corrupt,
            confirmed_fresh_corrupt=still_corrupt and any(m.get("fresh") is True for m in matching))
        write_json(out / "cargo-freshness-observation.json", state["cargo_freshness_observation"])
        restore_mutation()
        # The observation may recompile; establish another known corrupt test
        # immediately before the forced rebuild, without trusting its output.
        if digest(path) != state["selected_test_backup"]["raw_sha256"]:
            state["pre_rebuild_test_backup"] = backup_executable(path, "selected-test-after-no-run")
        state["pre_rebuild_mutation"] = mutate(path)
        list_suite("list-legacy-pre-rebuild", "error[SAIL3040]")
        markers = []
        for output in ("target", "target-verus"):
            marker = checkout / "source" / output / "release/sail-integrity-reset-marker"
            marker.parent.mkdir(parents=True, exist_ok=True)
            marker.write_text("must disappear even with --keep-intermediates\n")
            markers.append(marker)
        foreign = out / "foreign-build-dir"
        foreign.mkdir()
        sentinel = foreign / "sentinel"
        sentinel.write_text("caller override must not reach nested Cargo\n")
        foreign_before = {str(p.relative_to(foreign)): digest(p) for p in foreign.rglob("*") if p.is_file()}
        observer = out / "cargo-observer"
        observer.mkdir()
        observer_logs = observer / "logs"
        observer_script = Path(__file__).with_name("source-suite-cargo-observer.py")
        shutil.copyfile(observer_script, observer / "cargo")
        (observer / "cargo").chmod(0o700)
        real_cargo = shutil.which("cargo", path=env["PATH"])
        assert real_cargo, "real Cargo unavailable before observer PATH change"
        real_cargo = str(Path(real_cargo).absolute())  # Preserve the Cargo proxy's symlink/argv[0].
        write_json(observer / "observer-config.json", dict(real_cargo=real_cargo, logs=str(observer_logs)))
        state["cargo_observer"] = dict(source_sha256=digest(observer_script), real_cargo=real_cargo,
                                       real_cargo_sha256=digest(Path(real_cargo)), max_calls=64, max_stdout_bytes_per_call=16 * 1024**2)
        rebuild_started = True
        run("rebuild", [str(xtask), "build-verus-source", "--keep-intermediates"],
            env | {"CARGO_BUILD_BUILD_DIR": str(foreign), "PATH": str(observer) + os.pathsep + env["PATH"]}, seconds=2700)
        assert all(not marker.exists() for marker in markers), "fixed output tree was not reset"
        foreign_after = {str(p.relative_to(foreign)): digest(p) for p in foreign.rglob("*") if p.is_file()}
        assert foreign_before == foreign_after and sorted(p.name for p in foreign.iterdir()) == ["sentinel"], "caller build-dir override escaped clean environment"
        current, after = inventory(build, pins)
        write_json(out / "after.json", after)
        assert {b["name"] for b in current["test_binaries"]} == {b["name"] for b in manifest["test_binaries"]}
        rebuilt_entry = next(b for b in current["test_binaries"] if b["name"] == selected["name"])
        rebuilt_executable = str(checked_path(build, rebuilt_entry["path"]))
        recompiled = []
        assert not (observer_logs / "overflow").exists(), "Cargo observer count limit refused"
        sidecars = sorted(observer_logs.glob("*.json"))
        assert sidecars, "no actual nested Cargo observations"
        assert int((observer_logs / "counter").read_text()) == len(sidecars), "Cargo observer sidecar missing"
        for sidecar in sidecars:
            record = json.loads(sidecar.read_text())
            raw = sidecar.with_suffix(".stdout")
            assert record["complete"] and raw.stat().st_size == record["total_bytes"] and digest(raw) == record["stdout_sha256"], "Cargo observer incomplete/refused"
            for line in raw.read_text().splitlines():
                try:
                    message = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if (message.get("reason") == "compiler-artifact" and message.get("target", {}).get("name") == selected["name"]
                        and "test" in message.get("target", {}).get("kind", [])
                        and message.get("profile", {}).get("test") is True
                        and message.get("executable") == rebuilt_executable):
                    recompiled.append(dict(cargo_sidecar=sidecar.name, cargo_exit=record["exit"], artifact=message))
        assert any(m["cargo_exit"] == 0 and m["artifact"].get("fresh") is False for m in recompiled), "no actual selected-test recompilation observed"
        state["recompiled_selected_test"] = recompiled
        final_list = list_suite("list-after-rebuild")
        assert first.with_suffix(".stdout").read_bytes() == final_list.with_suffix(".stdout").read_bytes(), "suite selection changed"
        changed = None
        manifest_changed = False
        state["status"] = "pass"
    except BaseException as error:
        state["status"] = "failed-or-interrupted"
        state["error"] = repr(error)
        raise
    finally:
        try:
            restore_mutation()
            if manifest_changed and not rebuild_started:
                (build / "sail-build.json").write_bytes(original_manifest)
            state["rebuild_started"] = rebuild_started
            run("stock-source-after", ["git", "diff", "--no-ext-diff", "--exit-code", "HEAD", "--"], env, cwd=checkout)
            state["binary_sha256_after"] = digest(binary)
            state["xtask_sha256_after"] = digest(xtask)
            if state["binary_sha256_after"] != state["binary_sha256"] or state["xtask_sha256_after"] != state["xtask_sha256"]:
                raise RuntimeError("harness or builder binary changed")
            state["checkout_head_after"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip()
            if state["checkout_head_after"] != source_head:
                state["status"] = "failed-or-interrupted"
                raise RuntimeError("stock source HEAD changed")
        except BaseException as error:
            state["status"] = "failed-or-interrupted"
            state["finalization_error"] = repr(error)
            raise
        finally:
            save()
    print("source-suite integrity controls: pass (actual stock build; see retained scope)")


if __name__ == "__main__":
    main()
