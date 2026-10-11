#!/usr/bin/env python3
"""Compile the inert helper against actual pinned rustc source-map types.

Extracts the stock private resolver types/methods unchanged from --source.
This is a helper/API test, not a full source build or proof qualification.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--work-dir", required=True, type=Path)
    args = parser.parse_args()
    base = Path(__file__).resolve().parent
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    source = args.source / "rust_verify/src/spans.rs"
    text = source.read_text()
    types = text[text.index("#[derive(Debug, Clone)]\nenum ExternSourceInfo"):text.index("pub(crate) type SpanContext")]
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
""" + types + "\nstruct SpanContextX { local_crate: rustc_span::def_id::StableCrateId, next_span_id:std::sync::atomic::AtomicU64, local_files: HashMap<Vec<u8>,FileStartEndPos>, sail_imports:Mutex<Option<Result<sail_owned_imports::OwnedImportMappings,sail_owned_imports::CaptureFailure>>>, imported_crates: HashMap<u64, CrateInfo> }\nimpl SpanContextX {\n" + ranges + unpack + from_air + "}\n" + from_raw
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
    command = compiler + ["--edition=2024", "--test", str(work / "probe.rs"), "-C", "prefer-dynamic", "-o", str(work / "probe")]
    started = time.monotonic()
    built = subprocess.run(command, env=environment, capture_output=True, timeout=60)
    (work / "compile.stdout").write_bytes(built.stdout)
    (work / "compile.stderr").write_bytes(built.stderr)
    result = {"version": 1, "scope": "Deferred locator and unchanged Loaded controls; actual pinned types and extracted stock resolver, not source/proof qualification", "compiler": version, "stock_spans_sha256": sha(source), "capture_sha256": sha(base / "capture.rs"), "tests_sha256": sha(base / "tests.rs"), "existing_controls_sha256": sha(base / "existing-controls.rs"), "compile_exit_code": built.returncode}
    if built.returncode == 0:
        environment["DYLD_LIBRARY_PATH" if "apple" in host else "LD_LIBRARY_PATH"] = ":".join(map(str, directories))
        tested = subprocess.run([str(work / "probe"), "--test-threads=1", "--nocapture"], env=environment, capture_output=True, timeout=30)
        (work / "tests.stdout").write_bytes(tested.stdout)
        (work / "tests.stderr").write_bytes(tested.stderr)
        result.update(test_exit_code=tested.returncode, probe_sha256=sha(work / "probe"))
    result["elapsed_seconds"] = time.monotonic() - started
    (work / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    return result.get("test_exit_code", built.returncode)


if __name__ == "__main__":
    raise SystemExit(main())
