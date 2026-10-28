#!/usr/bin/env python3
"""Local-only real-tool regressions for compiler-read origin; never a benchmark."""

import argparse
import copy
import difflib
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import re
import select
import shutil
import signal
import subprocess
import sys
import time
import tomllib
import traceback

COMMON = " __VERUS_DRIVER_ARGS__"
SHADOW = " __VERUS_DRIVER_SHADOW__"
READS = (COMMON, "SAIL_PROVENANCE_DIR", "SAIL_COMPILER_INPUTS", "SAIL_COMPILER_INPUTS_BINDING")
OVERLAYS = (*READS, "SAIL_ENGINE", "SAIL_ENGINE_EVENTS", "SAIL_ENGINE_CONTEXT",
            "SAIL_ENGINE_FOCUS_REPORT", "SAIL_ENGINE_FUNCTION",
            "SAIL_RUSTC_DEV_SOURCE_BASE", "SAIL_VIR_SPAN_REMAP")
BASE = "use vstd::prelude::*;\nverus! { proof fn origin_lemma() { assert(true); } }\n"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write(path, document):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")


def git(root, *args):
    env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    return subprocess.check_output(["git", "--no-replace-objects", "-c", "safe.directory=" + str(root.resolve()), "-c", "core.fsmonitor=false", *args],
                                   cwd=root, env=env)


def source_inventory(root):
    require(not git(root, "status", "--porcelain").strip(), "qualification source must be clean")
    return {name: digest(root / name) for name in git(root, "ls-files", "-z").decode().split("\0") if name}


def runtime_inventory(root):
    paths = sorted(root.rglob("*"))
    require(not any(path.is_symlink() and path.is_dir() for path in paths),
            "runtime directory symlinks need a separately captured closed inventory")
    files = [path for path in paths if path.is_file()]
    require(files, "empty runtime inventory")
    return {str(path.relative_to(root)): {"sha256": digest(path),
            "symlink": os.readlink(path) if path.is_symlink() else None} for path in files}


def require_compiler_restore_report(actual, original, solver_version):
    """Check ADR 0050 restoration with precisely unavailable attribution."""
    require(original["mode"] == "default" and original["solver"]["version"] == solver_version,
            "retained A1 did not record the pinned default solver")
    expected = copy.deepcopy(original)
    functions = [function for crate in expected["crates"] for function in crate["functions"]]
    require(functions, "retained A1 has no functions")
    expected["attribution"]["notes"] = [
        f"{len(functions)} functions have no attributed obligations (their logs are missing, overwritten, or ambiguous)"]
    require(actual["attribution"] == {"status": "partial", "unmatched_sessions": 0,
                                     "notes": expected["attribution"]["notes"]},
            "retained A2 has unexpected missing-log attribution")
    expected["solver"]["version"] = None
    for function in functions:
        function["obligations"] = None
        function["provenance"]["source"] = "unrecorded"
        function["provenance"]["checks_solved"] = 0
        function["provenance"]["checks_served"] = 0
    for crate in actual["crates"]:
        for function in crate["functions"]:
            require(function["obligations"] is None and function["provenance"] == {
                "mode": "default", "source": "unrecorded", "checks_solved": 0,
                "checks_served": 0, "checks_unrecorded": 0},
                "retained A2 fabricated current obligation or solver provenance")
    # Everything else, including all verdicts, diagnostics, function identity,
    # establishment, timing, command and solver SHA, stays literally equal.
    require(actual == expected, "retained A2 changed a field outside the explicit attribution reconstruction")


