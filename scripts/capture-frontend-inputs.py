#!/usr/bin/env python3
"""Bounded ordinary-input diagnosis using an explicitly built research runtime.

No host management, compiler build, runtime identity fabrication or artifact
reuse. The runtime and patcher must be independently built and hashed first.
"""

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import time
import tomllib

RECORD_BYTES = 256 * 1024 * 1024
MANIFEST_BYTES = 1024 * 1024
RAW_HASH_BYTES = 64 * 1024 * 1024

PROJECTS = {
    "vest": ("https://github.com/secure-foundations/vest",
             "11af01ddaef76cceded03517cd48764adb9d35b1", "vest_lib",
             "vest_lib/src/asn1/der_ord.rs", 2341,
             ["module asn1::der_ord", "module asn1::boolean"]),
    "ironkv": ("https://github.com/verus-lang/verified-ironkv",
               "7e35ed097c4089b9fbda483ea2629a0033f4d874", "ironsht",
               "ironsht/src/host_impl_v.rs", 1780,
               ["module host_impl_v", "module verus_extra::clone_v"]),
}


def sha(path):
    result = hashlib.sha256()
    with path.open("rb") as source:
        while block := source.read(65536):
            result.update(block)
    return result.hexdigest()


def files(root):
    return {str(path.relative_to(root)): sha(path)
            for path in sorted(root.rglob("*")) if path.is_file()}


def verification(raw):
    text = raw.decode("utf-8")
    decoder, offset, values = json.JSONDecoder(), 0, []
    while offset < len(text):
        start = text.find("{", offset)
        if start < 0:
            break
        try:
            value, offset = decoder.raw_decode(text, start)
        except ValueError:
            offset = start + 1
            continue
        if isinstance(value, dict) and "verification-results" in value:
            result = value["verification-results"]
            if (result.get("errors", 1) != 0 or result.get("encountered-error", True)
                    or result.get("encountered-vir-error", True)):
                raise ValueError("verification reported errors")
            values.append({"verification-results": result,
                           "func-details": value.get("func-details")})
    if not values or not sum(value["verification-results"].get("verified", 0) for value in values):
        raise ValueError("missing positive verification JSON")
    return values


def validate_captures(directory, crate, buckets):
    manifests = sorted(directory.glob("*-manifest.json"))
    seen, result = [], []
    if len(manifests) != len(buckets):
        raise ValueError("missing/extra observer manifests")
    for path in manifests:
        value = json.loads(path.read_text())
        if (value.get("version") != 1 or value.get("crate") != crate
                or value.get("key_qualified") is not False
                or value.get("output_artifact_qualified") is not False
                or value.get("raw_pruned_bincode_finished") is not True
                or value.get("raw_pruned_bincode_byte_limit") != RAW_HASH_BYTES
                or value.get("raw_pruned_bincode_bytes", RAW_HASH_BYTES + 1) > RAW_HASH_BYTES
                or value.get("record_byte_limit") != RECORD_BYTES
                or value.get("process_record_limit") != 2
                or value.get("fresh_lowering_returned") is not True
                or value.get("fresh_bucket_returned") is not True):
            raise ValueError("incomplete or unexpected observer scope")
        artifacts = value.get("artifacts", [])
        if len(artifacts) != 3:
            raise ValueError("missing/extra observer artifacts")
        for artifact in artifacts:
            name = artifact["name"]
            if Path(name).name != name or artifact.get("representation_finished") is not True:
                raise ValueError("invalid or incomplete artifact")
            payload = directory / name
            if (payload.stat().st_size != artifact["bytes"]
                    or sha(payload) != artifact["sha256"]):
                raise ValueError("observer artifact bytes/digest mismatch")
        if sorted(a["name"].rsplit("-", 1)[-1] for a in artifacts) != [
                "ast.json", "context.debug", "sst.debug"]:
            raise ValueError("unexpected artifact roles")
        if path.stat().st_size > MANIFEST_BYTES:
            raise ValueError("observer manifest budget exceeded")
        if sum(a["bytes"] for a in artifacts) > RECORD_BYTES - MANIFEST_BYTES:
            raise ValueError("observer payload budget exceeded")
        seen.append(value["bucket"])
        result.append({"manifest": path.name, "sha256": sha(path), "bucket": value["bucket"]})
    if sorted(seen) != sorted(buckets):
        raise ValueError("captured bucket set differs")
    return result


