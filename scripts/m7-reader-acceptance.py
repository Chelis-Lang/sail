#!/usr/bin/env python3
"""Local warm-reader acceptance: real setup, authoritative proof, then unchanged censuses.

See docs/reader-acceptance.md for invocation, archival layouts and failure handling.
Setup follows the frozen repair workload: ordinary independent full verification,
then the exact repair-function context on unchanged source. Every setup cost and
failure is retained separately from reader latency. No prior receipt is copied.
"""
import argparse
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import re
import stat
import shutil
import subprocess
import time


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


HERE = Path(__file__).resolve().parent
bench = load("warm_reader_workload", HERE / "m7-owned-actors.py")
protocol = bench.protocol
retained = load("warm_reader_admission", HERE / "m7-retained-jobs-acceptance.py")
native_product = load("warm_reader_native_setup", HERE / "m7-native-product.py")


def copy_source(source, destination, target_directory):
    """Exclude actual Cargo output, preserving legitimate source directories named target."""
    source = source.resolve()
    excluded = Path(target_directory).resolve()
    def ignore(directory, entries):
        return [name for name in entries if name == ".git"
                or (Path(directory) / name).resolve() == excluded]
    shutil.copytree(source, destination, ignore=ignore, symlinks=True)


def whole_success(project):
    assert project["error"] is None and project["exit_code"] == 0, "full verification failed"
    assert project["targets"], "full proof has no target inventory"
    for target in project["targets"]:
        report = target["report"]
        assert target["complete"] and report["error"] is None and report["exit_code"] == 0, target
        assert report["crates"] and all(crate["verus"]["errors"] == 0
               and crate["verus"]["success"] is True
               and crate["verus"]["encountered_vir_error"] is False for crate in report["crates"]), target


def setup_state(document, manifest, allow_declarations):
    """Permit only the catalogue's explicit first-run declaration establishment transition."""
    assert document["batch"]["manifest_sha256"] == manifest, "setup used another source runtime"
    assert document["batch"].get("selection_error") is None, "setup selected no exact function"
    whole_success(document["batch"]["full"])
    return bench.warm_repair_state(document, allow_setup=allow_declarations)


def same_scope(project, package_id, target, kind):
    entries = [entry for entry in project["targets"] if entry["requested"]]
    assert len(entries) == 1, "setup must verify one exact requested target"
    entry = entries[0]
    assert (entry["package_id"], entry["target"], entry["kind"]) == (package_id, target, [kind]), entry


def admit(document, manifest, package_id, target, kind, function):
    result, _, _ = retained.selected(document, package_id, target, kind, function)
    assert document["repair"]["batch"]["manifest_sha256"] == manifest
    assert document["repair"]["batch"].get("selection_error") is None
    return result


PROJECTION_NOTE = "SAIL3114: current input validation is unavailable or still running; native coverage and kernel read leases must be established before claiming Current"


def pending_projection(document, original, manifest, package_id, target, kind, function):
    """Authorize waiting, never proof admission, from exact owned original evidence."""
    job, public = document["job"], document["result"]
    if (job["state"] != "completed" or job["error"] is not None or public is None
            or public["currentness"] != "unknown" or original.get("accepted") is not True):
        return False
    batch = original["batch"]
    full = batch["full"]
    if full["currentness"] != "current":
        return False
    assert batch["manifest_sha256"] == manifest and batch["selection_error"] is None
    assert batch["batched"] and batch["focus"] is not None
    whole_success(full)
    same_scope(full, package_id, target, kind)
    assert job["input_revision"] == full["input_revision"]
    projected = dict(full, currentness="unknown", notes=full["notes"] + [PROJECTION_NOTE])
    assert public == projected, "Unknown must be the projection of this exact owned Current report"
    functions = [item for target in full["targets"] if target["requested"]
                 for crate in target["report"]["crates"] for item in crate["functions"]
                 if item["function"] == function]
    assert len(functions) == 1 and functions[0]["verdict"] == "verified" and functions[0]["tier"] == "confirmed"
    return True


