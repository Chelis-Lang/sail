#!/usr/bin/env python3
"""Check captured production Rust bodies and their acceptance refinements.

Requires Python 3.11+. This runner is ordinary reviewed/tested code. Its reports
distinguish checked executable bodies, declaration projections and surrounding
runtime source that is bound to the commit but remains outside the proof.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import time
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = "proofs/refinement/manifest.json"
SYNTAX = "crates/sail-acceptance/src/syntax.rs"
# This reviewed source uses the identical argument/type/body metavariables in
# Cargo and stock profiles. Changes require an explicit runner review, beyond
# merely updating a manifest. Runtime-only implementation drift must refuse.
SYNTAX_SHA256 = 'b346fda2d5fc49adc2e20b98661f53423cfc5edc6a2c28b331a808b78864422a'
LIB_SHA256 = "3e8ae88827486b5c993273f7c26bd2da86ec18ac61844d2dde81c7dd2f26fda9"
API_LIB_SHA256 = "125f9959d72bf6bbff719526cdaad169301b9c1e156db5b46ea79b91d4c50491"
# Binding inventories cover both ordinary and proof-only imports. In particular,
# an erased import cannot shadow a Cargo glob with a different checked callee.
IMPORT_SHA256 = {'crates/sail-acceptance/src/query.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/inputs.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/authority.rs': 'd0adfcd0a1e64a0addc3db112f9f9fc707f3f4c68bd7b907047a00b8d7f4abb4', 'crates/sail-api/src/acceptance.rs': 'aafada80e80a25525d1ead7e907def7df72ca97f842aa7f8d795157feb1b3dbc'}
REQUIRED_CONTROLS = {'query': ['fifo_wrong_stack', 'partial_pair_consumes_head', 'replay_off_by_one', 'noisy_publication', 'corrupt_recovery', 'ungated_batch_release', 'incomplete_permutation', 'raw-ignore-command-bytes', 'raw-ignore-gap', 'raw-skip-separators', 'record-ignore-header-newline', 'record-change-separator', 'record-keep-separator-in-reply', 'frame-premature-root-close', 'frame-ignore-doubled-quote', 'frame-ignore-input-comment', 'head-ignore-framing', 'response-ignore-line-terminator', 'response-ignore-list-framing'], 'inputs': ['input-ignore-present-value', 'input-ignore-current-stamp', 'input-ignore-required-read-presence', 'input-ignore-inverse-permutation', 'input-provider-wrong-frame-child', 'input-provider-terminal-count', 'input-provider-required-read-frame', 'input-provider-required-read-presence', 'input-opened-object-binding', 'input-read-final-handle', 'input-read-final-path', 'input-read-chunk-bytes', 'input-read-eof-required', 'input-read-cache-object', 'input-read-requested-binding', 'input-environment-byte-name', 'input-environment-escaped-name'], 'authority': ['verification-enabled', 'effective-verification-enabled', 'whole-terminal-summary', 'confirmed-function', 'whole-command', 'requested-coverage', 'summary-count', 'normal-exit', 'fresh-selection', 'obligation-uniqueness', 'provenance-counts', 'request-original-context', 'request-independent-coverage', 'request-verification-enabled', 'request-current', 'request-repair-function', 'request-positive', 'request-repair-presence', 'submission-arguments', 'submission-mode', 'submission-whole-authority', 'submission-positive', 'driver-token-bytes', 'driver-token-coverage', 'driver-sources-order']}
COMPONENTS = {
    "query": {"models": ["context", "query_association"], "core": ["inputs", "query"]},
    "inputs": {"models": ["context"], "core": ["inputs"]},
    "authority": {"models": ["context", "authority"], "core": ["inputs", "authority"]},
}


def module(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / filename)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


COMMON = module("sail_acceptance_model_checker", "scripts/check-acceptance-proofs.py")
SOURCE = module("sail_acceptance_source_capture", "scripts/acceptance-source.py")
require = COMMON.require
Refusal = COMMON.Refusal


def validate_assembly(snapshots: dict[str, bytes]) -> None:
    """Bind proof module paths and private tokens to the real Cargo assembly."""
    library = "crates/sail-acceptance/src/lib.rs"
    require(hashlib.sha256(snapshots[library]).hexdigest() == LIB_SHA256,
        "production module assembly changed; explicit runner review required")
    require(hashlib.sha256(snapshots["crates/sail-api/src/lib.rs"]).hexdigest() == API_LIB_SHA256,
        "API factory module assembly changed; explicit runner review required")
    leaf = tomllib.loads(snapshots["crates/sail-acceptance/Cargo.toml"].decode())
    require(set(leaf) == {"package", "lints"} and leaf["lints"] == {"workspace": True},
        "custom leaf target/dependency/feature/build configuration")
    package = leaf["package"]
    require(set(package) == {"name", "description", "version", "edition", "rust-version", "publish", "license", "repository"}
        and package["name"] == "sail-acceptance"
        and all(package[key] == {"workspace": True} for key in
            ("version", "edition", "rust-version", "publish", "license", "repository")),
        "unexpected executable leaf package")
    workspace = tomllib.loads(snapshots["Cargo.toml"].decode())
    require(not ({"patch", "replace"} & set(workspace)), "workspace dependency overrides require review")
    protected_packages = ("sail-acceptance", "sail-api", "sail-cache", "sail-shim", "sail-verus", "sail-daemon")
    for name in protected_packages:
        require(workspace["workspace"]["dependencies"][name] == {"path": "crates/" + name},
            "workspace protected dependency redirected: " + name)
    api = tomllib.loads(snapshots["crates/sail-api/Cargo.toml"].decode())
    require(set(api) == {"package", "dependencies", "target", "lints"}
        and set(api["package"]) == set(package)
        and api["package"]["name"] == "sail-api"
        and all(api["package"][key] == {"workspace": True} for key in
            ("version", "edition", "rust-version", "publish", "license", "repository")),
        "API target/feature/build assembly changed")
    for name in ("sail-api", "sail-cache", "sail-shim", "sail-verus", "sail-daemon"):
        manifest = tomllib.loads(snapshots["crates/" + name + "/Cargo.toml"].decode())
        require(manifest["dependencies"]["sail-acceptance"] == {"workspace": True}, "runtime dependency redirected")
    protected = {
        "crates/sail-acceptance/src/query.rs": ("Fifo", "BoundPair", "CorrelatedQueue", "ReplayCursor", "PublicationBatch", "RecoveryRecord", "RawTranscript", "PrefixFeed"),
        "crates/sail-acceptance/src/inputs.rs": ("OriginalCapture", "OriginalGuard", "Readback", "ProviderSeal", "OpenedRead", "ReadChunk", "ReadSeal"),
        "crates/sail-api/src/acceptance.rs": ("CompleteReport", "CompleteSelection", "CompleteRequest", "SubmissionBinding", "SubmittedAuthority"),
    }
    for path, names in protected.items():
        stream = SOURCE.tokens(snapshots[path].decode())
        for name in names:
            declarations = [i for i in range(len(stream) - 2)
                if [t.value for t in stream[i:i + 3]] == ["pub", "struct", name]]
            require(len(declarations) == 1, "missing/ambiguous protected token " + name)
            at = declarations[0] + 3
            while at < len(stream) and stream[at].value != "{":
                at += 1
            require(at < len(stream), "missing protected fields")
            end = SOURCE.closing(stream, at)
            fields = stream[at + 1:end]
            require(not any(token.value == "pub" and level
                for token, level in zip(fields, SOURCE.top_level(fields), strict=True)),
                "public authority/state field permits unverified construction: " + name)
            # Deriving a new constructor (Default/Deserialize) would bypass the
            # function inventory and private-field minting boundary.
            previous = declarations[0] - 1
            while previous >= 0 and stream[previous].value == "]":
                start = previous - 1
                depth = 1
                while start >= 0:
                    if stream[start].value == "]": depth += 1
                    elif stream[start].value == "[":
                        depth -= 1
                        if depth == 0: break
                    start -= 1
                require(start > 0 and stream[start - 1].value == "#", "invalid token attribute")
                attribute = [t.value for t in stream[start + 1:previous]]
                require(attribute == ["derive", "(", "Debug", ")"]
                    or attribute and attribute[0] == "doc", "authority type derives unchecked constructor")
                previous = start - 2


def dependency_paths(component: str) -> list[str]:
    """The closed executable/model dependency set for one complete driver."""
    entry = COMPONENTS[component]
    result = [SYNTAX]
    result += ["proofs/acceptance/" + stem + ".rs" for stem in entry["models"]]
    result += ["crates/sail-acceptance/src/" + stem + ".rs" for stem in entry["core"]]
    result.append("proofs/refinement/" + component + ".rs")
    if component == "authority":
        result += ["crates/sail-api/src/acceptance.rs", "crates/sail-api/src/acceptance_sort.rs",
            *["crates/sail-api/src/" + name + ".rs" for name in SOURCE.API_MODULES]]
    return result


def declaration_source(path: str) -> bool:
    """Only the inventoried declarations of these actual API modules compile."""
    return path in {"crates/sail-api/src/" + name + ".rs" for name in SOURCE.API_MODULES}


def compiled_subset(path: str, content: bytes) -> None:
    """Refuse source loading, unchecked specs and alternate executable cfgs."""
    if path == SYNTAX:
        require(hashlib.sha256(content).hexdigest() == SYNTAX_SHA256,
            "syntax erasure implementation changed; explicit runner review required")
        return
    if path.startswith("proofs/acceptance/"):
        COMMON.standalone(content.decode())
        return
    text = content.decode()
    if path in IMPORT_SHA256:
        stream = SOURCE.tokens(text)
        imports = []
        for i, token in enumerate(stream):
            if token.value == "use":
                end = i
                while end < len(stream) and stream[end].value != ";":
                    end += 1
                require(end < len(stream), "unterminated captured import")
                imports.append([item.value for item in stream[i:end + 1]])
        require(hashlib.sha256(json.dumps(imports, separators=(",", ":")).encode()).hexdigest()
            == IMPORT_SHA256[path], "executable/proof import bindings changed; explicit runner review required")
    # Only proof imports may differ between builds. They do not supply an
    # executable branch. Actual code remains in the same source envelopes.
    import re
    text = re.sub(r"#\[cfg\(sail_verus\)\]\s*use\s+(?:vstd::|super::inputs::)[\sA-Za-z_:*,{}]+;", "", text)
    values = [token.value for token in SOURCE.tokens(text)]
    forbidden = {"mod", "extern", "macro_rules", "cfg", "cfg_attr", "assume", "admit", "external_body",
        "external_fn_specification", "assume_specification", "external", "trusted", "axiom"}
    require(not (forbidden & set(values)), "compiled source outside capture subset: " + path)
    loading = {"include", "include_str", "include_bytes", "env", "option_env",
        "file", "line", "column", "module_path"}
    require(not any(value in loading and values[i + 1] == "!"
        for i, value in enumerate(values[:-1])), "environment-dependent macro source: " + path)
    # The untrusted sorter is deliberately a sibling of the private token
    # constructors. Every privileged function must be visible to verification.
    if path.endswith("/acceptance_sort.rs"):
        SOURCE.candidate_forwarders(text)
    else:
        SOURCE.checked_item_envelopes(text, proof=path.startswith("proofs/refinement/"))


def inventory(root: Path, manifest: dict, snapshots: dict[str, bytes]) -> list[dict]:
    """Validate every root, actual source closure and selected negative control."""
    require(manifest.get("schema") == "sail-production-refinements-v1", "unknown refinement manifest")
    entries = manifest.get("components", [])
    require(len(entries) == len(COMPONENTS)
        and {entry["id"] for entry in entries} == set(COMPONENTS), "refinement root omitted or duplicated")
    actual = {str(path.relative_to(root)) for path in (root / "proofs/refinement").rglob("*.rs")}
    require(actual == {"proofs/refinement/" + name + ".rs" for name in COMPONENTS},
        "unlisted or missing refinement source")
    require({str(path.relative_to(root)) for path in (root / "crates/sail-acceptance/src").rglob("*.rs")}
        == {"crates/sail-acceptance/src/" + name + ".rs"
            for name in ("lib", "syntax", "query", "inputs", "authority")},
        "unlisted executable leaf source")
    for entry in entries:
        component = entry["id"]
        require(entry.get("source") == "proofs/refinement/" + component + ".rs", "wrong refinement source")
        require(entry.get("dependencies") == dependency_paths(component), "refinement dependency closure changed")
        require(type(entry.get("verified")) is int and entry["verified"] > 0, "invalid verified count")
        require(bool(entry.get("functions")) and len(entry["functions"]) == len(set(entry["functions"])),
            "missing or duplicate full function inventory")
        require([control["name"] for control in entry.get("negative_controls", [])]
            == REQUIRED_CONTROLS[component], "selected mutation inventory omitted, duplicated or reordered")
        for path in entry["dependencies"]:
            if component == "authority" and declaration_source(path):
                continue  # Only the mechanically selected declarations compile.
            compiled_subset(path, snapshots[path])
        for control in entry["negative_controls"]:
            path = control["source"]
            require(path in entry["dependencies"] and path != SYNTAX,
                "mutation does not target a checked dependency")
            before, after = control["before"].encode(), control["after"].encode()
            require(bool(before) and before != after and snapshots[path].count(before) == 1,
                "empty or ambiguous executable mutation")
            compiled_subset(path, snapshots[path].replace(before, after))
    SOURCE.api_stencils(snapshots, manifest.get("api_declarations"))
    require(manifest.get("api_declarations") is not None, "missing API representation inventory")
    return entries


def captured_driver(entry: dict, snapshots: dict[str, bytes], control: dict | None = None) -> tuple[str, bytes, dict[str, bytes]]:
    """Use fixed module bindings and exact source bytes; never reopen a path."""
    component = entry["id"]
    require(entry["dependencies"] == dependency_paths(component), "unexpected dependency closure")
    sources = {path: snapshots[path] for path in entry["dependencies"]}
    if control is not None:
        path = control["source"]
        before, after = control["before"].encode(), control["after"].encode()
        require(path in sources and sources[path].count(before) == 1 and before != after, "invalid captured mutation")
        sources[path] = sources[path].replace(before, after)
    files = {"syntax.rs": sources[SYNTAX]}
    declarations = ["#![forbid(unsafe_code)]", "#![allow(unused_imports)]",
        "use vstd::prelude::*;", "extern crate self as sail_acceptance;",
        '#[macro_use] #[path="syntax.rs"] mod syntax;']
    for stem in COMPONENTS[component]["core"]:
        filename = "core_" + stem + ".rs"
        files[filename] = sources["crates/sail-acceptance/src/" + stem + ".rs"]
        declarations += ['#[path="' + filename + '"] pub mod ' + stem + ';',
            "pub use " + stem + " as " + stem + "_core;"]
    for stem in COMPONENTS[component]["models"]:
        filename = "model_" + stem + ".rs"
        files[filename] = sources["proofs/acceptance/" + stem + ".rs"]
        name = "authority_model" if stem == "authority" else stem
        declarations.append('#[path="' + filename + '"] pub mod ' + name + ';')
    if component == "authority":
        stencils, _ = SOURCE.api_stencils(sources)
        files.update(stencils)
        files["api_acceptance.rs"] = sources["crates/sail-api/src/acceptance.rs"]
        files["acceptance_sort.rs"] = sources["crates/sail-api/src/acceptance_sort.rs"]
        declarations += ["pub mod " + name + ";" for name in SOURCE.API_MODULES]
        declarations += [
            '#[path="api_acceptance.rs"] pub mod acceptance;', "mod acceptance_sort;"]
    files["refinement.rs"] = sources[entry["source"]]
    declarations += ['#[path="refinement.rs"] pub mod refinement;', "fn main() {}"]
    for path, data in sources.items():
        if not (component == "authority" and declaration_source(path)):
            compiled_subset(path, data)
    return "check_" + component + ".rs", ("\n".join(declarations) + "\n").encode(), files


def captured_tree(root: Path, commit: str, paths: list[str]) -> dict[str, bytes]:
    """Read canonical Git blobs in one batch, with no extraction to disk."""
    require(all("\n" not in path and "\r" not in path for path in paths), "unsupported Git path")
    retained = {"HOME", "PATH", "USER", "SYSTEMROOT", "TMPDIR", "LANG", "LC_ALL"}
    environment = {key: value for key, value in os.environ.items() if key in retained}
    environment.update(GIT_NO_REPLACE_OBJECTS="1", GIT_CONFIG_NOSYSTEM="1",
        GIT_CONFIG_GLOBAL=os.devnull, GIT_OPTIONAL_LOCKS="0")
    data = subprocess.check_output(["git", "--no-replace-objects", "-c", "safe.directory=" + str(root),
        "-c", "core.fsmonitor=false", "-C", str(root), "cat-file", "--batch"],
        input="".join(commit + ":" + path + "\n" for path in paths).encode(), env=environment)
    result: dict[str, bytes] = {}
    at = 0
    for path in paths:
        end = data.find(b"\n", at)
        require(end >= at, "missing Git object header")
        header = data[at:end].split()
        require(len(header) == 3 and header[1] == b"blob" and header[2].isdigit(), "unexpected Git object")
        length = int(header[2])
        begin = end + 1
        at = begin + length
        require(at < len(data) and data[at] == 10, "truncated Git object")
        require(path not in result, "duplicate Git path")
        result[path] = data[begin:at]
        at += 1
    require(at == len(data), "unexpected trailing Git output")
    require(set(result) == set(paths), "source capture omitted a path")
    return result


def working_source_matches(root: Path, snapshots: dict[str, bytes]) -> None:
    """Recheck both content and auto-discovery before and after verification."""
    require(not any((root / "crates" / name / "build.rs").exists()
        for name in ("sail-acceptance", "sail-api")), "unreviewed protected build script")
    expected = {path for path in snapshots if path.startswith("crates/")
        and ("/src/" in path and path.endswith(".rs") or path.endswith("/Cargo.toml"))}
    actual = {str(path.relative_to(root)) for crate in (root / "crates").iterdir() if crate.is_dir()
        for path in list((crate / "src").rglob("*.rs")) + list(crate.glob("Cargo.toml"))}
    require(actual == expected, "working source discovery changed")
    for path, content in snapshots.items():
        relative = Path(path)
        require(not any((root / parent).is_symlink() for parent in [relative, *relative.parents]),
            "symlink in captured source path")
        require((root / path).is_file() and (root / path).read_bytes() == content,
            "working source differs from Git capture")


def selected_entries(entries: list[dict], component: str | None) -> list[dict]:
    """Select one complete root only after validating the full root inventory."""
    require(len(entries) == len(COMPONENTS)
        and {entry["id"] for entry in entries} == set(COMPONENTS),
        "refinement root omitted or duplicated")
    require(component is None or component in COMPONENTS, "unknown refinement component")
    return entries if component is None else [entry for entry in entries if entry["id"] == component]


def check(root: Path, archive: Path, install: Path, out: Path, timeout: int,
        component: str | None = None) -> None:
    """Qualify every selected actual-body proof and executable guard mutation."""
    started = time.monotonic()
    require(not out.is_relative_to(root.resolve()), "put raw refinement evidence outside checkout")
    commit = COMMON.git(root, "rev-parse", "HEAD")
    all_paths = COMMON.git(root, "ls-tree", "-r", "--name-only", commit).splitlines()
    paths = [path for path in all_paths if (
        path.startswith("crates/") and ("/src/" in path and path.endswith(".rs") or path.endswith("/Cargo.toml"))
        or path.startswith("proofs/") and path.endswith((".rs", ".json"))
        or path in {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "corpus/toolchain.toml", "xtask/src/layering.rs",
            "scripts/check-acceptance-proofs.py", "scripts/check-acceptance-refinements.py",
            "scripts/acceptance-source.py"})]
    require(MANIFEST in paths, "refinement manifest is not committed")
    snapshots = captured_tree(root, commit, paths)
    working_source_matches(root, snapshots)
    validate_assembly(snapshots)
    require(not COMMON.git(root, "status", "--porcelain=v1", "--", *paths), "refinement source is uncommitted")
    manifest = json.loads(snapshots[MANIFEST], object_pairs_hook=COMMON.unique_object)
    entries = selected_entries(inventory(root, manifest, snapshots), component)
    pins = tomllib.loads(snapshots["corpus/toolchain.toml"].decode())
    hosts = {("Darwin", "arm64"): "aarch64-apple-darwin", ("Linux", "x86_64"): "x86_64-unknown-linux-gnu"}
    host = hosts.get((platform.system(), platform.machine()))
    require(host is not None, "unsupported direct refinement platform")
    asset = pins["assets"][host]
    runtime = COMMON.validated_runtime(archive, install, asset)
    environment = COMMON.proof_environment(install, pins["verus"]["rust_toolchain"])
    rustc = subprocess.check_output(["rustup", "run", pins["verus"]["rust_toolchain"], "rustc", "-vV"], env=environment, text=True)
    require("commit-hash: " + pins["verus"]["rust_commit"] in rustc.splitlines(), "Rust compiler identity mismatch")
    out.mkdir(parents=True, exist_ok=False)
    results = []
    for entry in entries:
        for index, control in enumerate([None, *entry["negative_controls"]]):
            label = entry["id"] + "-" + str(index)
            name = "positive" if control is None else control["name"]
            driver, data, files = captured_driver(entry, snapshots, control)
            with tempfile.TemporaryDirectory(prefix="sail-stock-refinement-") as temporary:
                scratch = Path(temporary)
                for filename, content in files.items():
                    (scratch / filename).write_bytes(content)
                (scratch / driver).write_bytes(data)
                command = [str(install / "verus"), str(scratch / driver), "--cfg", "sail_verus",
                    "--no-cheating", "--output-json", "--num-threads", "1"]
                begin = time.monotonic()
                try:
                    result = subprocess.run(command, cwd=scratch, env=environment, text=True,
                        capture_output=True, timeout=timeout)
                except subprocess.TimeoutExpired as error:
                    for suffix, value in (("stdout", error.stdout), ("stderr", error.stderr)):
                        (out / (label + "." + suffix)).write_text(value.decode(errors="replace")
                            if isinstance(value, bytes) else value or "")
                    raise Refusal("refinement timeout in " + label) from error
            (out / (label + ".stdout")).write_text(result.stdout)
            (out / (label + ".stderr")).write_text(result.stderr)
            require(not COMMON.resource_failure(result.stdout + result.stderr), "refinement resource limit reached")
            document = COMMON.checked_document(result.stdout, result.returncode, entry, pins, broken=control is not None)
            results.append({"component": entry["id"], "control": name, "negative": control is not None,
                "seconds": time.monotonic() - begin, "exit": result.returncode,
                "driver_sha256": hashlib.sha256(data).hexdigest(),
                "compiled_source_sha256": {path: hashlib.sha256(value).hexdigest() for path, value in files.items()},
                "verification": document["verification-results"], "command": command})
            print(entry["id"] + ": " + name + " passed", flush=True)
    require(COMMON.git(root, "rev-parse", "HEAD") == commit, "checked commit changed")
    working_source_matches(root, snapshots)
    inventory(root, manifest, snapshots)
    require(COMMON.validated_runtime(archive, install, asset) == runtime, "stock runtime changed")
    report = {"commit": commit, "source_sha256": {path: hashlib.sha256(value).hexdigest()
        for path, value in snapshots.items()}, "stock_archive_sha256": asset["sha256"],
        "runtime_sha256": runtime, "rustc": rustc, "cases": results,
        "api_declarations": manifest["api_declarations"],
        "validated_components": list(COMPONENTS),
        "checked_components": [entry["id"] for entry in entries],
        "complete_suite": component is None,
        "source_roles": {entry["id"]: {
            "checked_source": [path for path in entry["dependencies"] if path not in
                {SYNTAX, "crates/sail-api/src/acceptance_sort.rs"} and not declaration_source(path)],
            "reviewed_syntax": [SYNTAX],
            "declaration_projections": [path for path in entry["dependencies"]
                if declaration_source(path)],
            "structurally_checked_forwarders": [path for path in entry["dependencies"]
                if path.endswith("/acceptance_sort.rs")],
        } for entry in entries},
        "bound_runtime_source_is_not_all_verified": True,
        "total_seconds": time.monotonic() - started,
        "scope": "checked executable bodies and component refinements; surrounding I/O, origin and service composition remain explicit obligations"}
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    (out / "report.md").write_text("# Production acceptance refinements\n\nCommit `" + commit
        + "`. All " + str(len(results)) + " selected actual-body checks pass through direct stock Verus.\n\n"
        + report["scope"] + ". Runtime files bound by the receipt are not all verified bodies.\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stock-archive", type=Path, required=True)
    parser.add_argument("--stock-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--component", choices=tuple(COMPONENTS),
        help="check one complete root and all its controls; default checks every root")
    args = parser.parse_args()
    try:
        require(args.timeout > 0, "timeout must be positive")
        check(ROOT, args.stock_archive.resolve(), args.stock_dir.resolve(), args.out.resolve(), args.timeout, args.component)
    except (Refusal, SOURCE.SourceRefusal, OSError, ValueError, KeyError, TypeError,
            subprocess.SubprocessError, zipfile.BadZipFile) as error:
        print("error[SAIL9002]: production refinement check refused: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
