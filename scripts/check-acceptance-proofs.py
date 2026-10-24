#!/usr/bin/env python3
"""Check the acceptance models directly with the pinned upstream Verus.

This is a local proof-checking spike, not production acceptance authority.
Requires Python 3.11 or newer. No Sail binary participates in verification.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile
import time
import tomllib
import zipfile


COMPONENTS = {"query-association", "input-binding", "authority"}
MANIFEST = Path("proofs/acceptance/manifest.json")


class Refusal(Exception):
    """A failed identity, inventory, scope or verification check."""


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate JSON member: " + key)
        result[key] = value
    return result


def require(condition: bool, message: str) -> None:
    if not condition:
        raise Refusal(message)


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def standalone(text: str) -> None:
    # Conservative refusal applies to original and generated proof candidates,
    # including tokens in comments/strings; this does not pretend to parse Rust.
    require(not re.search(r"\b(?:mod|include|include_str|include_bytes|extern|macro_rules)\b", text), "proof source is outside the standalone subset")


def inventory(root: Path, manifest: dict, snapshots: dict[str, bytes] | None = None) -> list[dict]:
    require(manifest.get("schema") == "sail-acceptance-proofs-v1", "unknown proof manifest")
    entries = manifest.get("components", [])
    require(len(entries) == len(COMPONENTS), "proof component omitted or duplicated")
    require({entry["id"] for entry in entries} == COMPONENTS, "wrong proof components")
    sources = [Path(entry["source"]) for entry in entries]
    require(len(set(sources)) == len(sources), "duplicate proof root")
    actual = {path.relative_to(root) for path in (root / "proofs/acceptance").rglob("*.rs")}
    require(set(sources) == actual, "missing or unlisted proof source")
    for entry, source in zip(entries, sources, strict=True):
        require(not source.is_absolute() and ".." not in source.parts, "invalid proof path")
        require(source.parts[:2] == ("proofs", "acceptance"), "proof outside inventory")
        require(not (root / source).is_symlink(), "symlink proof root")
        require((root / source).resolve().is_relative_to(root.resolve()), "proof path escapes root")
        require(type(entry["verified"]) is int and entry["verified"] > 0, "empty proof count")
        require(bool(entry["functions"]), "empty expected function inventory")
        require(len(entry["functions"]) == len(set(entry["functions"])), "duplicate function")
        require(bool(entry["negative_controls"]), "missing anti-vacuity controls")
        text = (root / source).read_text() if snapshots is None else snapshots[str(source)].decode()
        standalone(text)
        for control in entry["negative_controls"]:
            require(control["before"] != control["after"] and bool(control["before"]), "empty mutation")
            require(text.count(control["before"]) == 1, "ambiguous mutation")
            standalone(text.replace(control["before"], control["after"]))
    return entries


def validated_runtime(archive: Path, install: Path, asset: dict) -> dict[str, str]:
    require(digest(archive) == asset["sha256"], "stock archive identity mismatch")
    expected: dict[str, str] = {}
    prefix = asset["dir"] + "/"
    with zipfile.ZipFile(archive) as bundle:
        for item in bundle.infolist():
            if item.is_dir() or not item.filename.startswith(prefix):
                continue
            relative = Path(item.filename.removeprefix(prefix))
            require(not relative.is_absolute() and ".." not in relative.parts, "archive path escapes")
            require(relative.as_posix() not in expected, "duplicate stock archive member")
            target = install / relative
            require(not target.is_symlink() and target.resolve().is_relative_to(install.resolve()), "runtime path escapes")
            with bundle.open(item) as stream:
                expected[relative.as_posix()] = hashlib.file_digest(stream, "sha256").hexdigest()
            require(target.is_file() and digest(target) == expected[relative.as_posix()], "stock runtime member changed: " + str(relative))
    require({"verus", "rust_verify", "z3"} <= expected.keys(), "incomplete stock runtime")
    actual = {path.relative_to(install).as_posix() for path in install.rglob("*") if path.is_file()}
    require(actual == expected.keys(), "unlisted stock runtime file")
    require(expected["z3"] == asset["z3_sha256"], "pinned solver mismatch")
    return expected


def proof_environment(install: Path, toolchain: str) -> dict[str, str]:
    # Retain only process-discovery and standard OS locations. In particular,
    # no inherited Sail, solver, injected Cargo arguments or loader settings.
    retained = {"HOME", "USER", "LOGNAME", "PATH", "TMPDIR", "TEMP", "TMP", "LANG", "LC_ALL", "RUSTUP_HOME", "CARGO_HOME", "SYSTEMROOT"}
    result = {key: value for key, value in os.environ.items() if key in retained}
    result.update(RUSTUP_TOOLCHAIN=toolchain, VERUS_Z3_PATH=str(install / "z3"))
    return result


def checked_document(output: str, code: int, entry: dict, pins: dict, *, broken: bool) -> dict:
    try:
        document = json.loads(output, object_pairs_hook=unique_object)
    except (ValueError, TypeError) as error:
        raise Refusal("missing complete verifier JSON") from error
    results = document.get("verification-results", {})
    version = document.get("verus", {})
    require(version.get("commit") == pins["verus"]["commit"], "unexpected verifier source")
    require(version.get("version") == pins["verus"]["release"], "unexpected verifier release")
    require(results.get("is-verifying-entire-crate") is True, "verification scope narrowed")
    require(results.get("encountered-vir-error") is False, "invalid proof program")
    for counter in ("verified", "errors"):
        require(type(results.get(counter)) is int and results[counter] >= 0, "invalid verifier counter")
    stem = Path(entry["source"]).stem + "::"
    functions = {name for name in document.get("func-details", {}) if name.startswith(stem)}
    require(functions == set(entry["functions"]), "proof function inventory changed")
    if broken:
        require(code == 1 and results.get("success") is False, "broken proof did not terminate with verification failure")
        require(results.get("encountered-error") is True and results["errors"] > 0, "negative control did not fail verification")
        require(results["verified"] + results["errors"] == entry["verified"], "negative proof scope count changed")
    else:
        require(code == 0 and results.get("success") is True, "proof did not succeed")
        require(results.get("encountered-error") is False and results.get("errors") == 0, "proof contains errors")
        require(results.get("verified") == entry["verified"], "verified obligation count changed")
    return document


def git(root: Path, *arguments: str) -> str:
    return subprocess.check_output(["git", "-C", str(root), *arguments], text=True).strip()


def check(root: Path, archive: Path, install: Path, out: Path, timeout: int) -> None:
    require(not out.resolve().is_relative_to(root.resolve()), "put raw proof output outside the checkout")
    commit = git(root, "rev-parse", "HEAD")
    proof_paths = git(root, "ls-tree", "-r", "--name-only", commit, "--", "proofs/acceptance").splitlines()
    inputs = ["corpus/toolchain.toml", str(MANIFEST), "scripts/check-acceptance-proofs.py", *[source for source in proof_paths if source.endswith(".rs")]]
    # Parse and compile the actual committed bytes, rather than hashing a path
    # and later reopening it for compilation. Pre/post checks alone miss ABA.
    snapshots = {source: subprocess.check_output(["git", "-C", str(root), "show", commit + ":" + source]) for source in inputs}
    pins = tomllib.loads(snapshots["corpus/toolchain.toml"].decode())
    machine = (platform.system(), platform.machine())
    hosts = {("Darwin", "arm64"): "aarch64-apple-darwin", ("Linux", "x86_64"): "x86_64-unknown-linux-gnu"}
    require(machine in hosts, "unsupported proof-checking platform")
    asset = pins["assets"][hosts[machine]]
    manifest = json.loads(snapshots[str(MANIFEST)], object_pairs_hook=unique_object)
    entries = inventory(root, manifest, snapshots)
    for source in inputs:
        git(root, "ls-files", "--error-unmatch", "--", source)
    require(not git(root, "status", "--porcelain=v1", "--", *inputs), "proof inputs are not committed")
    require(all((root / source).read_bytes() == content for source, content in snapshots.items()), "working inputs differ from the selected commit")
    hashes = {source: hashlib.sha256(content).hexdigest() for source, content in snapshots.items()}
    runtime = validated_runtime(archive, install, asset)
    env = proof_environment(install, pins["verus"]["rust_toolchain"])
    rustc = subprocess.check_output(["rustup", "run", pins["verus"]["rust_toolchain"], "rustc", "-vV"], env=env, text=True)
    require("commit-hash: " + pins["verus"]["rust_commit"] in rustc.splitlines(), "Rust toolchain identity mismatch")
    out.mkdir(parents=True, exist_ok=False)
    rows = []
    for entry in entries:
        source = snapshots[entry["source"]]
        cases = [("positive", source, False)] + [(control["name"], source.replace(control["before"].encode(), control["after"].encode()), True) for control in entry["negative_controls"]]
        for index, (name, candidate, broken) in enumerate(cases):
            with tempfile.TemporaryDirectory(prefix="sail-stock-proof-") as scratch:
                path = Path(scratch) / Path(entry["source"]).name
                path.write_bytes(candidate)
                command = [str(install / "verus"), str(path), "--no-cheating", "--output-json", "--num-threads", "1"]
                start = time.monotonic()
                completed = subprocess.run(command, env=env, cwd=scratch, text=True, capture_output=True, timeout=timeout)
                label = entry["id"] + "-" + str(index)
                (out / (label + ".stdout")).write_text(completed.stdout)
                (out / (label + ".stderr")).write_text(completed.stderr)
                combined = (completed.stdout + completed.stderr).lower()
                require(not any(marker in combined for marker in ("resource limit exceeded", "timed out", "rlimit exceeded", "reached its resource limit")), "proof resource limit reached")
                document = checked_document(completed.stdout, completed.returncode, entry, pins, broken=broken)
                rows.append({"component": entry["id"], "control": name, "negative": broken, "candidate_sha256": hashlib.sha256(candidate).hexdigest(), "seconds": time.monotonic() - start, "exit": completed.returncode, "verification": document["verification-results"], "command": command})
                print(entry["id"] + ": " + name + " passed", flush=True)
    require(git(root, "rev-parse", "HEAD") == commit, "checked commit changed")
    require(all(digest(root / source) == value for source, value in hashes.items()), "proof source changed during checking")
    require(validated_runtime(archive, install, asset) == runtime, "runtime changed during checking")
    report = {"commit": commit, "source_sha256": hashes, "stock_archive_sha256": asset["sha256"], "runtime_sha256": runtime, "rustc": rustc, "cases": rows, "scope": "standalone acceptance models; production refinement remains unproved"}
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    (out / "report.md").write_text("# Acceptance model proof check\n\nCommit `" + commit + "`. Direct pinned stock Verus; full crate; `--no-cheating`; explicit pinned Z3.\n\nAll " + str(len(rows)) + " positive and mutation checks passed. These standalone model proofs do not establish production refinement or the composed service theorem.\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stock-archive", type=Path, required=True)
    parser.add_argument("--stock-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    try:
        require(args.timeout > 0, "timeout must be positive")
        check(Path(__file__).resolve().parents[1], args.stock_archive.resolve(), args.stock_dir.resolve(), args.out.resolve(), args.timeout)
    except (Refusal, OSError, KeyError, TypeError, ValueError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        print("error[SAIL9002]: acceptance proof check refused: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
