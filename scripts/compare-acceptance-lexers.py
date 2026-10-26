#!/usr/bin/env python3
"""Offline lexer differential over frozen Rust sources, mutations and source controls.

This local qualification helper invokes Git and, when requested, the existing
source-control unittest module. It never invokes Verus or selects proof evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import types
import unittest


LEXER = "scripts/acceptance-source.py"
TESTS = "scripts/test-check-acceptance-refinements.py"
MANIFESTS = ("proofs/acceptance/manifest.json", "proofs/refinement/manifest.json")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def unique_object(items):
    result = {}
    for key, value in items:
        require(key not in result, "duplicate differential manifest key")
        result[key] = value
    return result


def object_id(value: str) -> str:
    require(re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", value) is not None,
        "use a complete lowercase Git object ID")
    return value


def git(root: Path, *arguments: str, data: bytes | None = None) -> bytes:
    return subprocess.run(["git", "--no-replace-objects", "-c", "core.fsmonitor=false",
        "-C", str(root), *arguments], input=data, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, check=True).stdout


def checked_blob(oid: str, data: bytes) -> bytes:
    digest = hashlib.sha1 if len(oid) == 40 else hashlib.sha256
    require(digest(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest() == oid,
        "Git blob bytes differ from requested object identity")
    return data


def source_tree(root: Path, commit: str) -> dict[str, tuple[str, str]]:
    tree = {}
    for record in git(root, "ls-tree", "-rz", "--full-tree", commit).split(b"\0"):
        if not record:
            continue
        header, path = record.split(b"\t", 1)
        mode, kind, oid = header.decode().split()
        if kind == "blob":
            name = path.decode("utf-8")
            require(name not in tree, "duplicate Git source path")
            tree[name] = (mode, object_id(oid))
    return tree


def blobs(root: Path, ids: set[str]) -> dict[str, bytes]:
    ordered = sorted(ids)
    data = git(root, "cat-file", "--batch", data=b"".join(oid.encode() + b"\n" for oid in ordered))
    result = {}
    at = 0
    for oid in ordered:
        end = data.find(b"\n", at)
        require(end >= at, "missing Git batch header")
        fields = data[at:end].split()
        require(len(fields) == 3 and fields[0] == oid.encode()
            and fields[1] == b"blob" and fields[2].isdigit(), "unexpected Git batch object")
        begin = end + 1
        at = begin + int(fields[2])
        require(at < len(data) and data[at] == 10, "truncated Git batch body")
        result[oid] = checked_blob(oid, data[begin:at])
        at += 1
    require(at == len(data), "unexpected trailing Git batch output")
    return result


def load(name: str, path: Path, data: bytes):
    module = types.ModuleType(name)
    module.__file__ = str(path)
    sys.modules[name] = module
    exec(compile(data, str(path), "exec"), module.__dict__)
    return module


def outcome(module, function, source: str):
    try:
        result = function(source)
    except module.SourceRefusal as error:
        return ("refused", str(error)), error
    return ("tokens", tuple(tuple(token) for token in result)), result


class Comparison:
    def __init__(self, original, candidate):
        self.original = original
        self.candidate = candidate
        self.cases = []

    def compare(self, name: str, source: str, module=None, function=None):
        module = self.candidate if module is None else module
        function = module.tokens if function is None else function
        before, _ = outcome(self.original, self.original.tokens, source)
        after, result = outcome(module, function, source)
        item = {"name": name, "source_sha256": hashlib.sha256(source.encode("utf-8", "surrogatepass")).hexdigest(),
            "original": before[0], "candidate": after[0], "equal": before == after}
        if after[0] == "tokens":
            item["tokens"] = len(after[1])
        else:
            item["refusal"] = after[1]
        self.cases.append(item)
        require(before == after, "lexer differential mismatch: " + name)
        if after[0] == "refused":
            raise result
        return result

    def record(self, name: str, source: str) -> None:
        try:
            self.compare(name, source)
        except self.candidate.SourceRefusal:
            pass  # Equal exact refusal is a differential result, not acceptance.


def unit_controls(root: Path, tree: dict, contents: dict, comparison: Comparison) -> dict:
    test_bytes = contents[tree[TESTS][1]]
    require((root / TESTS).read_bytes() == test_bytes, "unit controls differ from frozen Git source")
    tests = load("sail_lexer_differential_unit_controls", root / TESTS, test_bytes)
    require(Path(tests.SOURCE.__file__).resolve() == (root / LEXER).resolve(),
        "unit controls did not import the selected candidate lexer")
    for module in (tests.CHECKER, tests.CHECKER.COMMON, tests.SOURCE):
        path = Path(module.__file__).resolve()
        relative = path.relative_to(root).as_posix()
        require(relative in tree, "unit control imported an untracked helper")
        expected = checked_blob(tree[relative][1], git(root, "cat-file", "blob", tree[relative][1]))
        require(path.read_bytes() == expected, "unit control helper differs from frozen source")
    original_tokens = tests.SOURCE.tokens
    calls = 0

    def compared_tokens(source):
        nonlocal calls
        calls += 1
        return comparison.compare("unit-token-call:" + str(calls), source, tests.SOURCE, original_tokens)

    tests.SOURCE.tokens = compared_tokens
    try:
        suite = unittest.defaultTestLoader.loadTestsFromModule(tests)
        result = unittest.TextTestRunner(stream=sys.stderr).run(suite)
    finally:
        tests.SOURCE.tokens = original_tokens
    require(result.wasSuccessful(), "existing source-control tests failed")
    require(calls > 0, "unit controls did not call the compared lexers")
    return {"tests": result.testsRun, "dual_lexer_calls": calls}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--source-commit", type=object_id, required=True)
    parser.add_argument("--original-blob", type=object_id, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--unit-controls", action="store_true",
        help="also run every existing source-control test with dual lexer comparisons")
    args = parser.parse_args()
    root = args.root.resolve()
    report = {"scope": "lexer equality only; no proof/runtime qualification or performance measurement",
        "source_commit": args.source_commit, "original_blob": args.original_blob, "status": "incomplete"}
    comparison = None
    # Reserve a fresh external receipt before work; interrupted output is never a pass.
    require(not args.out.resolve().is_relative_to(root), "put differential receipt outside checkout")
    with args.out.open("x") as receipt:
        try:
            require(git(root, "cat-file", "-t", args.source_commit).strip() == b"commit",
                "source commit is not a Git commit object")
            tree = source_tree(root, args.source_commit)
            rust = sorted(path for path in tree if path.endswith(".rs"))
            require(bool(rust), "no frozen Rust sources")
            requested = [*rust, *MANIFESTS, LEXER, TESTS]
            require(all(path in tree and tree[path][0] in {"100644", "100755"}
                for path in requested), "missing/nonregular frozen source")
            contents = blobs(root, {args.original_blob, *(tree[path][1] for path in requested)})
            original = load("sail_original_lexer", root / LEXER, contents[args.original_blob])
            candidate = load("sail_candidate_lexer", root / LEXER, contents[tree[LEXER][1]])
            comparison = Comparison(original, candidate)
            report["candidate_blob"] = tree[LEXER][1]
            report["rust_sources"] = len(rust)
            for path in rust:
                comparison.record("source:" + path, contents[tree[path][1]].decode("utf-8"))
            mutations = set()
            for manifest_path in MANIFESTS:
                manifest = json.loads(contents[tree[manifest_path][1]], object_pairs_hook=unique_object)
                require(bool(manifest["components"]), "empty differential manifest")
                for entry in manifest["components"]:
                    require(bool(entry["negative_controls"]), "empty component mutation inventory")
                    for control in entry["negative_controls"]:
                        name = manifest_path + ":" + entry["id"] + ":" + control["name"]
                        require(name not in mutations, "duplicate differential mutation identity")
                        mutations.add(name)
                        path = control.get("source", entry["source"])
                        require(path in rust, "mutation source outside frozen Rust inventory")
                        source = contents[tree[path][1]]
                        before, after = control["before"].encode(), control["after"].encode()
                        require(bool(before) and before != after and source.count(before) == 1,
                            "mutation missing/ambiguous in frozen source: " + name)
                        comparison.record("mutation:" + name, source.replace(before, after, 1).decode("utf-8"))
            require(bool(mutations), "no frozen mutations")
            report["mutations"] = len(mutations)
            if args.unit_controls:
                report["unit_controls"] = unit_controls(root, tree, contents, comparison)
            report["status"] = "passed"
        except (Exception, KeyboardInterrupt) as error:
            report["status"] = "failed"
            report["error"] = type(error).__name__ + ": " + str(error)
        finally:
            report["cases"] = [] if comparison is None else comparison.cases
            receipt.write(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items() if key != "cases"}))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