def original_json(stream):
    """Bounded, duplicate-free owned evidence; never a replacement for public admission."""
    payload = stream.read(32 * 1024 * 1024 + 1)
    assert len(payload) <= 32 * 1024 * 1024, "owned document exceeds protocol bound"
    def unique(pairs):
        value = {}
        for key, item in pairs:
            assert key not in value, "duplicate owned document field"
            value[key] = item
        return value
    return json.loads(payload, object_pairs_hook=unique), payload


def owned_file(directory, name):
    """Open one regular retained file without following a substituted symlink."""
    descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
    try:
        assert stat.S_ISREG(os.fstat(descriptor).st_mode), "owned evidence is not a regular file: " + name
        stream = os.fdopen(descriptor, "rb")
    except BaseException:
        os.close(descriptor)
        raise
    return stream


@contextmanager
def owned_job(cache, job, expected=None):
    """Resolve exactly one known layout through pinned, non-symlink directory descriptors."""
    assert isinstance(job, str) and re.fullmatch(r"[A-Za-z0-9_-]+", job), "unsafe owned job ID"
    directories = []
    anchor = os.open(cache.resolve(strict=True), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for layout in ("projects/jobs", "results/projects/jobs"):
            descriptor = os.dup(anchor)
            try:
                for part in (*layout.split("/"), job):
                    child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor)
                    os.close(descriptor)
                    descriptor = child
            except FileNotFoundError:
                os.close(descriptor)
                continue
            except BaseException:
                os.close(descriptor)
                raise
            directories.append((descriptor, layout))
        assert len(directories) == 1, "owned job layout is missing or ambiguous: " + job
        descriptor, layout = directories[0]
        with owned_file(descriptor, "record.json") as stream:
            record, _ = original_json(stream)
        status, submission = record["status"], record["submission"]
        assert status["version"] == 1 and status["id"] == job, "owned record identity mismatch"
        assert status["state"] in ("queued", "running", "completed", "cancelled", "superseded", "infrastructure_error"), "owned record lifecycle mismatch"
        assert (isinstance(status["project_dir"], str) and Path(status["project_dir"]).is_absolute()
                and status["project_dir"] == submission["project_dir"]
                and status["args"] == submission["args"]), "owned record scope mismatch"
        if expected is not None:
            for field in ("id", "project_dir", "args"):
                assert status[field] == expected[field], "owned public/record mismatch: " + field
            if "input_revision" in expected:
                assert status["input_revision"] == expected["input_revision"], "owned public/record mismatch: input_revision"
        yield descriptor, record, layout
    finally:
        for descriptor, _ in directories:
            os.close(descriptor)
        os.close(anchor)


def owned_original(cache, job, expected):
    """Read only the requested owned job, with exact sealed bytes when a seal exists."""
    with owned_job(cache, job, expected) as (directory, record, _):
        with owned_file(directory, "stdout") as stream:
            document, payload = original_json(stream)
        projection = record.get("projection")
        if projection is not None:
            assert hashlib.sha256(payload).hexdigest() == projection["stdout_sha256"], "owned output seal mismatch"
        return document


def archive_owned_job(cache, job, destination, expected=None):
    """Retain raw known files and explicit missing-file evidence, excluding secret record fields."""
    with owned_job(cache, job, expected) as (directory, record, layout):
        destination.mkdir(parents=True, exist_ok=False)
        copied, missing = {}, []
        for name in ("stdout", "stderr", "events.jsonl", "proof-source.jsonl"):
            try:
                stream = owned_file(directory, name)
            except FileNotFoundError:
                missing.append(name)
                continue
            with stream, (destination / name).open("xb") as output:
                digest, length = hashlib.sha256(), 0
                before = os.fstat(stream.fileno())
                while chunk := stream.read(1024 * 1024):
                    output.write(chunk)
                    digest.update(chunk)
                    length += len(chunk)
                after = os.fstat(stream.fileno())
            copied[name] = {"bytes": length, "sha256": digest.hexdigest(),
                            "changed_during_copy": (before.st_size, before.st_mtime_ns, before.st_ctime_ns)
                            != (after.st_size, after.st_mtime_ns, after.st_ctime_ns)}
        evidence = {"job": job, "layout": layout, "state": record["status"]["state"],
                    "files": copied, "missing": missing}
        bench.write_json(destination / "archive.json", evidence)
        assert record["status"]["state"] != "completed" or "stdout" not in missing, "completed owned job output is missing"
        return evidence