def validate_disabled(directory):
    if any(directory.iterdir()):
        raise ValueError("disabled observer unexpectedly emitted artifacts")
    return []


def dependency_source_identity(root):
    """Bind the full dependency checkout; a packaged runtime is not its source."""
    for name in ["Cargo.toml", "Cargo.lock", "vstd/Cargo.toml",
                 "builtin/Cargo.toml", "builtin_macros/Cargo.toml"]:
        if not (root / "source" / name).is_file():
            raise ValueError("dependency source checkout is incomplete: " + name)
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root).decode().strip()
    status = subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"],
                                     cwd=root).decode()
    if status:
        raise ValueError("dependency source checkout has tracked changes")
    return {"head": head, "lock_sha256": sha(root / "source/Cargo.lock")}


def validate_patch_receipt(raw, project):
    """Reject the inherited patcher's silent None/empty success."""
    match = re.fullmatch(r'Some\(("(?:[^"\\]|\\.)*")\) (\[[^\r\n]*\])\s*', raw.decode())
    if match is None:
        raise ValueError("patcher did not establish a nonempty dependency patch")
    path, names = Path(json.loads(match.group(1))), json.loads(match.group(2))
    path = path.resolve(strict=True)
    path.relative_to(project.resolve(strict=True))
    if path.name != "Cargo.toml" or not path.is_file() or not isinstance(names, list):
        raise ValueError("invalid dependency patch receipt")
    if not all(isinstance(name, str) for name in names) or "vstd" not in names:
        raise ValueError("dependency patch does not include vstd")
    return {"manifest": str(path), "crates": names}


def validate_dependency_resolution(metadata, root):
    """Require the actual Cargo graph to select the frozen fork library source."""
    expected = {"vstd": "vstd", "verus_builtin": "builtin",
                "verus_builtin_macros": "builtin_macros",
                "verus_state_machines_macros": "state_machines_macros"}
    required = {"vstd", "verus_builtin", "verus_builtin_macros"}
    found = {}
    for package in metadata["packages"]:
        name = package["name"]
        if name not in expected:
            continue
        manifest = (root / "source" / expected[name] / "Cargo.toml").resolve(strict=True)
        if (name in found or package.get("source") is not None
                or Path(package["manifest_path"]).resolve(strict=True) != manifest):
            raise ValueError("Cargo selected a foreign/duplicate Verus dependency: " + name)
        found[name] = {key: package[key] for key in ["id", "version", "manifest_path", "source"]}
    if not required.issubset(found):
        raise ValueError("Cargo graph omits a required fork Verus dependency")
    return found


