#!/usr/bin/env python3
"""Local warm-reader acceptance: real setup, authoritative proof, then unchanged censuses.

Setup follows the frozen repair workload: ordinary independent full verification,
then the exact repair-function context on unchanged source. Every setup cost and
failure is retained separately from reader latency. No prior receipt is copied.
"""
import argparse
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import shutil
import subprocess
import time


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


HERE = Path(__file__).resolve().parent
bench = load("warm_reader_workload", HERE / "m7-repair-bench.py")
protocol = bench.protocol
retained = load("warm_reader_admission", HERE / "m7-retained-jobs-acceptance.py")


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
        self.daemon = None
        self.summary = {"version": 1, "kind": "real-warm-reader-acceptance", "passed": False,
                        "platform": platform.platform(), "environment_label": args.environment_label,
                        "setup_contract": "ordinary complete full proof, then exact repair-function context",
                        "setup_invocation_budget_s": args.setup_timeout or None, "cli_budget_s": 30, "rpc_budget_s": 5,
                        "clients": 4, "samples_per_collector": 200, "p95_limit_ms": 200,
                        "setup": [], "commands": [], "proofs": {}, "censuses": {}, "cleanup_errors": []}
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
            "m7-reader-acceptance.py", "m7-repair-bench.py",
            "m7-retained-jobs-acceptance.py", "m7-job-read-bench.py")}
        frozen = json.loads((self.bin / "freeze.json").read_bytes())
        for name in ("sail", "sail-z3"):
            assert frozen["files"][name]["sha256"] == bench.sha(self.bin / name), "product freeze changed"
        probe_freeze = json.loads((self.probe_dir / "freeze.json").read_bytes())
        for name, path in [("job_read_bench", self.probe_dir / "job_read_bench"),
                           ("python-reader-probe.py", self.args.python_probe.resolve())]:
            assert probe_freeze["files"][name]["sha256"] == bench.sha(path), "collector freeze changed"
        self.summary["product_freeze"] = frozen
        self.summary["collector_freeze"] = probe_freeze
        self.save()

    def metadata(self, name, workspace):
        return self.command(name, ["cargo", "metadata", "--no-deps", "--format-version", "1"], workspace)

    def selection(self):
        suffix = ["--lib"] if self.args.kind == "lib" else ["--bin", self.args.target]
        return ["--package", self.args.package, *suffix]

    def submit(self, label, workspace):
        env = dict(self.env, CARGO_TARGET_DIR=str(workspace / "target"))
        job = self.command(label + "-submit", [str(self.bin / "sail"), "repair-check",
                           "--engine", "interactive", "--function", self.args.function,
                           "--background", "--output", "json", *self.selection()], workspace, env)
        self.owned.append(job["id"])
        return job["id"]

    def wait(self, label, job):
        began = time.monotonic()
        address = self.probe.endpoint(self.socket)
        poll = 0
        next_progress = 0
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
                result, _, _ = self.probe.call(address, {"operation": "result", "id": job})
                bench.write_json(self.out / (label + "-result.json"), result)
                assert result["response"] == "result", result
                self.summary["setup"].append({"stage": label, "job": job,
                    "wall_ms": (time.monotonic() - began) * 1000, "status_reads": poll + 1,
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
            document = self.wait(name, job)
            state = setup_state(document["repair"], manifest, attempt == 0)
            same_scope(document["result"], package_id, self.args.target, self.args.kind)
            if state == "ready":
                result = admit(document, manifest, package_id, self.args.target, self.args.kind, self.args.function)
                self.summary["proofs"][label] = {"job": job, "input_revision": result["input_revision"],
                    "package_id": package_id, "workspace": str(workspace), "complete_current": True}
                self.save()
                return job
        raise AssertionError("exact repair context never established genuine Current full evidence")

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
                baseline = self.warm("baseline", workspaces["baseline"], manifest)
                self.warm("active", workspaces["active"], manifest)
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
                for job in self.owned:
                    destination = self.out / "worker-output" / job
                    destination.mkdir(parents=True, exist_ok=True)
                    for name in ("stdout", "stderr", "events.jsonl", "proof-source.jsonl"):
                        source = self.cache / "results/projects/jobs" / job / name
                        if source.is_file():
                            shutil.copyfile(source, destination / name)
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
            assert not self.summary["cleanup_errors"], self.summary["cleanup_errors"]
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
    parser.add_argument("--keep-daemon", action="store_true", help="retain only the successful owned service for real retained-job inspection")
    args = parser.parse_args()
    if not math.isfinite(args.setup_timeout) or args.setup_timeout < 0:
        parser.error("preparation budget must be finite and nonnegative")
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