def wait_process(child, timeout, stage):
    """Report preparation progress while preserving the selected process deadline."""
    began = time.monotonic()
    while True:
        remaining = None if timeout is None else timeout - (time.monotonic() - began)
        if remaining is not None and remaining <= 0:
            raise subprocess.TimeoutExpired(child.args, timeout)
        try:
            return child.wait(timeout=10 if remaining is None else min(10, remaining))
        except subprocess.TimeoutExpired:
            if timeout is not None and time.monotonic() - began >= timeout:
                raise subprocess.TimeoutExpired(child.args, timeout) from None
            print(f"preparation {stage}: {time.monotonic() - began:.1f}s; owned process {child.pid}", flush=True)


def admit_census(value, job, active, revision, crate):
    """Bind the unmodified collector result to this proof and exact live crate."""
    assert value["passed"] is True
    assert value["job"] == job and value["active_job"] == active
    assert value["input_revision"] == revision and value["active_crate"] == crate
    assert value["clients"] == 4 and len(value["samples"]) == 200
    assert value["deadline_seconds"] == 5
    assert set(value["summary"]) == {"status", "result"}
    for operation, summary in value["summary"].items():
        assert summary["count"] == 100 and summary["p95_ms"] < 200
        samples = [sample for sample in value["samples"] if sample["operation"] == operation]
        assert len(samples) == 100
        for sample in samples:
            assert sample["current"] is True
            # The Python and Rust collectors retain distinct public evidence schemas.
            if "verifier_continued" in sample:
                assert sample["verifier_continued"] is True
                assert sample["active_before"] == sample["active_after"] == "running"
            else:
                assert sample["active_before"] is True and sample["active_after"] is True
                assert sample["error"] is None
                assert protocol.continuing_verifier(sample["verifiers_before"], sample["verifiers_after"])


class Service(bench.OwnedDaemons):
    def __init__(self, output, keep):
        super().__init__(output)
        self.keep = keep

    def __exit__(self, kind, failure, traceback):
        if self.keep and kind is None:
            for mode, (actor, stdout, stderr, _) in self.actors.items():
                assert actor.poll() is None, "retained owned daemon exited"
                self.records.append({"mode": mode, "pid": actor.pid, "retained": True})
                stdout.close()
                stderr.close()
            bench.write_json(self.output / "actor-lifecycle.json", {"actors": self.records, "cleanup_errors": []})
            return False
        return super().__exit__(kind, failure, traceback)


