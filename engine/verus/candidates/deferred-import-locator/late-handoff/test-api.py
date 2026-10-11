#!/usr/bin/env python3
"""Compile the late-handoff helper against actual pinned rustc source-map types.

Extracts the stock private resolver types/methods unchanged from --source.
This is a helper/API test, not a full source build or proof qualification.
"""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()



def pinned_libraries(deps):
    fingerprints = deps.parent / ".fingerprint"
    roots = list(fingerprints.glob("rust_verify-*/bin-rust_verify.json"))
    if len(roots) != 1:
        raise ValueError("require one actual pinned rust_verify binary fingerprint")
    root = json.loads(roots[0].read_text())
    fingerprints_used = {str(roots[0].relative_to(fingerprints)): sha(roots[0])}
    libraries = {}
    documents = {}
    for name in ("serde", "bincode_next"):
        requested = [item[3] for item in root["deps"] if item[1] == name]
        if len(requested) != 1:
            raise ValueError(f"missing or ambiguous pinned {name} dependency")
        package = name.replace("_", "-")
        matches = []
        for path in fingerprints.glob(f"{package}-*/lib-{name}"):
            raw = bytes.fromhex(path.read_text().strip())
            if len(raw) == 8 and int.from_bytes(raw, "little") == requested[0]:
                matches.append(path)
        if len(matches) != 1:
            raise ValueError(f"require unique actual fingerprint for {name}")
        identity = matches[0]
        document_path = identity.with_name(identity.name + ".json")
        document = json.loads(document_path.read_text())
        if document["rustc"] != root["rustc"] or document["rustflags"] != root["rustflags"]:
            raise ValueError("pinned compiler/flags mismatch")
        suffix = identity.parent.name.removeprefix(package + "-")
        library = deps / f"lib{name}-{suffix}.rlib"
        if not library.is_file():
            raise ValueError("actual pinned rlib unavailable")
        libraries[name] = library
        documents[name] = document
        fingerprints_used[str(identity.relative_to(fingerprints))] = sha(identity)
        fingerprints_used[str(document_path.relative_to(fingerprints))] = sha(document_path)
    serde_from_bincode = [item[3] for item in documents["bincode_next"]["deps"] if item[1] == "serde"]
    serde_from_driver = [item[3] for item in root["deps"] if item[1] == "serde"]
    if serde_from_bincode != serde_from_driver:
        raise ValueError("bincode serde crate identity differs from actual driver")
    return libraries, {"fingerprint_sha256": fingerprints_used,
                       "root_features": root["features"], "rustflags": root["rustflags"],
                       "root_profile": root["profile"], "dependency_profiles": {name: document["profile"] for name, document in documents.items()},
                       "dependency_features": {name: document["features"] for name, document in documents.items()}, "compiler_identity": root["rustc"],
                       "library_sha256": {p.name: sha(p) for p in libraries.values()}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--work-dir", required=True, type=Path)
    parser.add_argument("--vstd-metadata", type=Path)
    parser.add_argument("--deps", type=Path, help="exact pinned source target deps; required for actual metadata type decode")
    args = parser.parse_args()
    base = Path(__file__).resolve().parent
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    source = args.source / "rust_verify/src/spans.rs"
    text = source.read_text()
    types = text[text.index("#[derive(Debug, Clone)]\nenum ExternSourceInfo"):text.index("pub(crate) type SpanContext")]
    metadata_facts = None
    dependency_facts = None
    external = []
    metadata_code = ""
    if args.vstd_metadata:
        if args.deps is None:
            parser.error("--vstd-metadata requires --deps")
        spec = importlib.util.spec_from_file_location("sail_metadata_input", base / "metadata-input.py")
        preflight = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(preflight)
        suffix, metadata_facts = preflight.bounded_input(args.vstd_metadata)
        (work / "vstd.metadata-suffix").write_bytes(suffix)
        (work / "metadata-preflight.json").write_text(json.dumps(metadata_facts, indent=2) + "\n")
        libraries, dependency_facts = pinned_libraries(args.deps)
        for name, library in libraries.items():
            external += ["--extern", f"{name}={library}"]
        external += ["-L", f"dependency={args.deps}", "--cfg", "sail_vstd_metadata"]
        export = (args.source / "rust_verify/src/import_export.rs").read_text()
        metadata_type = export[export.index("#[derive(Debug, Deserialize, Serialize)]"):export.index("#[derive(Debug, Deserialize, Serialize)]", export.index("pub(crate) struct CrateMetadata"))]
        # This exact loop from the pinned constructor generates remaining-file
        # recipes; the diagnostic fixture supplies an EMPTY current namespace.
        remaining = text[text.index("        for (imported_crate, files) in remaining_crate_files.iter()"):text.index("        // Only optional interactive preparation")]
        metadata_code = "use serde::{Deserialize, Serialize};\n" + metadata_type + "\nfn remaining_original_descriptors(remaining_crate_files: HashMap<u64, HashMap<Vec<u8>,FileStartEndPos>>) -> HashMap<u64,CrateInfo> {let mut imported_crates=HashMap::new();\n" + remaining + "\nimported_crates}\n"
    else:
        types = types.replace("#[derive(Clone, Debug, Deserialize, Serialize)]", "#[derive(Clone, Debug)]")
    ranges = text[text.index("    fn pos_to_extern_source_file("):text.index("    fn pack_span(")]
    unpack = text[text.index("    fn unpack_span("):text.index("    pub(crate) fn get_next_span_id(")]
    from_air = text[text.index("    pub(crate) fn from_air_span("):text.index("    pub(crate) fn spanned_new<")]
    from_raw = text[text.index("pub(crate) fn from_raw_span("):text.index("// Note: this produces a span")]
    messages = (args.source / "vir/src/messages.rs").read_text()
    span_type = messages[messages.index("pub type RawSpan ="):messages.index("// Private prepared-artifact span binding")]

    probe = """#![feature(rustc_private)]
#![allow(dead_code)]
extern crate rustc_driver;
extern crate rustc_span;
extern crate rustc_data_structures;
extern crate rustc_middle;
mod verifier {pub fn sail_note_import_charge() {}}
mod vir { pub mod messages { use std::sync::Arc;
""" + span_type + """
}}
mod spans {
use rustc_span::{BytePos, FileName, Span, SpanData};
use rustc_span::source_map::SourceMap;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::vir;

""" + metadata_code + types + "\nstruct SpanContextX { local_crate: rustc_span::def_id::StableCrateId, next_span_id:std::sync::atomic::AtomicU64, local_files: HashMap<Vec<u8>,FileStartEndPos>, sail_imports:Mutex<Option<Result<sail_owned_imports::PendingImportSeeds,sail_owned_imports::CaptureFailure>>>, imported_crates: HashMap<u64, CrateInfo> }\nimpl SpanContextX {\n" + text[text.index("    /// ONE compiler-thread handoff"):text.index("    fn pos_to_extern_source_file(")] + ranges + unpack + from_air + "}\n" + from_raw
    probe += f"#[path={json.dumps(str(base / 'capture.rs'))}] mod sail_owned_imports;\n"
    probe += f"#[path={json.dumps(str(base / 'existing-controls.rs'))}] mod existing;\n"
    probe += f"#[path={json.dumps(str(base / 'tests.rs'))}] mod tests;\n}}\n"
    (work / "probe.rs").write_text(probe)
    environment = dict(os.environ, RUSTC_BOOTSTRAP="1")
    compiler = ["rustc", "+1.98.1"]
    sysroot = Path(subprocess.check_output(compiler + ["--print", "sysroot"], text=True).strip())
    version = subprocess.check_output(compiler + ["-vV"], text=True)
    host = next(line.removeprefix("host: ") for line in version.splitlines() if line.startswith("host: "))
    directories = [sysroot / "lib", sysroot / "lib/rustlib" / host / "lib"]
    command = compiler + ["--edition=2024", "--test", str(work / "probe.rs"), "-C", "prefer-dynamic", "-o", str(work / "probe")] + external
    if args.vstd_metadata:
        environment["SAIL_TEST_VSTD_METADATA_SUFFIX"] = str(work / "vstd.metadata-suffix")
    started = time.monotonic()
    built = subprocess.run(command, env=environment, capture_output=True, timeout=60)
    (work / "compile.stdout").write_bytes(built.stdout)
    (work / "compile.stderr").write_bytes(built.stderr)
    result = {"version": 1, "scope": "Late handoff and unchanged OriginalDescriptor/Deferred/Loaded controls; actual pinned types/resolver, optional sealed metadata fixture with empty Foreign namespace; no complete-source/proof qualification", "compiler": version, "stock_spans_sha256": sha(source), "capture_sha256": sha(base / "capture.rs"), "tests_sha256": sha(base / "tests.rs"), "existing_controls_sha256": sha(base / "existing-controls.rs"), "compile_exit_code": built.returncode}
    result["metadata_preflight"] = metadata_facts
    result["actual_metadata_control"] = bool(args.vstd_metadata)
    if args.vstd_metadata:
        result["metadata_control_sha256"] = sha(base / "metadata-control.rs")
        result["pinned_import_export_sha256"] = sha(args.source / "rust_verify/src/import_export.rs")
        result["pinned_dependency_mapping"] = dependency_facts
    if built.returncode == 0:
        environment["DYLD_LIBRARY_PATH" if "apple" in host else "LD_LIBRARY_PATH"] = ":".join(map(str, directories))
        tested = subprocess.run([str(work / "probe"), "--test-threads=1", "--nocapture"], env=environment, capture_output=True, timeout=30)
        (work / "tests.stdout").write_bytes(tested.stdout)
        (work / "tests.stderr").write_bytes(tested.stderr)
        result.update(test_exit_code=tested.returncode, probe_sha256=sha(work / "probe"))
    if args.vstd_metadata:
        result["vstd_metadata_post_sha256"] = sha(args.vstd_metadata)
        if result["vstd_metadata_post_sha256"] != metadata_facts["input_sha256"]:
            raise ValueError("sealed vstd metadata changed during control")
    result["elapsed_seconds"] = time.monotonic() - started
    (work / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    return result.get("test_exit_code", built.returncode)


if __name__ == "__main__":
    import sys
    import traceback
    try:
        raise SystemExit(main())
    except Exception as error:
        # Keep setup failure distinct from compile/test results, including the
        # sealed input after hash; never turn an exception into a positive run.
        if "--work-dir" in sys.argv:
            work = Path(sys.argv[sys.argv.index("--work-dir") + 1])
            work.mkdir(parents=True, exist_ok=True)
            failure = {"scope": "metadata/helper setup or invocation failure; no eligibility/qualification credit", "error_type": type(error).__name__, "error": str(error)}
            if "--vstd-metadata" in sys.argv:
                path = Path(sys.argv[sys.argv.index("--vstd-metadata") + 1])
                failure["vstd_metadata_post_sha256"] = sha(path)
            (work / "failure.json").write_text(json.dumps(failure, indent=2) + "\n")
            (work / "failure.stderr").write_text(traceback.format_exc())
        raise