def body_edit(original, line):
    text = original.decode("utf-8")
    lines = text.splitlines(keepends=True)
    offset = sum(map(len, lines[:line - 1]))
    function = text.find("fn ", offset)
    if function < 0 or function >= offset + len(lines[line - 1]):
        raise ValueError("frozen body-edit location is no longer a function")
    parens, brackets = 0, 0
    for index in range(function, len(text)):
        char = text[index]
        parens += (char == "(") - (char == ")")
        brackets += (char == "[") - (char == "]")
        if char == "{" and parens == 0 and brackets == 0:
            return (text[:index + 1] + " assert(true);" + text[index + 1:]).encode()
    raise ValueError("frozen body-edit function has no body")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", choices=PROJECTS, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--patcher", type=Path, required=True)
    parser.add_argument("--verus-source", type=Path, required=True,
                        help="full frozen Verus checkout; packaged runtime layout is insufficient")
    parser.add_argument("--verus-source-commit", required=True)
    parser.add_argument("--runtime-manifest", type=Path, required=True,
                        help="exact path->SHA-256 JSON for independently built runtime")
    parser.add_argument("--patcher-sha256", required=True)
    parser.add_argument("--work", type=Path, required=True, help="new owned output path")
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("native x86-64 Linux only")
    runtime, patcher = args.runtime.resolve(strict=True), args.patcher.resolve(strict=True)
    dependency_source = args.verus_source.resolve(strict=True)
    source_identity = dependency_source_identity(dependency_source)
    if source_identity["head"] != args.verus_source_commit:
        parser.error("dependency source differs from the frozen build commit")
    expected_runtime = json.loads(args.runtime_manifest.read_text())
    if files(runtime) != expected_runtime or sha(patcher) != args.patcher_sha256:
        parser.error("runtime/patcher differs from frozen identities")
    args.work = args.work.absolute()
    args.work.mkdir(parents=True, exist_ok=False)
    root, out, target = args.work / "project", args.work / "results", args.work / "target"
    out.mkdir()
    url, pin, package, edit, line, buckets = PROJECTS[args.project]
    manifest = {"scope": "ordinary-input research; no frontend reuse or performance acceptance",
                "complete": False, "capture_gate": False, "candidate_gate": False,
                "project": args.project, "pin": pin, "buckets": buckets,
                "runtime_sha256": expected_runtime, "patcher_sha256": args.patcher_sha256,
                "dependency_source": source_identity,
                "steps": [], "errors": [], "platform": platform.platform()}
    toolchain = tomllib.loads((dependency_source / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    environment = dict(os.environ, RUSTUP_TOOLCHAIN=toolchain, CARGO_INCREMENTAL="0",
                       CARGO_BUILD_JOBS="8", CARGO_TARGET_DIR=str(target))
    for key in list(environment):
        if key.startswith(("SAIL_", "__VERUS_")) or key in [
                "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER", "LD_PRELOAD", "DYLD_INSERT_LIBRARIES",
                "CARGO_BUILD_BUILD_DIR", "CARGO_BUILD_TARGET_DIR"]:
            environment.pop(key, None)
    environment["VERUS_Z3_PATH"] = str(runtime / "z3")
    saved = {}

    def save():
        temporary = out / "manifest.tmp"
        temporary.write_text(json.dumps(manifest, indent=2) + "\n")
        temporary.replace(out / "manifest.json")

    def run(label, command, cwd, env, timeout=600):
        row = {"label": label, "command": list(map(str, command)), "cwd": str(cwd),
               "timeout_seconds": timeout,
               "environment_sha256": hashlib.sha256(json.dumps(sorted(env.items())).encode()).hexdigest()}
        manifest["steps"].append(row)
        save()
        started = time.monotonic()
        with (out / (label + ".stdout")).open("wb") as so, (out / (label + ".stderr")).open("wb") as se:
            child = subprocess.Popen(command, cwd=cwd, env=env, stdout=so, stderr=se,
                                     stdin=subprocess.DEVNULL, start_new_session=True)
            try:
                child.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                row["timeout"] = True
            finally:
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGKILL)
                row["exit"] = child.wait(timeout=10)
                row["wall_seconds"] = time.monotonic() - started
                row["stdout_sha256"] = sha(out / (label + ".stdout"))
                row["stderr_sha256"] = sha(out / (label + ".stderr"))
                save()
        if row["exit"] != 0:
            raise ValueError(label + " failed; retained raw output")
        return row

    def interrupted(number, _frame):
        raise KeyboardInterrupt("signal " + str(number))

    signal.signal(signal.SIGTERM, interrupted)
    try:
        run("clone", ["git", "clone", "--no-checkout", "--filter=blob:none", url, str(root)],
            args.work, environment, 180)
        run("checkout", ["git", "checkout", "--detach", pin], root, environment, 120)
        actual_pin = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root).decode().strip()
        if actual_pin != pin:
            raise ValueError("project pin mismatch")
        saved = {path: path.read_bytes() for path in root.rglob("*")
                 if path.is_file() and ".git" not in path.relative_to(root).parts
                 and path.name in ["Cargo.toml", "Cargo.lock"]}
        saved[root / edit] = original = (root / edit).read_bytes()
        run("patch", [str(patcher), str(root / package), str(root), str(dependency_source)],
            root, environment, 60)
        manifest["dependency_patch"] = validate_patch_receipt((out / "patch.stdout").read_bytes(), root)
        command = [str(runtime / "cargo-verus"), "verify", "--", "--output-json", "--time"]
        prepared = run("prepare", command, root / package, environment)
        prepared["verification"] = verification((out / "prepare.stdout").read_bytes())
        run("dependency-metadata", ["cargo", "metadata", "--format-version", "1", "--locked"],
            root / package, environment, 120)
        manifest["dependency_resolution"] = validate_dependency_resolution(
            json.loads((out / "dependency-metadata.stdout").read_bytes()), dependency_source)
        locked = {path: path.read_bytes() for path in root.rglob("Cargo.lock")
                  if ".git" not in path.relative_to(root).parts}
        if not locked:
            raise ValueError("no resolved lockfile found")
        lock_copy = out / "resolved-locks"
        for path, raw in locked.items():
            copy = lock_copy / path.relative_to(root)
            copy.parent.mkdir(parents=True, exist_ok=True)
            copy.write_bytes(raw)
        manifest["resolved_locks"] = {str(p.relative_to(root)): sha(p) for p in locked}
        tee = out / "tee-z3.sh"
        tee.write_text('#!/bin/sh\nf=$(mktemp "$SAIL_M8_SMT/z3-XXXXXXXXXX") || exit 125\n'
                       'tee "$f" | "$SAIL_M8_SOLVER" "$@"\n')
        tee.chmod(0o755)
        rows = []
        for label, source in [("disabled", original), ("same-1", original), ("same-2", original),
                              ("comment", b"// sail M8 raw-input observation\n" + original),
                              ("body", body_edit(original, line))]:
            (root / edit).write_bytes(source)
            run(label + "-clean", ["cargo", "clean", "-p", package], root / package, environment, 60)
            sample = out / label
            observer, smt = sample / "observer", sample / "smt"
            observer.mkdir(parents=True)
            smt.mkdir()
            env = dict(environment, VERUS_Z3_PATH=str(tee), SAIL_M8_SMT=str(smt),
                       SAIL_M8_SOLVER=str(runtime / "z3"))
            if label != "disabled":
                env.update(SAIL_M8_INPUT_EVIDENCE_DIR=str(observer),
                           SAIL_M8_INPUT_EVIDENCE_CRATE=package,
                           SAIL_M8_INPUT_EVIDENCE_BUCKETS="\n".join(buckets))
            row = run(label, [command[0], "verify", "--locked", *command[2:]], root / package, env)
            row["source_sha256"] = hashlib.sha256(source).hexdigest()
            row["verification"] = verification((out / (label + ".stdout")).read_bytes())
            row["observer"] = (validate_captures(observer, package, buckets) if label != "disabled"
                               else validate_disabled(observer))
            row["smt_multiset"] = dict(Counter(sha(path) for path in smt.iterdir() if path.is_file()))
            if not row["smt_multiset"]:
                raise ValueError("empty SMT capture")
            if {str(p.relative_to(root)): sha(p) for p in root.rglob("Cargo.lock")
                    if ".git" not in p.relative_to(root).parts} != manifest["resolved_locks"]:
                raise ValueError("resolved lockfile changed")
            rows.append(row)
            save()
        baseline = rows[0]
        manifest["same_source_raw_smt_equal"] = all(
            row["smt_multiset"] == baseline["smt_multiset"] for row in rows[1:3])
        manifest["verification_json_equal"] = all(
            row["verification"] == baseline["verification"] for row in rows[1:])
        manifest["edited_raw_smt_equal"] = {
            row["label"]: row["smt_multiset"] == baseline["smt_multiset"] for row in rows[3:]}
        manifest["complete"] = True
        manifest["capture_gate"] = (manifest["same_source_raw_smt_equal"]
                                    and manifest["verification_json_equal"])
    except BaseException as error:
        manifest["errors"].append(type(error).__name__ + ": " + str(error))
    finally:
        try:
            for path in root.rglob("Cargo.lock"):
                if ".git" not in path.relative_to(root).parts and path not in saved:
                    path.unlink()
            for path, raw in saved.items():
                path.write_bytes(raw)
            if saved and subprocess.check_output(["git", "diff"], cwd=root):
                raise ValueError("tracked project inputs differ after restoration")
            if dependency_source_identity(dependency_source) != source_identity:
                raise ValueError("dependency source changed during capture")
            if files(runtime) != expected_runtime or sha(patcher) != args.patcher_sha256:
                raise ValueError("runtime or patcher changed during capture")
        except BaseException as error:
            manifest["errors"].append("final identity/restoration: " + str(error))
            manifest["capture_gate"] = False
        save()
    return 0 if manifest["capture_gate"] and not manifest["errors"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