class Run:
    def __init__(self, args):
        self.args = args
        self.out = args.out.resolve()
        self.out.mkdir(parents=True, exist_ok=False, mode=0o700)
        self.bin = args.bin_dir.resolve()
        self.source = args.source_workspace.resolve()
        self.probe_dir = args.probe_bin_dir.resolve()
        self.probe = load("actual_frozen_reader_probe", args.python_probe.resolve())
        self.cache = self.out / "cache"
        self.socket = self.out / "cache.sock"
        self.env = dict(os.environ, SAIL_CACHE_DIR=str(self.cache),
                        SAIL_DAEMON_SOCKET=str(self.socket), SAIL_MANAGED_FORCE="1")
        self.owned = []
        self.job_started = {}
        self.job_scopes = {}
        self.daemon = None
        self.summary = {"version": 1, "kind": "real-warm-reader-acceptance", "passed": False,
                        "platform": platform.platform(), "environment_label": args.environment_label,
                        "setup_contract": "ordinary complete full proof, then exact repair-function context",
                        "setup_invocation_budget_s": args.setup_timeout or None, "cli_budget_s": 30, "rpc_budget_s": 5,
                        "clients": 4, "samples_per_collector": 200, "p95_limit_ms": 200,
                        "setup": [], "commands": [], "proofs": {}, "censuses": {}, "cleanup_errors": [],
                        "worker_archives": {}, "archival_errors": [],
                        "unmeasured_preparation": {"enabled": args.prepare_contexts, "phases": []}}
        self.save()

    def save(self):
        bench.write_json(self.out / "summary.json", self.summary)

    def command(self, name, arguments, workspace, env=None, timeout=30, json_output=True):
        directory = self.out / name
        directory.mkdir(parents=True, exist_ok=False)
        began = time.monotonic()
        child = None
        failure = None
        with (directory / "stdout").open("wb") as stdout, (directory / "stderr").open("wb") as stderr:
            try:
                child = subprocess.Popen(arguments, cwd=workspace, env=env or self.env,
                                         stdout=stdout, stderr=stderr, start_new_session=True)
                wait_process(child, timeout, name)
            except BaseException as error:
                failure = {"type": type(error).__name__, "message": str(error)}
                if child is not None:
                    try:
                        bench.interrupt_owned(child)
                    except BaseException as cleanup:
                        failure["cleanup_error"] = {"type": type(cleanup).__name__, "message": str(cleanup)}
                raise
            finally:
                record = {"stage": name, "wall_ms": (time.monotonic() - began) * 1000,
                          "exit_code": child.returncode if child is not None else None,
                          "command": list(map(str, arguments)), "deadline_s": timeout,
                          "failure": failure, "measured_reader_latency": False}
                bench.write_json(directory / "command.json", record)
                self.summary["commands"].append(record)
                if not name.endswith("-census"):
                    self.summary["setup"].append(record)
                self.save()
        assert child.returncode == 0, (name, child.returncode)
        return json.loads((directory / "stdout").read_bytes()) if json_output else None

    def freeze(self):
        self.summary["controller_scripts"] = {name: bench.sha(HERE / name) for name in (
            "m7-reader-acceptance.py", "m7-owned-actors.py",
            "m7-retained-jobs-acceptance.py", "m7-job-read-bench.py", "m7-native-product.py")}
        frozen = json.loads((self.bin / "freeze.json").read_bytes())
        for name in ("sail", "sail-z3"):
            assert frozen["files"][name]["sha256"] == bench.sha(self.bin / name), "product freeze changed"
        probe_freeze = json.loads((self.probe_dir / "freeze.json").read_bytes())
        for name, path in [("job_read_bench", self.probe_dir / "job_read_bench"),
                           ("python-reader-probe.py", self.args.python_probe.resolve())]:
            assert probe_freeze["files"][name]["sha256"] == bench.sha(path), "collector freeze changed"
        # Installation checks are environmental prerequisites, never Current authority.
        product_setup = native_product.audit_products(self.bin)
        self.summary["product_ownership_prerequisite"] = product_setup
        self.save()
        if not product_setup["ownership_prerequisite_met"]:
            raise ValueError("SAIL0948: installed products need service filesystem UID ownership or effective CAP_LEASE; create new byte-verified copies")
        inventory = getattr(self.args, "lease_inventory", None)
        if inventory is not None:
            rows, digest = native_product.load_json(inventory)
            setup = native_product.audit_bindings(rows)
            setup["inventory_sha256"] = digest
            self.summary["supplied_guard_ownership_prerequisite"] = setup
            self.save()
            if not setup["ownership_prerequisite_met"]:
                raise ValueError("SAIL0948: supplied candidate Guard inventory has foreign-owned files without effective CAP_LEASE")
        self.summary["product_freeze"] = frozen
        self.summary["collector_freeze"] = probe_freeze
        self.save()

    def metadata(self, name, workspace):
        return self.command(name, ["cargo", "metadata", "--no-deps", "--format-version", "1"], workspace)

    def selection(self):
        suffix = ["--lib"] if self.args.kind == "lib" else ["--bin", self.args.target]
        return ["--package", self.args.package, *suffix]

    def submit(self, label, workspace):
        began = time.monotonic()
        env = dict(self.env, CARGO_TARGET_DIR=str(workspace / "target"))
        job = self.command(label + "-submit", [str(self.bin / "sail"), "repair-check",
                           "--engine", "interactive", "--function", self.args.function,
                           "--background", "--output", "json", *self.selection()], workspace, env)
        self.owned.append(job["id"])
        self.job_scopes[job["id"]] = {field: job[field] for field in ("id", "project_dir", "args")}
        self.job_started[job["id"]] = began
        return job["id"]

    def wait(self, label, job, manifest, package_id):
        began = self.job_started[job]
        address = self.probe.endpoint(self.socket)
        poll = 0
        next_progress = 0
        awaiting_projection = False
        result_reads = 0
        while not self.args.setup_timeout or time.monotonic() - began < self.args.setup_timeout:
            if time.monotonic() - began >= next_progress:
                print(f"setup {label}: {time.monotonic() - began:.1f}s; job {job}", flush=True)
                next_progress += 10
            response, elapsed, length = self.probe.call(address, {"operation": "status", "id": job})
            assert response["response"] == "status", response
            with (self.out / (label + "-lifecycle.jsonl")).open("a") as stream:
                stream.write(json.dumps({"at_unix": time.time(), "elapsed_s": time.monotonic() - began,
                                         "rpc_ms": elapsed, "bytes": length, "response": response}) + "\n")
            if response["value"]["state"] in ("completed", "cancelled", "infrastructure_error", "superseded"):
                if (awaiting_projection and response["value"]["state"] == "completed"
                        and response["value"]["currentness"] == "unknown"):
                    poll += 1
                    time.sleep(.25)
                    continue
                result, _, _ = self.probe.call(address, {"operation": "result", "id": job})
                result_reads += 1
                bench.write_json(self.out / (label + f"-result-{result_reads}.json"), result)
                bench.write_json(self.out / (label + "-result.json"), result)
                assert result["response"] == "result", result
                document = result["value"]
                if document["result"] is not None and document["result"]["currentness"] == "unknown":
                    original = owned_original(self.cache, job, document["job"])
                    if pending_projection(document, original, manifest, package_id, self.args.target,
                                          self.args.kind, self.args.function):
                        awaiting_projection = True
                        poll += 1
                        time.sleep(.25)
                        continue
                self.summary["setup"].append({"stage": label, "job": job,
                    "wall_ms": (time.monotonic() - began) * 1000, "status_reads": poll + 1,
                    "result_reads": result_reads, "awaited_original_current_projection": awaiting_projection,
                    "measured_reader_latency": False})
                self.save()
                return result["value"]
            poll += 1
            time.sleep(.25)
        raise AssertionError("recorded preparation budget exceeded: " + label)

    def warm(self, label, workspace, manifest):
        env = dict(self.env, CARGO_TARGET_DIR=str(workspace / "target"))
        metadata = self.command(label + "-metadata", ["cargo", "metadata", "--no-deps",
                                "--format-version", "1"], workspace, env)
        package_id = retained.package_identity(metadata, self.args.package, self.args.target, self.args.kind)
        full = self.command(label + "-ordinary-full", [str(self.bin / "sail"), "verify", "--incremental",
                             "--output", "json", *self.selection()], workspace, env, self.args.setup_timeout or None)
        whole_success(full)
        same_scope(full, package_id, self.args.target, self.args.kind)
        for attempt in range(2):
            name = label + "-repair-context-" + str(attempt)
            job = self.submit(name, workspace)
            document = self.wait(name, job, manifest, package_id)
            state = setup_state(document["repair"], manifest, attempt == 0)
            same_scope(document["result"], package_id, self.args.target, self.args.kind)
            if state == "ready":
                result = admit(document, manifest, package_id, self.args.target, self.args.kind, self.args.function)
                self.summary["proofs"][label] = {"job": job, "input_revision": result["input_revision"],
                    "package_id": package_id, "workspace": str(workspace), "complete_current": True}
                self.save()
                return job
        raise AssertionError("exact repair context never established genuine Current full evidence")

    def prepare(self, label, workspace, manifest):
        """Explicit setup phase only; never extend the subsequent bounded checks."""
        bounded_budget = self.args.setup_timeout
        assert bounded_budget > 0, "explicit preparation requires a positive subsequent setup budget"
        phase = {"stage": label + "-unmeasured-preparation", "deadline_s": None,
                 "subsequent_setup_budget_s": bounded_budget, "measured_reader_latency": False,
                 "same_owned_workspace": str(workspace), "completed": False}
        self.summary["unmeasured_preparation"]["phases"].append(phase)
        began = time.monotonic()
        try:
            self.args.setup_timeout = 0
            phase["job"] = self.warm(phase["stage"], workspace, manifest)
            phase["completed"] = True
        except BaseException as error:
            phase["failure"] = {"type": type(error).__name__, "message": str(error)}
            raise
        finally:
            self.args.setup_timeout = bounded_budget
            phase["wall_ms"] = (time.monotonic() - began) * 1000
            self.save()

    def establish_workspaces(self, workspaces, manifest):
        if self.args.prepare_contexts:
            for label, workspace in workspaces.items():
                self.prepare(label, workspace, manifest)
        baseline = self.warm("baseline", workspaces["baseline"], manifest)
        self.warm("active", workspaces["active"], manifest)
        return baseline

    def active_worker(self, job, workspace):
        began = time.monotonic()
        while time.monotonic() - began < 600:
            response, _, _ = self.probe.call(self.probe.endpoint(self.socket), {"operation": "status", "id": job})
            assert response["response"] == "status" and response["value"]["state"] in ("queued", "running"), response
            assert self.daemon.poll() is None
            table = self.probe.process_table()
            for pid, row in table.items():
                try:
                    with (Path("/proc") / str(pid) / "cmdline").open("rb") as stream:
                        raw = stream.read(65537)
                    if len(raw) > 65536:
                        continue
                    argv = raw.split(b"\0")
                    cwd = (Path("/proc") / str(pid) / "cwd").resolve()
                except OSError:
                    continue
                if len(argv) < 2 or argv[1] != b"_repair-worker" or cwd != workspace:
                    continue
                ancestor = row["parent"]
                child_start = row["started_ticks"]
                seen = set()
                while ancestor not in seen and ancestor in table and ancestor != self.daemon.pid:
                    seen.add(ancestor)
                    parent = table[ancestor]
                    if parent["started_ticks"] > child_start:
                        break
                    child_start = parent["started_ticks"]
                    ancestor = parent["parent"]
                if ancestor == self.daemon.pid and self.probe.active_verifiers(table, pid, row["started_ticks"], self.args.target.replace('-', '_')):
                    after = self.probe.process_table().get(pid)
                    if after and after["state"] in ("R", "S", "D") and all(after[key] == row[key] for key in ("pid", "parent", "name", "started_ticks")):
                        return row
            time.sleep(.1)
        raise AssertionError("selected target's live actual verifier never observed")

    def census(self, label, argv, job, active):
        # Existing collectors retain their own exact 5-second RPC and 200-ms
        # acceptance thresholds. The process bound is the existing local600.
        self.command(label, argv, self.out, timeout=600, json_output=False)
        value = json.loads((self.out / (label + ".json")).read_bytes())
        admit_census(value, job, active, self.summary["proofs"]["baseline"]["input_revision"],
                     self.args.target.replace('-', '_'))
        self.summary["censuses"][label] = value
        self.save()

    def finish_jobs(self):
        """Archive failures never skip cancellation of exact owned jobs."""
        for job in self.owned:
            try:
                evidence = archive_owned_job(self.cache, job, self.out / "worker-output" / job, self.job_scopes[job])
                self.summary["worker_archives"][job] = evidence
            except (OSError, ValueError, AssertionError, KeyError, TypeError) as error:
                self.summary["archival_errors"].append({"job": job, "type": type(error).__name__, "message": str(error)})
        for job in self.owned:
            try:
                response, _, _ = self.probe.call(self.probe.endpoint(self.socket), {"operation": "status", "id": job})
                assert response["response"] == "status", response
                if response["value"]["state"] in ("queued", "running"):
                    cancelled, _, _ = self.probe.call(self.probe.endpoint(self.socket), {"operation": "cancel", "id": job, "superseded": False})
                    bench.write_json(self.out / ("owned-cancel-" + job + ".json"), cancelled)
                    assert cancelled["response"] == "status" and cancelled["value"]["state"] not in ("queued", "running"), cancelled
            except (OSError, ValueError, AssertionError) as error:
                self.summary["cleanup_errors"].append(str(error))
        self.save()

    def run(self):
        self.freeze()
        imported = self.command("engine-install", [str(self.bin / "sail"), "engine", "install", "--from",
                                str(self.args.source_runtime.resolve()), "--output", "json"], self.source)
        self.env["RUSTUP_TOOLCHAIN"] = imported["rust_toolchain"]
        manifest = imported["manifest_sha256"]
        metadata = self.metadata("source-metadata", self.source)
        workspaces = {name: self.out / name for name in ("baseline", "active")}
        for workspace in workspaces.values():
            copy_source(self.source, workspace, metadata["target_directory"])
        with Service(self.out, self.args.keep_daemon) as service:
            service.start("reader", self.bin / "sail", workspaces["baseline"], self.env)
            self.daemon = service.actors["reader"][0]
            try:
                baseline = self.establish_workspaces(workspaces, manifest)
                active = self.submit("active-measured-proof", workspaces["active"])
                worker = self.active_worker(active, workspaces["active"])
                common = ["--socket", str(self.socket), "--job", baseline, "--active-job", active,
                          "--active-worker", str(worker["pid"]), "--active-crate", self.args.target.replace('-', '_'),
                          "--environment", self.args.environment_label]
                self.census("python-census", ["python3", str(self.args.python_probe.resolve()), *common,
                            "--clients", "4", "--samples", "200", "--out", str(self.out / "python-census.json")], baseline, active)
                self.census("rust-census", [str(self.probe_dir / "job_read_bench"), *common,
                            "--out", str(self.out / "rust-census.json")], baseline, active)
                self.summary["passed"] = True
                self.summary["retained_daemon_pid"] = self.daemon.pid if self.args.keep_daemon else None
            finally:
                self.finish_jobs()
            assert not self.summary["cleanup_errors"], self.summary["cleanup_errors"]
            assert not self.summary["archival_errors"], self.summary["archival_errors"]
        self.save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("bin-dir", "source-runtime", "source-workspace", "python-probe", "probe-bin-dir", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    for name in ("package", "target", "function", "environment-label"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--kind", choices=("lib", "bin"), required=True)
    parser.add_argument("--setup-timeout", type=float, default=600,
                        help="unmeasured preparation budget per proof; 0 explicitly prepares to completion with owned cancellation, without changing measured bounds")
    parser.add_argument("--prepare-contexts", action="store_true",
                        help="record an additional unmeasured complete ordinary/exact-context preparation phase before the original bounded setup checks")
    parser.add_argument("--lease-inventory", type=Path,
                        help="optional complete original Guard binding array for this exact candidate; setup metadata only, never proof authority")
    parser.add_argument("--keep-daemon", action="store_true", help="retain only the successful owned service for real retained-job inspection")
    args = parser.parse_args()
    if not math.isfinite(args.setup_timeout) or args.setup_timeout < 0:
        parser.error("preparation budget must be finite and nonnegative")
    if args.prepare_contexts and args.setup_timeout == 0:
        parser.error("explicit context preparation requires a positive subsequent setup budget")
    if os.environ.get("CI"):
        parser.error("long warm-reader acceptance is local-only")
    if platform.system() != "Linux" or not Path("/proc").is_dir():
        parser.error("actual kernel-bound verifier census requires Linux")
    run = Run(args)
    try:
        run.run()
    except BaseException as error:
        run.summary["passed"] = False
        run.summary["error"] = {"type": type(error).__name__, "message": str(error)}
        run.save()
        raise


if __name__ == "__main__":
    main()