class Qualification:
    def __init__(self, args):
        self.args = args
        self.repo = Path(__file__).resolve().parents[1]
        self.out = args.out.resolve()
        require(not self.out.exists(), "evidence directory must be new")
        require(not self.out.is_relative_to(self.repo), "raw evidence must remain outside the repository")
        self.out.mkdir(parents=True, mode=0o700)
        self.project = self.out / "work/origin_fixture"
        self.cache = self.out / "work/cache"
        self.socket = self.out / "work/d.sock"
        self.env = dict(os.environ, SAIL_CACHE_DIR=str(self.cache),
                        SAIL_DAEMON_SOCKET=str(self.socket), SAIL_VERUS_BUILD=str(args.build),
                        VERUS_Z3_PATH=str(args.runtime_root / "z3"),
                        NO_COLOR="1", CARGO_TERM_COLOR="never")
        for name in (*OVERLAYS, "SAIL_MANAGED_FORCE", "SAIL_JOB_EVENTS", "SAIL_JOB_GUARD_PARENT",
                     "SAIL_PROOF_EVENTS", "SAIL_VSTD_BOUNDARY_RESEARCH", "SAIL_RETAINED_CONTENT_RESEARCH",
                     "SAIL_CARGO_VERUS", "SAIL_VERUS_RELEASE", "SAIL_VERUS_SOURCE_BUILD"):
            self.env.pop(name, None)
        self.calls = []
        self.cases = []
        self.identity_ready = False
        self.daemon = None
        self.daemon_log = None
        self.started = time.monotonic()
        self.report = {"kind": "sail-compiler-origin-qualification", "schema": 1,
                       "requested_scope": args.scope,
                       "implemented_scope_complete": False, "overall_qualification_complete": False,
                       "failure": None, "final_source_runtime_checked": False, "calls": self.calls, "cases": self.cases,
                       "unrun_required_overall": [
                           {"case": "actual owned compiler interruption and restart",
                            "reason": "requires a separately completed lifecycle packet; basic scope does not execute it; partial progress appears in cases"},
                           {"case": "retained A-to-B-to-A restoration with paired runtime",
                            "reason": "requires a separately completed lifecycle packet; basic scope does not execute it; partial progress appears in cases"},
                       ],
                       "scope": "Warm ordinary CLI/job/MCP reuse; four tracked settings in ghost/exec phases, mode changes and removal; provenance mode-flip and ghost-receipt noop semantic failures; legacy/unknown origin cleanup. Not full runtime acceptance, conformance, a service theorem or a performance measurement."}

    def save(self):
        self.report["seconds"] = time.monotonic() - self.started
        write(self.out / "report.json", self.report)

    def capture_identity(self):
        self.head = git(self.repo, "rev-parse", "HEAD").decode().strip()
        self.sources = source_inventory(self.repo)
        self.runtime = runtime_inventory(self.args.build)
        self.pins = tomllib.loads((self.repo / "corpus/toolchain.toml").read_text())
        asset = {("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
                 ("Darwin", "arm64"): "aarch64-apple-darwin"}.get((platform.system(), platform.machine()))
        require(asset in self.pins["assets"], "no pinned solver asset for this execution platform")
        self.fork = tomllib.loads((self.repo / "engine/verus/fork.toml").read_text())["fork"]
        self.manifest = json.loads((self.args.build / "sail-build.json").read_text())
        self.manifest_sha256 = digest(self.args.build / "sail-build.json")
        self.artifact_pin = self.repo / "engine/verus" / ("artifact-linux.json" if platform.system() == "Linux" else "artifact-macos.json")
        artifact = json.loads(self.artifact_pin.read_text())
        source_lock = digest(self.repo / "engine/verus/Cargo.lock")
        features = ["sail-fork:" + self.fork["commit"], "host:" + asset,
                    "source-lock:" + source_lock, "vstd-build:cargo-verus-release"]
        require(self.fork["upstream"] == self.pins["verus"]["commit"], "fork/upstream pin disagreement")
        require(self.manifest.get("schema_version") == 1 and self.manifest.get("commit") == self.pins["verus"]["commit"]
                and self.manifest.get("rust_toolchain") == self.pins["verus"]["rust_toolchain"]
                and self.manifest.get("features") == features, "installed variant does not match source pins")
        require(artifact.get("manifest_sha256") == self.manifest_sha256 and artifact.get("fork_commit") == self.fork["commit"]
                and artifact.get("source_lock_sha256") == source_lock and artifact.get("schema_version") == 2
                and artifact.get("commit") == self.pins["verus"]["commit"]
                and artifact.get("rust_toolchain") == self.pins["verus"]["rust_toolchain"]
                and artifact.get("z3_sha256") == self.pins["assets"][asset]["z3_sha256"]
                and artifact.get("verus_features") == [], "installed variant differs from staged platform artifact pin")
        runtime_artifacts = {name: item["sha256"] for name, item in self.runtime.items()
                             if Path(name).parent == Path("verus/source/target-verus/release")}
        require(self.manifest.get("artifacts") == runtime_artifacts, "installed manifest omits or misidentifies runtime files")
        require(digest(self.args.runtime_root / "z3") == self.pins["assets"][asset]["z3_sha256"], "candidate does not use pinned Z3")
        self.env["RUSTUP_TOOLCHAIN"] = self.pins["verus"]["rust_toolchain"]
        for name in ("cargo-verus", "verus", "rust_verify", "z3"):
            member = self.args.runtime_root / name
            require(member.is_file() and member.resolve().parent == self.args.runtime_root,
                    f"missing or external runtime executable {name}")
        require(self.args.cargo_verus == (self.args.runtime_root / "cargo-verus").resolve(),
                "cargo-verus must be the inventoried runtime member")
        host = json.loads(self.args.host_record.read_text())
        require(host.get("native") is True and host.get("orchestration"),
                "host record must explicitly attest native execution and identify its orchestration")
        require(host.get("system") == platform.system() and host.get("machine") == platform.machine(),
                "host record does not match the execution platform; platform detection alone is not native evidence")
        build = json.loads(self.args.build_receipt.read_text())
        require(re.fullmatch(r"[0-9a-f]{40}", build.get("sail_source_commit", "")), "missing Sail build source")
        require(build.get("verus_source_commit") == self.fork["commit"], "Verus build receipt differs from fork pin")
        require(build.get("sail_sha256") == digest(self.args.sail), "Sail build receipt digest mismatch")
        require(build.get("shim_sha256") == digest(self.args.sail.with_name("sail-z3")), "shim build receipt digest mismatch")
        require(build.get("runtime_sha256") == {name: item["sha256"] for name, item in self.runtime.items()},
                "runtime build receipt inventory mismatch")
        # The owning build receipt supplies binary/source provenance. Independently
        # require every compiled repository input to match its canonical source.
        for name in self.sources:
            if name.startswith(("crates/", "xtask/", "engine/")) or name in (
                    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "corpus/toolchain.toml"):
                actual = (self.repo / name).read_bytes()
                expected = git(self.repo, "show", build["sail_source_commit"] + ":" + name)
                require(actual == expected, f"compiled input differs from build receipt: {name}")
        for label, path in (("host-record", self.args.host_record), ("build-receipt", self.args.build_receipt)):
            (self.out / (label + ".json")).write_bytes(path.read_bytes())
        (self.out / "sail-build.json").write_bytes((self.args.build / "sail-build.json").read_bytes())
        (self.out / "platform-artifact.json").write_bytes(self.artifact_pin.read_bytes())
        (self.out / "fork.toml").write_bytes((self.repo / "engine/verus/fork.toml").read_bytes())
        self.report.update(commit=self.head, source_sha256=self.sources,
                           sail_sha256=digest(self.args.sail), shim_sha256=digest(self.args.sail.with_name("sail-z3")),
                           runtime_root=str(self.args.runtime_root), installed_build=str(self.args.build), runtime_files=self.runtime,
                           installed_manifest_sha256=self.manifest_sha256, platform_artifact_sha256=digest(self.artifact_pin),
                           build_source=build["sail_source_commit"], verus_source=build["verus_source_commit"],
                           host_record_sha256=digest(self.args.host_record), build_receipt_sha256=digest(self.args.build_receipt),
                           runtime_selection="normal validated SAIL_VERUS_BUILD installed variant; no cargo-verus development override")
        status = json.loads(self.call("validated-engine-status", [str(self.args.sail), "engine", "status", "--output", "json"], self.repo))
        require(status.get("fork_commit") == self.fork["commit"] and status.get("manifest_sha256") == self.manifest_sha256
                and Path(status["build"]).resolve() == self.args.build and Path(status["verus"]).resolve() == (self.args.runtime_root / "verus").resolve(),
                "Sail did not select and validate the intended installed variant")
        self.verus_identity = self.call("verus-version", [str(self.args.runtime_root / "verus"), "--version", "--output-json"], self.repo)
        require(json.loads(self.verus_identity)["verus"]["commit"] == build["verus_source_commit"], "actual verifier full commit differs from build receipt")
        self.rustc_identity = self.call("rustc-version", ["rustup", "run", self.env["RUSTUP_TOOLCHAIN"], "rustc", "-vV"], self.repo)
        require(("commit-hash: " + self.pins["verus"]["rust_commit"]).encode() in self.rustc_identity.splitlines(), "wrong compiler toolchain commit")
        self.identity_ready = True
        self.report["rustc_identity"] = self.rustc_identity.decode()
        self.report["fixture_environment"] = {name: self.env.get(name) for name in (*OVERLAYS, "RUSTUP_TOOLCHAIN", "VERUS_Z3_PATH", "SAIL_VERUS_BUILD", "SAIL_CARGO_VERUS")}
        self.save()

    def call(self, label, command, cwd=None, timeout=None, *, expected_exit=0):
        require(expected_exit == 0 or (expected_exit == 101
                and list(map(str, command)) == [str(self.args.sail), "verify", "--output", "json"]),
                "only success or a controlled public Cargo verification failure is expected")
        directory = self.out / "calls" / f"{len(self.calls):03d}-{label}"
        directory.mkdir(parents=True)
        row = {"label": label, "command": list(map(str, command)), "cwd": str(cwd or self.project),
               "timeout_seconds": timeout or self.args.timeout, "expected_exit": expected_exit,
               "force_requested": self.env.get("SAIL_MANAGED_FORCE") == "1", "complete": False}
        self.calls.append(row)
        self.save()
        begin = time.monotonic()
        process = subprocess.Popen(row["command"], cwd=cwd or self.project, env=self.env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        try:
            stdout, stderr = process.communicate(timeout=row["timeout_seconds"])
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate(timeout=10)
            row["timeout"] = True
        (directory / "stdout").write_bytes(stdout)
        (directory / "stderr").write_bytes(stderr)
        row.update(exit=process.returncode, seconds=time.monotonic() - begin,
                   stdout_sha256=digest(directory / "stdout"), stderr_sha256=digest(directory / "stderr"))
        self.save()
        require(not row.get("timeout"), f"{label}: command timeout")
        require(process.returncode == expected_exit, f"{label}: exit {process.returncode}, expected {expected_exit}; see raw streams")
        row["complete"] = True
        self.save()
        return stdout

    def results(self):
        return set(self.cache.glob("projects/*/*/runs/*/result.json"))

    def project_result(self, label, document, fresh, original=None):
        require(document.get("schema_version") == 2 and document.get("kind") == "sail-project-result", f"{label}: wrong project schema")
        require(document.get("currentness") == "current" and document.get("exit_code") == 0,
                f"{label}: result is not complete/current/successful")
        require(document.get("no_reuse") is None and document.get("error") is None, f"{label}: unmanaged or failed result")
        targets = [target for target in document["targets"] if target["requested"]]
        require(len(targets) == 1 and targets[0]["target"] == "origin_fixture" and targets[0]["complete"], f"{label}: exact whole target missing")
        target = targets[0]
        functions = [function for crate in target["report"]["crates"] for function in crate["functions"]]
        require(any(function["function"].split("::")[-1] == "origin_lemma" for function in functions),
                f"{label}: benign proof obligation was not reported")
        require(all(function["verdict"] == "verified" and function["establishment"] == "confirmed" for function in functions),
                f"{label}: benign fixture has a non-confirmed verdict")
        if fresh:
            require(target["establishing_run"] == document["run_id"], f"{label}: fresh report has wrong attempt identity")
        require(target["reused"] is (not fresh), f"{label}: wrong fresh/reused authority")
        count = document["execution"]["verifier_invocations"]
        require(count > 0 if fresh else count == 0, f"{label}: unexpected real compiler count {count}")
        if original is not None:
            require(target["establishing_run"] == original["establishing_run"] and target["report"] == original["report"],
                    f"{label}: warm result lost its original report/provenance")
        write(self.out / "projects" / (label + ".json"), document)
        self.cases.append({"label": label, "run_id": document["run_id"], "fresh": fresh,
                           "compiler_invocations": count, "establishing_run": target["establishing_run"],
                           "result_checks_passed": True, "passed": not fresh})
        self.save()
        return target

    def verify(self, label, json_output=True, fresh=True, original=None):
        before = self.results()
        arguments = [str(self.args.sail), "verify"] + (["--output", "json"] if json_output else [])
        raw = self.call(label, arguments)
        if json_output:
            document = json.loads(raw)
        else:
            added = self.results() - before
            require(len(added) == 1, f"{label}: shown-output call has no unique recorded result")
            document = json.loads(next(iter(added)).read_text())
        target = self.project_result(label, document, fresh, original)
        return document, target

    def run_directory(self, document):
        candidates = list(self.cache.glob(f"projects/*/{document['context_sha256']}/runs/{document['run_id']}"))
        require(len(candidates) == 1, "missing unique actual run directory")
        return candidates[0]

    def observe(self, label, document, target, name=None, phase=None, json_output=True):
        run = self.run_directory(document)
        calls = []
        for path in (run / "invocations").glob("*.json"):
            invocation = json.loads(path.read_text())
            if invocation["identity"]["manifest"] == str(self.project / "Cargo.toml"):
                calls.append((path, invocation))
        require(len(calls) == 1, f"{label}: missing unique real target invocation")
        path, invocation = calls[0]
        require(invocation["success"] and invocation["run_id"] == document["run_id"], f"{label}: target was not freshly established")
        sidecar = run / "sidecars" / path.stem
        reads = []
        for raw_path in sidecar.rglob("inputs.json"):
            value = json.loads(raw_path.read_text())
            if value.get("kind") == "sail-compiler-read-inputs":
                reads.append((raw_path, value))
        require(len(reads) == 1, f"{label}: no unique actual compiler observation")
        raw_path, raw = reads[0]
        require(raw.get("schema") == 2 and raw.get("complete") is True and raw.get("pid", 0) > 0,
                f"{label}: missing schema2 complete actual compiler document")
        expected_binding = hashlib.sha256((document["run_id"] + "\0" + path.stem).encode()).hexdigest()
        require(raw.get("binding") == expected_binding, f"{label}: actual compiler request/run identity mismatch")
        require(set(raw["phases"]) == {"verification", "compilation"}, f"{label}: incomplete compiler phases")
        require(set(raw["driver_environment"]) == {COMMON}, f"{label}: unclassified driver bookkeeping")
        union = {}
        for current in raw["phases"].values():
            require(current["sources"], f"{label}: phase has no source reads")
            for key, value in current["tracked_environment"].items():
                require(key not in union or union[key] == value, f"{label}: conflicting actual phase values")
                union[key] = value
        require(invocation["compiler_environment"] == union, f"{label}: invocation rewrote actual source values")
        origin = Path(json.loads((run.parent.parent / "runtime.json").read_text())["compiler_read_origins"])
        marker = origin / "pending" / (path.stem + ".json")
        if name is not None:
            require(name in raw["phases"]["verification"]["tracked_environment"], f"{label}: ghost/verification source read absent")
            require((name in raw["phases"]["compilation"]["tracked_environment"]) is (phase == "exec"), f"{label}: wrong source phase attribution")
            value = union[name]
            if name == COMMON:
                require(value == raw["driver_environment"][COMMON] and isinstance(value, str), f"{label}: raw COMMON was normalized")
            elif name == "SAIL_PROVENANCE_DIR":
                require(value == (str(sidecar / "provenance") if json_output else None), f"{label}: wrong effective provenance input")
            elif name == "SAIL_COMPILER_INPUTS":
                require(value == str(raw_path.parent), f"{label}: wrong actual compiler request endpoint")
            else:
                require(value == raw["binding"] and re.fullmatch(r"[0-9a-f]{64}", value), f"{label}: wrong request binding")
            require(marker.is_file(), f"{label}: source-read ineligibility was not persisted")
            pending = json.loads(marker.read_text())
            require(pending["version"] == 2 and pending["identity"] == invocation["identity"] and pending["package_id"] == target["package_id"], f"{label}: marker target mismatch")
            for directory in (run / "pending-receipts", run.parent.parent / "receipts"):
                for receipt_path in directory.glob("*.json"):
                    receipt = json.loads(receipt_path.read_text())
                    require(receipt["invocation"]["identity"]["manifest"] != str(self.project / "Cargo.toml"), f"{label}: ineligible target published a receipt")
        else:
            require(not any(key in union for key in OVERLAYS), f"{label}: baseline has a sensitive source read")
            require(not marker.exists(), f"{label}: qualified same-unit completion did not clear its marker")
        folder = self.out / "compiler" / label
        folder.mkdir(parents=True)
        if marker.is_file():
            (folder / "pending-marker.json").write_bytes(marker.read_bytes())
        write(folder / "marker-state.json", {"present": marker.is_file(), "path": str(marker),
              "sha256": digest(marker) if marker.is_file() else None})
        receipts = {}
        for directory in (run / "pending-receipts", run.parent.parent / "receipts"):
            for receipt_path in directory.glob("*.json"):
                receipt = json.loads(receipt_path.read_text())
                receipts[str(receipt_path)] = {"sha256": digest(receipt_path),
                    "selected_target": receipt["invocation"]["identity"]["manifest"] == str(self.project / "Cargo.toml")}
                if receipts[str(receipt_path)]["selected_target"]:
                    (folder / (directory.name + "-" + receipt_path.name)).write_bytes(receipt_path.read_bytes())
        write(folder / "receipt-inventory.json", receipts)
        for filename, data in (("invocation.json", path.read_bytes()), ("inputs.json", raw_path.read_bytes()),
                               ("source.rs", (self.project / "src/lib.rs").read_bytes())):
            (folder / filename).write_bytes(data)
        write(folder / "location.json", {"original_document": str(raw_path), "source_sha256": digest(self.project / "src/lib.rs"),
                                          "marker": str(marker), "driver_value": raw["driver_environment"][COMMON]})
        next(row for row in self.cases if row["label"] == label).update(passed=True, compiler_identity=path.stem,
                                                                        request_binding=expected_binding)
        self.save()
        return origin, union.get(name)

    def job(self, label, original):
        status = json.loads(self.call(label + "-submit", [str(self.args.sail), "verify", "--background", "--output", "json"]))
        require(status.get("id"), "job submission has no identity")
        deadline = time.monotonic() + self.args.timeout
        while True:
            response = json.loads(self.call(label + "-result", [str(self.args.sail), "jobs", "result", status["id"], "--output", "json"]))
            require(response.get("response") == "result" and response["value"]["job"]["id"] == status["id"], "job reply identity mismatch")
            if response["value"]["result"] is not None:
                require(response["value"]["job"]["state"] == "completed", "job did not complete normally")
                return self.project_result(label, response["value"]["result"], False, original)
            require(response["value"]["job"]["state"] in ("queued", "running") and time.monotonic() < deadline, "job failed/timed out")
            time.sleep(0.1)

    def mcp(self, original):
        folder = self.out / "mcp"
        folder.mkdir()
        with (folder / "stderr").open("wb") as stderr, (folder / "stdout").open("wb") as raw_stdout, (folder / "frames.jsonl").open("w") as transcript:
            child = subprocess.Popen([str(self.args.sail), "mcp"], cwd=self.project, env=self.env,
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr, start_new_session=True)
            buffered = bytearray()
            def exchange(message):
                transcript.write(json.dumps({"sent": message}) + "\n"); transcript.flush()
                child.stdin.write((json.dumps(message) + "\n").encode()); child.stdin.flush()
                deadline = time.monotonic() + self.args.timeout
                while time.monotonic() < deadline:
                    while b"\n" not in buffered:
                        require(select.select([child.stdout], [], [], max(0, deadline - time.monotonic()))[0], "MCP response timeout")
                        chunk = os.read(child.stdout.fileno(), 65536)
                        require(chunk, "MCP closed before response")
                        raw_stdout.write(chunk); raw_stdout.flush(); buffered.extend(chunk)
                    line, _, remaining = buffered.partition(b"\n")
                    buffered[:] = remaining
                    value = json.loads(line)
                    transcript.write(json.dumps({"received": value}) + "\n"); transcript.flush()
                    if value.get("id") == message["id"]:
                        require("error" not in value, "MCP RPC error")
                        return value["result"]
                raise RuntimeError("MCP response timeout")
            try:
                exchange({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                    "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "sail-origin-qualification", "version": "1"}}})
                child.stdin.write((json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n").encode()); child.stdin.flush()
                reply = exchange({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
                    "name": "verify", "arguments": {"project_dir": str(self.project), "detail": "targets"}}})
                require(not reply.get("isError") and "project_result" in reply["structuredContent"], "MCP omitted actual project result")
                self.project_result("warm-mcp", reply["structuredContent"]["project_result"], False, original)
            finally:
                child.stdin.close()
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL); child.wait(timeout=10)
                self.report["mcp"] = {"command": [str(self.args.sail), "mcp"], "exit": child.returncode}
            require(child.returncode == 0, "MCP process failed")
        self.report["mcp"].update(stdout_sha256=digest(folder / "stdout"), stderr_sha256=digest(folder / "stderr"),
                                  transcript_sha256=digest(folder / "frames.jsonl"))
        self.save()

    def setup(self):
        self.project.parent.mkdir(parents=True)
        self.call("new-project", [str(self.args.cargo_verus), "verus", "new", "--lib", "origin_fixture"], self.project.parent)
        (self.project / "src/lib.rs").write_text(BASE)
        self.daemon_log = (self.out / "daemon.log").open("wb")
        self.daemon = subprocess.Popen([str(self.args.sail), "serve", "--cache", str(self.cache), "--socket", str(self.socket), "--idle-timeout", "0"],
                                       env=self.env, stdout=self.daemon_log, stderr=subprocess.STDOUT, start_new_session=True)
        deadline = time.monotonic() + 10
        while not self.socket.exists() and self.daemon.poll() is None and time.monotonic() < deadline:
            time.sleep(0.02)
        require(self.socket.exists() and self.daemon.poll() is None, "owned daemon failed to start")

    def execute(self):
        self.capture_identity()
        self.setup()
        if self.args.scope == "lifecycle":
            self.process_interruption_control()
            self.retained_control()
            require(self.cases and all(row["passed"] for row in self.cases), "lifecycle evidence incomplete")
            self.report["implemented_scope_complete"] = True
            self.report["unrun_required_overall"] = [{"case": "basic CLI/job/MCP/source-read controls",
                "reason": "not executed by this lifecycle-only invocation; requires its separate qualified packet"},
                {"case": "whole native/conformance/both-platform runtime qualification",
                 "reason": "separate owning acceptance gates; not implemented by this bounded runner"}]
            return
        # A new Cargo project can first discover script/declared inputs. Keep
        # such preparation visible and uncredited; a fresh Current run is still
        # mandatory before any warm-reuse assertions.
        self.report["bootstrap_attempts"] = []
        for attempt in range(3):
            cold = json.loads(self.call(f"baseline-establish-{attempt}", [str(self.args.sail), "verify", "--output", "json"]))
            if cold.get("currentness") == "current":
                original = self.project_result("baseline-cold", cold, True)
                break
            require(cold.get("exit_code") == 0 and cold.get("error") is None and cold.get("no_reuse") is None
                    and cold.get("currentness") in ("historical", "unknown"), "baseline preparation failed")
            self.report["bootstrap_attempts"].append({"run_id": cold["run_id"], "currentness": cold["currentness"],
                                                       "qualification_credit": False})
            self.save()
        else:
            raise RuntimeError("baseline never established a fresh Current result")
        origin, _ = self.observe("baseline-cold", cold, original)
        self.verify("baseline-warm", fresh=False, original=original)
        self.job("warm-job", original)
        self.mcp(original)
        for filename in ("Cargo.toml", "Cargo.lock"):
            path = self.project / filename
            require(path.is_file(), "fixture setup omitted " + filename)
            destination = self.out / "fixture" / filename
            destination.parent.mkdir(exist_ok=True)
            destination.write_bytes(path.read_bytes())
        source = self.project / "src/lib.rs"
        for index, name in enumerate(READS):
            for phase in ("ghost", "exec"):
                prefix = f"read-{index}-{phase}"
                extra = (f'\nverus! {{ proof fn observe_setting() {{ assert(match option_env!({json.dumps(name)}) {{ Some(_) => true, None => true }}); }} }}\n'
                         if phase == "ghost" else f'\npub const ORIGIN_VALUE: Option<&str> = option_env!({json.dumps(name)});\n')
                source.write_text(BASE + extra)
                values = []
                for mode, as_json in (("human-first", False), ("json", True), ("json-repeat", True), ("human-again", False)):
                    label = prefix + "-" + mode
                    document, target = self.verify(label, as_json)
                    _, value = self.observe(label, document, target, name, phase, as_json)
                    values.append(value)
                require(values[1] != values[2], f"{prefix}: two actual JSON compiler requests did not expose distinct raw run values")
                source.write_text(BASE)
                document, recovered = self.verify(prefix + "-removed")
                self.observe(prefix + "-removed", document, recovered)
                identities = {row["compiler_identity"] for row in self.cases
                              if row["label"].startswith(prefix + "-") and row["fresh"]}
                require(len(identities) == 1, f"{prefix}: removal fixture changed compiler-unit identity")
                self.verify(prefix + "-recovered-warm", fresh=False, original=recovered)
        self.semantic_provenance_control()
        self.semantic_shadow_control()
        # Corrupt only non-authorizing fixture metadata. Each case must discard
        # old artifacts and genuinely verify before reinitializing its state.
        for label in ("missing-origin", "legacy-origin", "malformed-origin"):
            mutation = self.out / "origin-mutations" / label
            self.capture_origin_state(origin, mutation / "before")
            if label == "missing-origin":
                (origin / "schema").unlink()
            elif label == "legacy-origin":
                (origin / "schema").write_text("1")
            else:
                (origin / "pending/unknown.json").write_text("{")
            self.capture_origin_state(origin, mutation / "mutated")
            document, target = self.verify(label)
            self.observe(label, document, target)
            self.capture_origin_state(origin, mutation / "after")
            runtime = json.loads((self.run_directory(document).parent.parent / "runtime.json").read_text())
            family_target = Path(runtime["compiler_read_origins"]).parent / "target"
            tag = family_target / "CACHEDIR.TAG"
            require(tag.is_file() and tag.read_bytes().startswith(b"Signature: 8a477f597d28d172789f06886806bc55\n"),
                    f"{label}: migration lost source-cache exclusion")
            self.verify(label + "-warm", fresh=False, original=target)
        require(self.cases and all(row["passed"] for row in self.cases), "a required result/raw-observation check is incomplete")
        self.report["implemented_scope_complete"] = True

    def semantic_provenance_control(self):
        """Same source: a real human proof must become a real JSON failure."""
        previous = self.project
        parent = self.out / "work/semantic"
        parent.mkdir()
        self.call("semantic-new-project", [str(self.args.cargo_verus), "verus", "new", "--lib", "origin_fixture"], parent)
        self.project = parent / "origin_fixture"
        source = self.project / "src/lib.rs"
        source.write_text(BASE + '\nverus! { proof fn provenance_is_absent() {\n'
                          '    assert(match option_env!("SAIL_PROVENANCE_DIR") { None => true, Some(_) => false }) by(bit_vector);\n'
                          '} }\n')
        source_sha256 = digest(source)
        preparation = []
        self.report["semantic_preparation"] = preparation
        try:
            # Discovering inputs does not establish this precondition. If needed,
            # use the supported force flag to obtain a genuinely fresh human proof.
            for attempt in range(3):
                if attempt:
                    self.env["SAIL_MANAGED_FORCE"] = "1"
                before = self.results()
                self.call(f"semantic-human-attempt-{attempt}", [str(self.args.sail), "verify"])
                added = self.results() - before
                require(len(added) == 1, "semantic human call has no unique actual result")
                actual = next(iter(added))
                baseline = json.loads(actual.read_text())
                (self.out / "projects" / f"semantic-human-attempt-{attempt}.stored.json").write_bytes(actual.read_bytes())
                require(digest(source) == source_sha256, "semantic source changed during human preparation")
                if baseline.get("currentness") == "current":
                    target = self.project_result("semantic-human-proved", baseline, True)
                    functions = [function for crate in target["report"]["crates"] for function in crate["functions"]]
                    require(sum(function["function"].split("::")[-1] == "provenance_is_absent" for function in functions) == 1,
                            "human baseline did not actually report the semantic provenance proof")
                    self.observe("semantic-human-proved", baseline, target,
                                 "SAIL_PROVENANCE_DIR", "ghost", False)
                    next(row for row in self.cases if row["label"] == "semantic-human-proved").update(
                        control="semantic-mode-flip", expected_verdict="verified", source_sha256=source_sha256)
                    break
                require(baseline.get("exit_code") == 0 and baseline.get("error") is None
                        and baseline.get("no_reuse") is None and baseline.get("currentness") in ("historical", "unknown"),
                        "semantic human preparation did not successfully verify")
                preparation.append({"run_id": baseline["run_id"], "currentness": baseline["currentness"],
                                    "forced": bool(attempt), "qualification_credit": False})
                self.save()
            else:
                raise RuntimeError("semantic control has no complete fresh Current human proof")
            self.env.pop("SAIL_MANAGED_FORCE", None)
            require(digest(source) == source_sha256 and "SAIL_PROVENANCE_DIR" not in self.env,
                    "semantic mode flip changed its source or ambient provenance")
            self.semantic_failure(baseline, source_sha256)
        finally:
            self.env.pop("SAIL_MANAGED_FORCE", None)
            self.project = previous

    def semantic_failure(self, baseline, source_sha256, label="semantic-json-failed",
                         function_name="provenance_is_absent", prior_label="semantic-human-proved",
                         control="semantic-mode-flip"):
        case = {"label": label, "fresh": True, "expected_exit": 101,
                "control": control, "baseline_run_id": baseline["run_id"],
                "expected_function": function_name, "expected_verdict": "failed", "passed": False}
        self.cases.append(case)
        self.save()
        document = json.loads(self.call(label, [str(self.args.sail), "verify", "--output", "json"], expected_exit=101))
        write(self.out / "projects" / (label + ".json"), document)
        require(document.get("kind") == "sail-project-result" and document.get("schema_version") == 2
                and document.get("currentness") == "current" and document.get("exit_code") == 101,
                "semantic JSON result is not a Current ordinary verification failure")
        require(document.get("error") is None and document.get("no_reuse") is None,
                "semantic control failed setup instead of its proof")
        require(document["run_id"] != baseline["run_id"] and document["context_sha256"] == baseline["context_sha256"],
                "semantic mode flip lost its original context or new attempt identity")
        require(document["execution"]["verifier_invocations"] > 0, "semantic JSON call skipped the actual compiler")
        targets = [target for target in document["targets"] if target["requested"]]
        require(len(targets) == 1 and targets[0]["target"] == "origin_fixture", "semantic failed target missing")
        target = targets[0]
        require(target["reused"] is False and target["complete"] is False
                and target["establishing_run"] == document["run_id"], "semantic JSON call reused prior verified authority")
        require(target["report"].get("error") is None and target["report"].get("exit_code") == 1
                and target["report"]["summary"]["failed"] == 1
                and target["report"]["summary"]["unknown"] == 0 and target["report"]["summary"]["provisional"] == 0,
                "semantic target is not the single expected settled verification failure")
        functions = [function for crate in target["report"]["crates"] for function in crate["functions"]]
        failed = [function for function in functions if function["function"].split("::")[-1] == function_name]
        require(len(failed) == 1 and failed[0]["verdict"] == "failed" and failed[0]["establishment"] == "confirmed",
                "semantic obligation was not confirmed failed")
        obligations = [obligation for obligation in failed[0]["obligations"] or []
                       if not obligation["advisory"] and obligation["outcome"] == "failed"]
        require(any(check["answer"] == "sat" for obligation in obligations for check in obligation["checks"]),
                "semantic failure has no settled failing SMT obligation")
        run = self.run_directory(document)
        calls = [(path, json.loads(path.read_text())) for path in (run / "invocations").glob("*.json")]
        calls = [(path, invocation) for path, invocation in calls
                 if invocation["identity"]["manifest"] == str(self.project / "Cargo.toml")]
        require(len(calls) == 1, "semantic failure has no unique actual target invocation")
        path, invocation = calls[0]
        prior = next(row for row in self.cases if row["label"] == prior_label)
        require(path.stem == prior["compiler_identity"] and invocation["run_id"] == document["run_id"]
                and invocation["success"] is False and invocation["report"] == target["report"],
                "semantic failure is not the new same-unit failed compiler attempt")
        started = [(entry, json.loads(entry.read_text())) for entry in (run / "invocations").glob("*.started")]
        require(len(started) == document["execution"]["verifier_invocations"], "semantic compiler counter lacks actual start records")
        selected = [(entry, record) for entry, record in started
                    if record["run_id"] == document["run_id"] and record["target"] == "origin_fixture"]
        require(len(selected) == 1 and selected[0][0].stem.isdigit() and int(selected[0][0].stem) > 0,
                "semantic failed compiler start missing")
        runtime = json.loads((run.parent.parent / "runtime.json").read_text())
        require(runtime["run_id"] == document["run_id"] and runtime["attribution_logs"] is True,
                "semantic JSON request did not enable its actual attribution overlay")
        marker = Path(runtime["compiler_read_origins"]) / "pending" / (path.stem + ".json")
        require(marker.is_file(), "failed compiler erased its non-authorizing origin marker")
        pending = json.loads(marker.read_text())
        require(pending["version"] == 2 and pending["identity"] == invocation["identity"]
                and pending["package_id"] == target["package_id"], "semantic failed marker has wrong identity")
        for directory in (run / "pending-receipts", run.parent.parent / "receipts"):
            for receipt_path in directory.glob("*.json"):
                receipt = json.loads(receipt_path.read_text())
                require(receipt["invocation"]["identity"]["manifest"] != str(self.project / "Cargo.toml"),
                        "semantic failed target retained a prior verified receipt")
        sidecar = run / "sidecars" / path.stem
        raw = json.loads((sidecar / "verus.stdout").read_bytes())
        result = raw["verification-results"]
        require(result["success"] is False and result["errors"] > 0 and result["is-verifying-entire-crate"] is True
                and raw["verus"]["commit"] == self.report["verus_source"], "actual Verus did not report the expected complete-scope failure")
        require(digest(self.project / "src/lib.rs") == source_sha256, "semantic source changed before the failed report")
        folder = self.out / "compiler" / label
        folder.mkdir(parents=True)
        for filename, actual in (("invocation.json", path), ("pending-marker.json", marker),
                                 ("source.rs", self.project / "src/lib.rs"), ("verus.stdout", sidecar / "verus.stdout"),
                                 ("verus.stderr", sidecar / "verus.stderr"), ("runtime.json", run.parent.parent / "runtime.json")):
            (folder / filename).write_bytes(actual.read_bytes())
        for entry, _ in selected:
            (folder / entry.name).write_bytes(entry.read_bytes())
        # Failed verification need not finish the two successful compiler phases.
        # Preserve any documents, but never demand or manufacture inputs.json.
        observed = []
        for index, inputs in enumerate(sorted(sidecar.rglob("inputs.json"))):
            (folder / f"optional-inputs-{index}.json").write_bytes(inputs.read_bytes())
            observed.append({"path": str(inputs), "sha256": digest(inputs)})
        write(folder / "observation-status.json", {"required_for_failure": False, "documents": observed})
        case.update(run_id=document["run_id"], compiler_invocations=document["execution"]["verifier_invocations"],
                    establishing_run=target["establishing_run"], compiler_identity=path.stem,
                    source_sha256=source_sha256, compiler_pids=[int(entry.stem) for entry, _ in selected], failed_obligations=[obligation["id"] for obligation in obligations], passed=True)
        self.save()

    def observe_shadow(self, document, target):
        run = self.run_directory(document)
        calls = [(path, json.loads(path.read_bytes())) for path in (run / "invocations").glob("*.json")]
        calls = [(path, call) for path, call in calls
                 if call["identity"]["manifest"] == str(self.project / "Cargo.toml")]
        require(len(calls) == 1, "shadow baseline has no unique actual target invocation")
        path, call = calls[0]
        require(call["success"] is True and call["run_id"] == document["run_id"]
                and call["report"] == target["report"], "shadow baseline is not its actual fresh proof")
        functions = [function for crate in target["report"]["crates"] for function in crate["functions"]]
        require(sum(function["function"].split("::")[-1] == "shadow_is_absent" for function in functions) == 1,
                "shadow baseline did not report its semantic proof")
        sidecar = run / "sidecars" / path.stem
        observations = [(entry, json.loads(entry.read_bytes())) for entry in sidecar.rglob("inputs.json")]
        observations = [(entry, raw) for entry, raw in observations if raw.get("kind") == "sail-compiler-read-inputs"]
        require(len(observations) == 1, "shadow baseline has no unique raw compiler observation")
        entry, raw = observations[0]
        binding = hashlib.sha256((document["run_id"] + "\0" + path.stem).encode()).hexdigest()
        require(raw.get("schema") == 2 and raw.get("complete") is True and raw.get("pid", 0) > 0
                and raw.get("binding") == binding and set(raw["phases"]) == {"verification", "compilation"},
                "shadow baseline has no complete schema2 two-phase observation")
        require(set(raw["driver_environment"]) == {COMMON}, "unexpected driver bookkeeping")
        require(SHADOW in raw["phases"]["verification"]["tracked_environment"]
                and raw["phases"]["verification"]["tracked_environment"][SHADOW] is None
                and SHADOW not in raw["phases"]["compilation"]["tracked_environment"],
                "shadow read is not the expected ghost-only absent source input")
        union = {}
        for phase in raw["phases"].values():
            require(phase["sources"], "compiler observation has no source inputs")
            for name, value in phase["tracked_environment"].items():
                require(name not in union or union[name] == value, "conflicting source-read phases")
                union[name] = value
        require(call["compiler_environment"] == union, "invocation changed raw shadow source values")
        runtime = json.loads((run.parent.parent / "runtime.json").read_bytes())
        marker = Path(runtime["compiler_read_origins"]) / "pending" / (path.stem + ".json")
        require(not marker.exists(), "eligible inherited shadow read did not clear its marker")
        folder = self.out / "compiler/shadow-cold"
        folder.mkdir(parents=True)
        for filename, actual in (("invocation.json", path), ("inputs.json", entry),
                                 ("source.rs", self.project / "src/lib.rs")):
            (folder / filename).write_bytes(actual.read_bytes())
        next(case for case in self.cases if case["label"] == "shadow-cold").update(
            passed=True, compiler_identity=path.stem, request_binding=binding)

    def capture_shadow_noop_record(self, baseline):
        records = [(path, json.loads(path.read_bytes())) for path in (self.cache / "noop").glob("*.json")]
        records = [(path, raw) for path, raw in records
                   if raw["result"]["context_sha256"] == baseline["context_sha256"]
                   and raw["result"]["workspace"] == str(self.project)]
        require(len(records) == 1, "actual noop hit has no unique matching record")
        path, raw = records[0]
        require(path.stem == raw["key"] and raw["version"] == 4
                and SHADOW in raw["environment"] and raw["environment"][SHADOW] is None,
                "actual replay record omitted its ghost-only receipt requirement")
        target = Path(json.loads((self.run_directory(baseline).parent.parent / "runtime.json").read_bytes())
                      ["compiler_read_origins"]).parent / "target"
        folder = self.out / "compiler/shadow-cold"
        info = []
        for index, dep in enumerate(sorted(target.rglob("*.d"))):
            contents = dep.read_bytes()
            require(SHADOW.encode() not in contents, "shadow fixture unexpectedly reached dep-info")
            (folder / f"dep-info-{index}.d").write_bytes(contents)
            info.append({"path": str(dep), "sha256": digest(dep)})
        require(info, "no actual dep-info was inspected")
        write(folder / "dep-info-inventory.json", info)
        (folder / "noop-record.json").write_bytes(path.read_bytes())
        return raw["key"]

    def semantic_shadow_control(self):
        """A genuine noop hit must miss after its ghost receipt input changes."""
        previous = self.project
        parent = self.out / "work/shadow"
        parent.mkdir()
        self.call("shadow-new-project", [str(self.args.cargo_verus), "verus", "new", "--lib", "origin_fixture"], parent)
        self.project = parent / "origin_fixture"
        source = self.project / "src/lib.rs"
        source.write_text(BASE + '\nverus! { proof fn shadow_is_absent() {\n'
                          '    assert(match option_env!(" __VERUS_DRIVER_SHADOW__") { None => true, Some(_) => false }) by(bit_vector);\n'
                          '} }\n')
        source_sha256 = digest(source)
        trace = self.out / "shadow-noop.trace"
        previous_trace = self.env.get("SAIL_MANAGED_NOOP_TRACE")
        self.env["SAIL_MANAGED_NOOP_TRACE"] = str(trace)
        self.env.pop(SHADOW, None)
        self.report["shadow_preparation"] = []
        try:
            for attempt in range(3):
                if attempt:
                    self.env["SAIL_MANAGED_FORCE"] = "1"
                baseline = json.loads(self.call(f"shadow-cold-{attempt}", [str(self.args.sail), "verify", "--output", "json"]))
                write(self.out / f"projects/shadow-preparation-{attempt}.json", baseline)
                if baseline.get("currentness") == "current":
                    original = self.project_result("shadow-cold", baseline, True)
                    self.observe_shadow(baseline, original)
                    break
                require(baseline.get("exit_code") == 0 and baseline.get("error") is None
                        and baseline.get("no_reuse") is None and baseline.get("currentness") in ("historical", "unknown"),
                        "shadow preparation did not successfully verify")
                self.report["shadow_preparation"].append({"run_id": baseline["run_id"], "qualification_credit": False})
                self.save()
            else:
                raise RuntimeError("no complete fresh Current shadow baseline")
            self.env.pop("SAIL_MANAGED_FORCE", None)
            for attempt in range(3):
                time.sleep(2.1)  # Actual ADR 0082 settling; no fabricated old stamp.
                before = trace.read_bytes() if trace.exists() else b""
                self.verify(f"shadow-warm-{attempt}", fresh=False, original=original)
                after = trace.read_bytes()
                require(after.startswith(before), "noop trace was replaced during warm call")
                delta = after[len(before):]
                (self.out / f"shadow-warm-{attempt}.trace").write_bytes(delta)
                if b"hit" in delta.splitlines():
                    record_key = self.capture_shadow_noop_record(baseline)
                    self.report["shadow_noop_hit"] = {"attempt": attempt, "record_key": record_key}
                    break
            else:
                raise RuntimeError("baseline never reached an actual unchanged-call replay hit")
            require(digest(source) == source_sha256, "shadow baseline source changed")
            before = trace.read_bytes()
            before_environment = dict(self.env)
            self.env[SHADOW] = "source-visible-change"
            self.semantic_failure(baseline, source_sha256, "shadow-changed-failed", "shadow_is_absent",
                                  "shadow-cold", "semantic-receipt-input")
            require(self.env == dict(before_environment, **{SHADOW: "source-visible-change"}),
                    "shadow witness changed another caller environment input")
            after = trace.read_bytes()
            require(after.startswith(before), "noop trace was replaced during changed call")
            delta = after[len(before):]
            (self.out / "shadow-changed.trace").write_bytes(delta)
            require(b"hit" not in delta.splitlines() and b"miss: a declared variable changed" in delta.splitlines(),
                    "changed caller input did not miss the actual recorded-key lookup")
            write(self.out / "compiler/shadow-changed-failed/noop-lookup.json",
                  {"recorded_key": record_key, "source_sha256": source_sha256,
                   "caller_input": {"name": SHADOW, "before": None, "after": self.env[SHADOW]},
                   "after_lookup": "same workspace/context reached recorded environment mismatch"})
        finally:
            self.env.pop(SHADOW, None)
            if previous_trace is None:
                self.env.pop("SAIL_MANAGED_NOOP_TRACE", None)
            else:
                self.env["SAIL_MANAGED_NOOP_TRACE"] = previous_trace
            self.env.pop("SAIL_MANAGED_FORCE", None)
            self.project = previous

    @staticmethod
    def process_identity(pid):
        root = Path("/proc") / str(pid)
        tail = (root / "stat").read_text().rsplit(")", 1)[1].split()
        return {"pid": pid, "state": tail[0], "group": int(tail[2]), "start_ticks": int(tail[19]),
                "executable": str((root / "exe").resolve(strict=True)),
                "cwd": str((root / "cwd").resolve(strict=True)),
                "command": (root / "cmdline").read_bytes().split(b"\0")[:-1]}

    @staticmethod
    def group_processes(group):
        result = []
        for entry in Path("/proc").iterdir():
            if not entry.name.isdigit():
                continue
            try:
                tail = (entry / "stat").read_text().rsplit(")", 1)[1].split()
                if int(tail[2]) == group and tail[0] != "Z":
                    result.append(int(entry.name))
            except (FileNotFoundError, ProcessLookupError):
                pass
        return sorted(result)

    @staticmethod
    def pidfd_live(descriptor):
        return not select.select([descriptor], [], [], 0)[0]

    def compiler_identity_predicate(self, identity):
        return {"cwd_matches": identity["cwd"] == str(self.project),
            "installed_executable_matches": identity["executable"] in
                {str((self.args.runtime_root / name).resolve()) for name in ("verus", "rust_verify")},
            "crate_name_option_present": b"--crate-name" in identity["command"],
            "crate_name_value_present": b"origin_fixture" in identity["command"],
            "source_argument_present": any(argument.endswith(b"src/lib.rs") for argument in identity["command"])}

    def wait_for_compiler_exec(self, descriptor, initial, binding, folder):
        """Wait across the observed rustup exec; never authorize that launcher."""
        rustup = shutil.which("rustup", path=self.env.get("PATH"))
        require(rustup is not None, "selected rustup is unavailable")
        rustup = str(Path(rustup).resolve())
        compiler = str((self.args.runtime_root / "rust_verify").resolve())
        host = next(feature.removeprefix("host:") for feature in self.manifest["features"] if feature.startswith("host:"))
        toolchain = self.pins["verus"]["rust_toolchain"] + "-" + host
        expected_prefix = [b"rustup", b"run", toolchain.encode(), b"--", compiler.encode()]
        record = {"authorized_for_signal": False, "completed_wait": False,
                  "pid": initial["pid"], "start_ticks": initial["start_ticks"], "group": initial["group"],
                  "expected_binding": binding.decode(), "observations": []}
        deadline = time.monotonic() + 10
        identity = initial
        try:
            while True:
                require(self.pidfd_live(descriptor), "compiler exited during launcher exec wait")
                predicate = self.compiler_identity_predicate(identity)
                require(all(value for name, value in predicate.items() if name != "installed_executable_matches"),
                        "compiler command/source/cwd changed during exec wait")
                require((identity["pid"], identity["start_ticks"], identity["group"])
                        == (initial["pid"], initial["start_ticks"], initial["group"]),
                        "exec wait observed a different process identity")
                environment = dict(entry.split(b"=", 1) for entry in
                    (Path("/proc") / str(initial["pid"]) / "environ").read_bytes().split(b"\0") if b"=" in entry)
                require(environment.get(b"SAIL_COMPILER_INPUTS_BINDING") == binding and self.pidfd_live(descriptor),
                        "compiler request binding or liveness changed during exec wait")
                observed = dict(identity, command=[argument.decode(errors="backslashreplace") for argument in identity["command"]])
                if not record["observations"] or any(observed[name] != record["observations"][-1][name]
                                                     for name in ("executable", "command")):
                    record["observations"].append(observed)
                if identity["executable"] == compiler:
                    require(all(predicate.values()), "selected compiler failed its original identity predicate")
                    record["completed_wait"] = True
                    return identity
                require(identity["executable"] == rustup and identity["command"][:5] == expected_prefix,
                        "unexpected executable or command during compiler exec wait")
                require(time.monotonic() < deadline, "selected compiler did not finish the observed rustup exec")
                time.sleep(0.01)
                identity = self.process_identity(initial["pid"])
                require(self.pidfd_live(descriptor), "held compiler exited while rereading exec identity")
        finally:
            write(folder / "compiler-exec-wait.json", record)

    def open_live_process(self, pid, group=None):
        """Return a held live identity, or omit only an actually exited PID."""
        descriptor = None
        try:
            descriptor = os.pidfd_open(pid)
            identity = self.process_identity(pid)
            if not self.pidfd_live(descriptor):
                return None
            require(group is None or identity["group"] == group,
                    "enumerated live process no longer belongs to the owned group")
            require(identity["state"] != "Z", "enumerated process is not live")
            # Close the numeric-PID/identity-read interval while the descriptor
            # still refers to a living process. A reused numeric PID can never
            # contribute a descriptor to the signaling set.
            if not self.pidfd_live(descriptor):
                return None
            held = descriptor
            descriptor = None
            return held, identity
        except (FileNotFoundError, ProcessLookupError):
            if descriptor is None or not self.pidfd_live(descriptor):
                return None
            raise
        finally:
            if descriptor is not None:
                os.close(descriptor)

    def process_interruption_control(self):
        """Public job cancellation of an identity-checked live compiler group."""
        require(platform.system() == "Linux" and hasattr(os, "pidfd_open")
                and hasattr(signal, "pidfd_send_signal"), "lifecycle cancellation requires native Linux pidfds")
        source = self.project / "src/lib.rs"
        source.write_text(BASE)
        self.report["scope"] = "Public managed-job compiler cancellation/restart and eligible retained Vest A-to-B-to-A; separate from basic controls."
        self.report["lifecycle_preparation"] = []
        for attempt in range(3):
            if attempt:
                self.env["SAIL_MANAGED_FORCE"] = "1"
            baseline = json.loads(self.call(f"interrupt-establish-{attempt}", [str(self.args.sail), "verify", "--output", "json"]))
            write(self.out / f"projects/interrupt-preparation-{attempt}.json", baseline)
            if baseline.get("currentness") == "current":
                target = self.project_result("interrupt-established", baseline, True)
                self.observe("interrupt-established", baseline, target)
                break
            require(baseline.get("exit_code") == 0 and baseline.get("error") is None
                    and baseline.get("no_reuse") is None and baseline.get("currentness") in ("historical", "unknown"),
                    "interruption preparation failed")
            self.report["lifecycle_preparation"].append({"run_id": baseline["run_id"], "qualification_credit": False})
        else:
            raise RuntimeError("interruption fixture has no fresh Current baseline")
        self.env.pop("SAIL_MANAGED_FORCE", None)
        source.write_text(BASE + '\nverus! { proof fn interrupted_read() { assert(match option_env!("SAIL_PROVENANCE_DIR") { None => true, Some(_) => true }); } }\n')
        expected_source = digest(source)
        before = set(self.cache.glob("projects/*/*/runs/*/invocations/*.started"))
        job = json.loads(self.call("interrupt-submit", [str(self.args.sail), "verify", "--background", "--output", "json"]))
        require(job.get("id"), "interruption job has no actual identity")
        folder = self.out / "compiler/interruption"
        folder.mkdir(parents=True)
        descriptors = {}
        unverified = set()
        compiler_fd = None
        owned = False
        try:
            deadline = time.monotonic() + self.args.timeout
            while time.monotonic() < deadline:
                started = [(path, json.loads(path.read_bytes())) for path in
                           set(self.cache.glob("projects/*/*/runs/*/invocations/*.started")) - before]
                selected = [(path, raw) for path, raw in started if raw["target"] == "origin_fixture"]
                if selected:
                    require(len(selected) == 1, "ambiguous live target compiler")
                    started_path, start = selected[0]
                    pid = int(started_path.stem)
                    live_compiler = self.open_live_process(pid)
                    require(live_compiler is not None, "recorded compiler exited before ownership validation")
                    candidate_fd, identity = live_compiler
                    unverified.add(candidate_fd)
                    run = started_path.parent.parent
                    runtime_path = run.parent.parent / "runtime.json"
                    runtime = json.loads(runtime_path.read_bytes())
                    require(runtime["run_id"] == start["run_id"] == run.name, "live compiler run mismatch")
                    markers = [(path, json.loads(path.read_bytes())) for path in
                               (Path(runtime["compiler_read_origins"]) / "pending").glob("*.json")]
                    markers = [(path, raw) for path, raw in markers
                               if raw["identity"]["manifest"] == str(self.project / "Cargo.toml")]
                    require(len(markers) == 1 and markers[0][1]["version"] == 2, "live compiler has no own write-ahead marker")
                    marker, pending = markers[0]
                    unit = marker.stem
                    binding = hashlib.sha256((run.name + "\0" + unit).encode()).hexdigest().encode()
                    environment = dict(entry.split(b"=", 1) for entry in (Path("/proc") / str(pid) / "environ").read_bytes().split(b"\0") if b"=" in entry)
                    require(environment.get(b"SAIL_COMPILER_INPUTS_BINDING") == binding,
                            "live compiler has another request binding")
                    expected_executables = sorted(str((self.args.runtime_root / name).resolve())
                                                  for name in ("verus", "rust_verify"))
                    compiler_predicate = self.compiler_identity_predicate(identity)
                    # This is refusal diagnosis only. Capturing a candidate PID
                    # authorizes no signal; all command/ownership and held-fd
                    # liveness guards below still have to pass independently.
                    write(folder / "compiler-candidate-unverified.json", {"authorized_for_signal": False,
                        "identity": dict(identity, command=[argument.decode(errors="backslashreplace") for argument in identity["command"]]),
                        "expected_cwd": str(self.project), "expected_executables": expected_executables,
                        "recorded_original_wrapper": runtime.get("original_wrapper"),
                        "observed_executable_sha256": digest(Path(identity["executable"])),
                        "actual_binding": environment[b"SAIL_COMPILER_INPUTS_BINDING"].decode(),
                        "expected_binding": binding.decode(), "predicate": compiler_predicate,
                        "held_pidfd_live_after_observation": self.pidfd_live(candidate_fd)})
                    (folder / "candidate.started").write_bytes(started_path.read_bytes())
                    (folder / "candidate-marker.json").write_bytes(marker.read_bytes())
                    if not compiler_predicate["installed_executable_matches"]:
                        identity = self.wait_for_compiler_exec(candidate_fd, identity, binding, folder)
                        compiler_predicate = self.compiler_identity_predicate(identity)
                    require(all(compiler_predicate.values()),
                            "live PID is not the selected installed target compiler")
                    live_leader = self.open_live_process(identity["group"], identity["group"])
                    require(live_leader is not None, "public-job group leader exited before ownership validation")
                    leader_fd, leader = live_leader
                    unverified.add(leader_fd)
                    leader_environment = dict(entry.split(b"=", 1) for entry in
                        (Path("/proc") / str(leader["pid"]) / "environ").read_bytes().split(b"\0") if b"=" in entry)
                    require(digest(Path(leader["executable"])) == digest(self.args.sail)
                            and leader["group"] == leader["pid"] == identity["group"]
                            and leader["cwd"] == str(self.project)
                            and b"verify" in leader["command"] and leader_environment.get(b"SAIL_JOB_GUARD_PARENT") == b"1"
                            and leader_environment.get(b"SAIL_JOB_EVENTS", b"").endswith(("/" + job["id"] + "/events.jsonl").encode()),
                            "compiler group has no owned public-job worker")
                    require(self.pidfd_live(candidate_fd) and self.pidfd_live(leader_fd),
                            "compiler or job leader exited during binding/command validation")
                    compiler_fd = candidate_fd
                    descriptors[pid] = candidate_fd
                    descriptors[leader["pid"]] = leader_fd
                    unverified.remove(candidate_fd)
                    unverified.remove(leader_fd)
                    owned = True
                    member_identities = []
                    for member in self.group_processes(identity["group"]):
                        if member not in descriptors:
                            live_member = self.open_live_process(member, identity["group"])
                            if live_member is not None:
                                descriptor, member_identity = live_member
                                descriptors[member] = descriptor
                                member_identity["command"] = [argument.decode() for argument in member_identity["command"]]
                                member_identities.append(member_identity)
                    require(self.pidfd_live(compiler_fd) and self.pidfd_live(leader_fd),
                            "compiler or job leader exited before first control signal")
                    signal.pidfd_send_signal(compiler_fd, signal.SIGSTOP)
                    (folder / "pending-before-cancel.json").write_bytes(marker.read_bytes())
                    (folder / "runtime-before-cancel.json").write_bytes(runtime_path.read_bytes())
                    (folder / "compiler.started").write_bytes(started_path.read_bytes())
                    (folder / "source.rs").write_bytes(source.read_bytes())
                    for record in (identity, leader):
                        record["command"] = [argument.decode() for argument in record["command"]]
                    write(folder / "owned-processes.json", {"compiler": identity, "leader": leader,
                          "pidfd_members": sorted(descriptors), "members": member_identities,
                          "binding": binding.decode(), "job": job["id"]})
                    break
                time.sleep(0.01)
            else:
                raise RuntimeError("no live owned target compiler reached cancellation")
            cancelled = json.loads(self.call("interrupt-cancel", [str(self.args.sail), "jobs", "cancel", job["id"], "--output", "json"]))
            require(cancelled["response"] == "status" and cancelled["value"]["state"] == "cancelled", "public job did not cancel")
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                ready = select.select(list(descriptors.values()), [], [], 0)[0]
                remaining = self.group_processes(identity["group"])
                if len(ready) == len(descriptors) and not remaining:
                    break
                time.sleep(0.02)
            else:
                raise RuntimeError("owned compiler group survived public cancellation")
            response = json.loads(self.call("interrupt-result", [str(self.args.sail), "jobs", "result", job["id"], "--output", "json"]))
            require(response["response"] == "result" and response["value"]["job"]["state"] == "cancelled"
                    and response["value"]["result"] is None, "cancelled job published a proof result")
            require(marker.read_bytes() == (folder / "pending-before-cancel.json").read_bytes()
                    and digest(source) == expected_source, "interrupted marker or source changed before restart")
            (folder / "pending-after-cancel.json").write_bytes(marker.read_bytes())
            write(folder / "extinction.json", {"all_opened_pidfds_exited": True, "remaining_group_members": remaining,
                  "scope": "public job cancellation; no arbitrary parent-crash or power-loss guarantee"})
        finally:
            # Every signal addresses an already opened owned PID, never a reused
            # numeric PID. The public worker owns the complete cancellation tree.
            if owned and compiler_fd is not None:
                for sig in (signal.SIGCONT, signal.SIGKILL):
                    try:
                        signal.pidfd_send_signal(compiler_fd, sig)
                    except ProcessLookupError:
                        pass
            for descriptor in descriptors.values():
                if owned:
                    try:
                        signal.pidfd_send_signal(descriptor, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                os.close(descriptor)
            for descriptor in unverified:
                os.close(descriptor)
        restarted, target = self.verify("interrupt-restart")
        self.observe("interrupt-restart", restarted, target, "SAIL_PROVENANCE_DIR", "ghost", True)
        require(digest(source) == expected_source, "restart changed interrupted source")
        repeated, target = self.verify("interrupt-repeat")
        self.observe("interrupt-repeat", repeated, target, "SAIL_PROVENANCE_DIR", "ghost", True)
        source.write_text(BASE)
        self.env["SAIL_MANAGED_FORCE"] = "1"
        try:
            recovered, target = self.verify("interrupt-read-removed")
            self.observe("interrupt-read-removed", recovered, target)
        finally:
            self.env.pop("SAIL_MANAGED_FORCE", None)
        self.verify("interrupt-recovered-warm", fresh=False, original=target)
        self.cases.append({"label": "actual-job-cancellation", "job_id": job["id"], "run_id": run.name,
                           "compiler_identity": unit, "passed": True})

    def retained_result(self, label, original=None, fresh=True, allow_preparation=False, compiler_restore=False):
        document = json.loads(self.call(label, [str(self.args.sail), "verify", "--output", "json",
                                                "-p", "vest_lib", "--", "--rlimit", "100"]))
        write(self.out / ("projects/" + label + ".json"), document)
        if allow_preparation and document.get("currentness") in ("historical", "unknown"):
            require(document.get("exit_code") == 0 and document.get("error") is None and document.get("no_reuse") is None,
                    f"{label}: retained discovery did not successfully verify")
            return document, None
        require(document.get("schema_version") == 2 and document.get("kind") == "sail-project-result"
                and document.get("currentness") == "current" and document.get("exit_code") == 0
                and document.get("error") is None and document.get("no_reuse") is None,
                f"{label}: retained fixture is not a successful Current run")
        targets = [target for target in document["targets"] if target["requested"]]
        require(len(targets) == 1 and targets[0]["target"] == "vest_lib" and targets[0]["complete"],
                f"{label}: whole Vest target missing")
        target = targets[0]
        functions = [function for crate in target["report"]["crates"] for function in crate["functions"]]
        require(functions and all(function["verdict"] == "verified" and function["establishment"] == "confirmed"
                                  for function in functions), f"{label}: incomplete or failed Vest proof")
        count = document["execution"]["verifier_invocations"]
        require(target["reused"] is (not fresh) and (count > 0 if fresh else count == 0),
                f"{label}: expected fresh proof or genuine zero-verifier restore missing")
        require(target["establishing_run"] == (document["run_id"] if fresh else original["establishing_run"]),
                f"{label}: wrong establishing attempt")
        if original is not None:
            if compiler_restore:
                require_compiler_restore_report(target["report"], original["report"], self.pins["z3"]["version"])
            else:
                require(target["report"] == original["report"], f"{label}: original reused report changed")
        self.cases.append({"label": label, "passed": True, "run_id": document["run_id"],
                           "establishing_run": target["establishing_run"], "compiler_invocations": count})
        self.save()
        return document, target

    def retained_evidence(self, label, document, target, restored_from=None):
        run = self.run_directory(document)
        calls = [(path, json.loads(path.read_bytes())) for path in (run / "invocations").glob("*.json")]
        calls = [(path, call) for path, call in calls
                 if call["identity"]["manifest"] == str(self.project / "vest_lib/Cargo.toml")]
        require(len(calls) == 1, f"{label}: no actual retained target invocation")
        path, call = calls[0]
        require(call["run_id"] == document["run_id"] and call["success"] is True
                and call.get("retained_from") == restored_from and call["report"] == target["report"],
                f"{label}: compiler/restore origin was not preserved")
        folder = self.out / "compiler" / label
        folder.mkdir(parents=True)
        for filename, actual in (("invocation.json", path), ("source.rs", self.project / "vest_lib/src/lib.rs"),
                                 ("runtime.json", run.parent.parent / "runtime.json")):
            (folder / filename).write_bytes(actual.read_bytes())
        runtime = json.loads((run.parent.parent / "runtime.json").read_bytes())
        require(not (Path(runtime["compiler_read_origins"]) / "pending" / (path.stem + ".json")).exists(),
                f"{label}: eligible same-unit proof left an origin marker")
        receipts = [(entry, json.loads(entry.read_bytes())) for entry in (run.parent.parent / "receipts").glob("*.json")]
        receipts = [(entry, receipt) for entry, receipt in receipts
                    if receipt["invocation"]["identity"] == call["identity"]]
        require(len(receipts) == 1 and receipts[0][1]["version"] == 10
                and receipts[0][1]["invocation"].get("retained_from") == restored_from,
                f"{label}: committed original-lineage receipt missing")
        (folder / "receipt.json").write_bytes(receipts[0][0].read_bytes())
        (folder / "Cargo.toml").write_bytes((self.project / "Cargo.toml").read_bytes())
        (folder / "Cargo.lock").write_bytes((self.project / "Cargo.lock").read_bytes())
        for started in (run / "invocations").glob("*.started"):
            (folder / started.name).write_bytes(started.read_bytes())
        for name in ("verus.stdout", "verus.stderr"):
            (folder / name).write_bytes((run / "sidecars" / path.stem / name).read_bytes())
        if restored_from is None:
            observations = [(entry, json.loads(entry.read_bytes())) for entry in
                            (run / "sidecars" / path.stem).rglob("inputs.json")]
            observations = [(entry, raw) for entry, raw in observations if raw.get("kind") == "sail-compiler-read-inputs"]
            require(len(observations) == 1, f"{label}: actual fresh compiler document missing")
            entry, raw = observations[0]
            binding = hashlib.sha256((document["run_id"] + "\0" + path.stem).encode()).hexdigest()
            require(raw.get("schema") == 2 and raw.get("complete") is True and raw.get("pid", 0) > 0
                    and raw.get("binding") == binding and set(raw["phases"]) == {"verification", "compilation"}
                    and set(raw["driver_environment"]) == {COMMON}, f"{label}: fresh source observation invalid")
            union = {}
            for phase in raw["phases"].values():
                require(phase["sources"], f"{label}: empty source phase")
                for name, value in phase["tracked_environment"].items():
                    require(name not in union or union[name] == value, f"{label}: inconsistent phase environment")
                    union[name] = value
            require(call["compiler_environment"] == union and not any(name in union for name in OVERLAYS),
                    f"{label}: raw source values changed or source-sensitive retained proof admitted")
            (folder / "inputs.json").write_bytes(entry.read_bytes())
        else:
            require(not list((run / "invocations").glob("*.started")), f"{label}: restore started a verifier")
        return path.stem, call

    def retained_control(self):
        """Use a pinned prepared public Vest copy; never alter retention costs."""
        require(self.args.retained_project is not None, "lifecycle scope requires a prepared pinned public Vest checkout")
        original_project = self.args.retained_project.resolve(strict=True)
        pin = next(project for project in tomllib.loads((self.repo / "corpus/corpus.toml").read_text())["project"]
                   if project["name"] == "vest")
        require(git(original_project, "rev-parse", "HEAD").decode().strip() == pin["commit"]
                and Path(git(original_project, "rev-parse", "--show-toplevel").decode().strip()).resolve() == original_project,
                "retained source is not the pinned complete Vest checkout")
        changed = set(git(original_project, "diff", "--name-only").decode().splitlines())
        changed.update(git(original_project, "diff", "--cached", "--name-only").decode().splitlines())
        require(all(Path(name).name in ("Cargo.toml", "Cargo.lock") for name in changed),
                "prepared retained source changes more than its public dependency manifests/lock")
        untracked = git(original_project, "ls-files", "--others", "--exclude-standard", "-z").decode().split("\0")
        require(all(not name or Path(name).name == "Cargo.lock" for name in untracked), "unexpected untracked retained inputs")
        names = set(git(original_project, "ls-files", "-z").decode().split("\0")) | set(untracked)
        names.discard("")
        entries = {}
        for entry in git(original_project, "ls-tree", "-r", "-z", pin["commit"]).split(b"\0"):
            if entry:
                header, name = entry.split(b"\t", 1)
                mode, kind, oid = header.decode().split()
                require(mode in ("100644", "100755") and kind == "blob", "unsupported pinned Vest tree member")
                entries[name.decode()] = oid
        tracked = set(git(original_project, "ls-files", "-z").decode().split("\0")) - {""}
        require(tracked == set(entries), "Vest index membership differs from the canonical pinned tree")
        ordered = sorted(entries)
        environment = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
        batch = subprocess.run(["git", "--no-replace-objects", "-c", "core.fsmonitor=false",
                                "-c", f"safe.directory={original_project}", "cat-file", "--batch"],
                               cwd=original_project, env=environment,
                               input="".join(entries[name] + "\n" for name in ordered).encode(),
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
        stream = io.BytesIO(batch.stdout)
        canonical = {}
        for name in ordered:
            oid, kind, size = stream.readline().decode().split()
            require(oid == entries[name] and kind == "blob", "canonical Vest batch object mismatch")
            body = stream.read(int(size))
            require(len(body) == int(size) and stream.read(1) == b"\n", "truncated canonical Vest object")
            canonical[name] = body
        require(stream.read() == b"", "unexpected trailing canonical Vest object data")
        initial = {name: digest(original_project / name) for name in names}
        for name, body in canonical.items():
            if Path(name).name not in ("Cargo.toml", "Cargo.lock"):
                require(initial[name] == hashlib.sha256(body).hexdigest(),
                        f"actual Vest source differs from canonical pinned Git blob: {name}")
        manifest_diff = []
        for name in sorted(names):
            if Path(name).name in ("Cargo.toml", "Cargo.lock"):
                actual = (original_project / name).read_bytes()
                require(hashlib.sha256(actual).hexdigest() == initial[name], "prepared manifest changed during capture")
                manifest_diff.extend(difflib.unified_diff(canonical.get(name, b"").decode().splitlines(keepends=True),
                    actual.decode().splitlines(keepends=True), fromfile="pinned/" + name, tofile="prepared/" + name))
        (self.out / "retained-canonical-manifest.patch").write_text("".join(manifest_diff))
        destination = self.out / "work/retained-vest"
        for name in sorted(names):
            source = original_project / name
            require(source.is_file() and not source.is_symlink(), "retained fixture contains unsupported tracked links")
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            require(digest(target) == initial[name], "retained source changed while copying its exact bytes")
        write(self.out / "retained-source.json", {"corpus_commit": pin["commit"], "files": initial,
              "canonical_blobs": {name: {"git_oid": entries[name], "sha256": hashlib.sha256(body).hexdigest()}
                                  for name, body in canonical.items()}})
        (self.out / "retained-manifest.patch").write_bytes(git(original_project, "diff", "HEAD", "--", "**/Cargo.toml", "Cargo.toml", "**/Cargo.lock", "Cargo.lock"))
        previous = self.project
        self.project = destination
        try:
            source = self.project / "vest_lib/src/lib.rs"
            original = source.read_bytes()
            self.report["retained_preparation"] = []
            for attempt in range(3):
                first, a1 = self.retained_result(f"retained-a1-attempt-{attempt}", allow_preparation=True)
                if a1 is not None:
                    break
                self.report["retained_preparation"].append({"run_id": first["run_id"], "qualification_credit": False})
                self.save()
            else:
                raise RuntimeError("no complete fresh Current retained A1; no forced/altered-cost substitute is permitted")
            unit, call = self.retained_evidence("retained-a1", first, a1)
            source.write_bytes(original + b"\n// Compiler-origin retained B witness.\n")
            edited, b = self.retained_result("retained-b")
            b_unit, _ = self.retained_evidence("retained-b", edited, b)
            require(b_unit == unit and edited["context_sha256"] == first["context_sha256"], "B changed retained unit/context")
            source.write_bytes(original)
            restored, a2 = self.retained_result("retained-a2", a1, False, compiler_restore=True)
            a_unit, restored_call = self.retained_evidence("retained-a2", restored, a2, first["run_id"])
            require(a_unit == unit and restored_call["compiler_environment"] == call["compiler_environment"]
                    and restored["context_sha256"] == first["context_sha256"] and source.read_bytes() == original,
                    "A2 did not retain exact original source/unit/environment lineage")
            for name in ("verus.stdout", "verus.stderr"):
                require((self.out / "compiler/retained-a2" / name).read_bytes()
                        == (self.out / "compiler/retained-a1" / name).read_bytes(),
                        "A2 did not replay original compiler output bytes")
            self.retained_result("retained-a3", a2, False)
            require({name: digest(original_project / name) for name in names} == initial,
                    "external retained source fixture changed during qualification")
        finally:
            self.project = previous

    def capture_origin_state(self, origin, output):
        output.mkdir(parents=True)
        state = {"original_directory": str(origin), "files": {}}
        for path in sorted(origin.rglob("*")):
            if path.is_file():
                relative = path.relative_to(origin)
                destination = output / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(path.read_bytes())
                state["files"][str(relative)] = digest(path)
        write(output / "inventory.json", state)

    def close(self):
        # Cancel only jobs in this runner's private service/cache before stopping
        # that service, so failure cannot leave an owned verifier writing here.
        if self.daemon is not None and self.daemon.poll() is None:
            try:
                listing = json.loads(self.call("cleanup-list", [str(self.args.sail), "jobs", "list", "--output", "json"], timeout=10))
                require(listing["response"] == "list", "cleanup job listing failed")
                for job in listing["value"]:
                    if job["state"] in ("queued", "running"):
                        self.call("cleanup-cancel", [str(self.args.sail), "jobs", "cancel", job["id"], "--output", "json"], timeout=10)
            except Exception as error:
                self.report["cleanup_failure"] = str(error)
                self.report["implemented_scope_complete"] = False
            self.daemon.terminate()
            try:
                self.daemon.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(self.daemon.pid, signal.SIGKILL); self.daemon.wait(timeout=10)
        if self.daemon_log is not None:
            self.daemon_log.close()
        if self.identity_ready:
            require(self.call("rustc-version-final", ["rustup", "run", self.env["RUSTUP_TOOLCHAIN"], "rustc", "-vV"], self.repo) == self.rustc_identity, "compiler toolchain changed during execution")
            require(git(self.repo, "rev-parse", "HEAD").decode().strip() == self.head and source_inventory(self.repo) == self.sources,
                    "qualification source changed during execution")
            require(self.call("verus-version-final", [str(self.args.runtime_root / "verus"), "--version", "--output-json"], self.repo) == self.verus_identity,
                    "actual verifier identity changed during execution")
            require(runtime_inventory(self.args.build) == self.runtime and digest(self.args.sail) == self.report["sail_sha256"]
                    and digest(self.args.sail.with_name("sail-z3")) == self.report["shim_sha256"], "runtime/binary changed during execution")
            self.report["final_source_runtime_checked"] = True
        self.save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("sail", "build", "build-receipt", "host-record", "out"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--timeout", type=int, default=240, help="wall deadline per real verification; solver resources are unchanged")
    parser.add_argument("--scope", choices=("basic", "lifecycle"), default="basic")
    parser.add_argument("--retained-project", type=Path, help="prepared pinned public Vest checkout; lifecycle only")
    args = parser.parse_args()
    require(os.environ.get("CI") != "true" and os.environ.get("GITHUB_ACTIONS") != "true", "local-only qualification refuses CI")
    require(platform.system() in ("Linux", "Darwin"), "qualification requires an owned native Linux/macOS host record")
    require(args.timeout > 0, "timeout must be positive")
    for name in ("sail", "build", "build_receipt", "host_record"):
        setattr(args, name, getattr(args, name).resolve(strict=True))
    args.runtime_root = (args.build / "verus/source/target-verus/release").resolve(strict=True)
    args.cargo_verus = (args.runtime_root / "cargo-verus").resolve(strict=True)
    require(not args.build_receipt.is_relative_to(args.build), "build receipt must be outside the immutable installed inventory")
    qualification = Qualification(args)
    try:
        qualification.execute()
    except Exception as error:
        qualification.report["failure"] = f"{type(error).__name__}: {error}"
        (qualification.out / "failure.log").write_text(traceback.format_exc())
    finally:
        try:
            qualification.close()
        except Exception as error:
            qualification.report["failure"] = (qualification.report["failure"] or "") + f"; final guard: {error}"
            qualification.report["implemented_scope_complete"] = False
            qualification.save()
    print(json.dumps({key: qualification.report.get(key) for key in (
        "implemented_scope_complete", "overall_qualification_complete", "failure", "seconds", "unrun_required_overall")}))
    return 0 if qualification.report["implemented_scope_complete"] and not qualification.report["failure"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
