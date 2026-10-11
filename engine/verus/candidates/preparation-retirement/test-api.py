#!/usr/bin/env python3
"""Pinned helper controls and a finish-removed sensitivity; no proof or Cargo."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument("--work-dir", type=Path, required=True)
args = parser.parse_args()
base = Path(__file__).resolve().parent
work = args.work_dir.resolve()
work.mkdir(parents=True, exist_ok=False)
env = {key: value for key, value in os.environ.items() if not key.startswith("SAIL_ENGINE")}
env["RUSTC_BOOTSTRAP"] = "1"
sysroot = subprocess.check_output(["rustc", "+1.98.1", "--print", "sysroot"], text=True).strip()
env["DYLD_LIBRARY_PATH"] = str(Path(sysroot) / "lib")
targetlib = Path(subprocess.check_output(["rustc", "+1.98.1", "--print", "target-libdir"], text=True).strip())
std = list(targetlib.glob("libstd-*.rmeta"))
if len(std) != 1:
    raise ValueError("require one exact pinned std metadata root")
metadata = subprocess.check_output(["rustc", "+1.98.1", "-Z", "ls=root", str(std[0])], text=True, env=env)
(work / "pinned-std-dependencies.txt").write_text(metadata)
matches = re.findall(r"^\d+ (libc-[0-9a-f]+) hash ([0-9a-f]+) ", metadata, re.M)
if len(matches) != 1:
    raise ValueError("require exact std libc dependency identity")
libc = targetlib / ("lib" + matches[0][0] + ".rmeta")
if not libc.is_file():
    raise ValueError("matching std libc full metadata absent")
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
records = []
for arm in ["candidate", "finish-removed"]:
    root = work / arm
    root.mkdir()
    for name in ["namespace.rs", "tests.rs"]:
        shutil.copyfile(base / name, root / name)
    if arm == "finish-removed":
        path = root / "namespace.rs"
        source = path.read_text()
        begin = source.index("    pub(super) fn finish(&self) {")
        end = source.index("\n}\nimpl Drop for Lease", begin)
        source = source[:begin] + """    pub(super) fn finish(&self) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.requester = false;
        }
        self.shared.changed.notify_all();
    }
""".rstrip() + source[end:]
        path.write_text(source)
    binary = root / "probe"
    commands = [
        ("compile", ["rustc", "+1.98.1", "--edition=2024", "--test", "-C", "prefer-dynamic", "--extern", "libc=" + str(libc), str(root / "tests.rs"), "-o", str(binary)], 60),
        ("tests", [str(binary), "--test-threads=1"] if arm == "candidate" else [str(binary), "namespace::retirement_tests::normal_finish_waits_for_actual_compiler_guard_and_owner_retirement", "--exact", "--test-threads=1"], 30),
    ]
    for name, command, limit in commands:
        start = time.monotonic()
        with (root / (name + ".stdout")).open("wb") as out, (root / (name + ".stderr")).open("wb") as err:
            child = subprocess.Popen(command, stdout=out, stderr=err, env=env, start_new_session=True)
            timed_out = False
            try:
                code = child.wait(timeout=limit)
            except subprocess.TimeoutExpired:
                timed_out = True
                os.killpg(child.pid, signal.SIGKILL)
                code = child.wait()
        record = {"arm": arm, "phase": name, "argv": command, "exit_code": code, "elapsed_seconds": time.monotonic() - start, "deadline_seconds": limit, "timed_out": timed_out}
        records.append(record)
        if timed_out or (name == "compile" and code) or (arm == "candidate" and code):
            break
    if records[-1]["phase"] != "tests":
        break
candidate = [r for r in records if r["arm"] == "candidate"]
negative = [r for r in records if r["arm"] == "finish-removed"]
sensitive = False
if len(negative) == 2 and negative[-1]["exit_code"] == 101 and not negative[-1]["timed_out"]:
    output = (work / "finish-removed/tests.stdout").read_text() + (work / "finish-removed/tests.stderr").read_text()
    sensitive = "normal_finish_waits_for_actual_compiler_guard_and_owner_retirement" in output and "left: 0" in output and "right: 16384" in output
passed = len(candidate) == 2 and all(r["exit_code"] == 0 and not r["timed_out"] for r in candidate) and sensitive
summary = {"version": 1, "scope": "actual pinned helper and SourceMap controls plus finish-removed counterfactual; full source integration/runtime/qualification pending", "status": "PASS" if passed else "FAIL", "records": records, "sensitivity_expected_failure_observed": sensitive, "pinned_std_libc": {"name": libc.name, "metadata_sha256": sha(libc), "crate_hash": matches[0][1], "std_metadata_sha256": sha(std[0])}, "raw": {str(path.relative_to(work)): {"bytes": path.stat().st_size, "sha256": sha(path)} for path in work.rglob("*") if path.is_file()}}
(work / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps({"status": summary["status"], "records": records, "sensitive": sensitive}))
raise SystemExit(0 if passed else 1)
