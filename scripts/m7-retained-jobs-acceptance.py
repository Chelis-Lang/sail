#!/usr/bin/env python3
"""Bounded real CLI/MCP integration over an existing complete Current repair job.

Local only. Does not edit the project or establish proof/performance evidence.
The caller supplies a genuine completed job from the separate real-project
workload. One explicitly owned forced repair is started and cancelled; no other
job or daemon is stopped. Workflow/command/RPC budgets remain 120/30/5 seconds.
"""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import select
import signal
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("retained_job_protocol", REPO / "scripts/m7-job-read-bench.py")
protocol = importlib.util.module_from_spec(spec)
spec.loader.exec_module(protocol)


def package_identity(metadata, package, target, kind):
    """Resolve an exact expected package/target from the caller's real metadata."""
    packages = [entry for entry in metadata["packages"]
                if package in (entry["name"], entry["id"])]
    assert len(packages) == 1, "package must resolve uniquely in actual Cargo metadata"
    entry = packages[0]
    targets = [item for item in entry["targets"] if item["name"] == target and item["kind"] == [kind]]
    assert len(targets) == 1, "requested target/kind is absent or ambiguous in the expected package"
    return entry["id"]


def selected(document, package_id, target, kind, function):
    """Require genuine independent full evidence and one exact requested scope."""
    job = document["job"]
    result = document["result"]
    repair = document["repair"]
    assert job["state"] == "completed" and job["currentness"] == "current" and job["error"] is None, job
    assert result is not None and result["currentness"] == "current" and result["error"] is None
    assert job["input_revision"] == result["input_revision"], "job status and full result must bind the same input revision"
    assert result["exit_code"] == 0 and repair is not None and repair["accepted"]
    assert repair["batch"]["full"] == result and repair["batch"]["batched"]
    for dependency in result["targets"]:
        report = dependency["report"]
        assert dependency["complete"] and report["error"] is None and report["exit_code"] == 0, dependency
        assert report["crates"] and all(crate["verus"]["errors"] == 0
               and crate["verus"]["success"] is True
               and crate["verus"]["encountered_vir_error"] is False for crate in report["crates"]), dependency
    requested = [entry for entry in result["targets"] if entry["requested"]]
    assert len(requested) == 1, "real integration requires one exact requested Cargo target"
    entry = requested[0]
    assert entry["target"] == target and entry["kind"] == [kind] and entry["complete"], entry
    functions = [item for crate in entry["report"]["crates"] for item in crate["functions"]
                 if item["function"] == function]
    assert len(functions) == 1, "exact function must be uniquely present in the actual full inventory"
    assert functions[0]["verdict"] == "verified" and functions[0]["tier"] == "confirmed", "provisional function evidence cannot qualify retained full proof"
    assert package_id and entry["package_id"] == package_id, "full proof must bind the exact expected Cargo package ID"
    return result, entry, functions[0]


class Acceptance:
    def __init__(self, args):
        self.args = args
        self.sail = str(args.sail.resolve())
        self.environment = dict(os.environ, SAIL_CACHE_DIR=str(args.cache.resolve()),
                                SAIL_DAEMON_SOCKET=str(args.socket.resolve()))
        self.project = args.project.resolve()
        self.out = args.out.resolve()
        self.out.mkdir(parents=True, exist_ok=False, mode=0o700)
        self.started = time.monotonic()
        self.deadline = self.started + 120
        self.operations = []
        self.owned_job = None
        self.mcp = None
        self.mcp_errors = None
        self.request_id = 0
        self.pending = bytearray()
        self.summary = {"version": 1, "kind": "real-retained-job-cli-mcp-integration",
                        "passed": False, "platform": platform.platform(),
                        "environment_label": args.environment_label,
                        "sail_sha256": hashlib.sha256(Path(self.sail).read_bytes()).hexdigest(),
                        "job": args.job, "workflow_budget_s": 120, "command_budget_s": 30,
                        "rpc_budget_s": protocol.DEADLINE,
                        "qualification": "Requires separate genuine Current full proof; not performance acceptance."}

    def retain(self, name, document):
        (self.out / (name + ".json")).write_text(json.dumps(document, indent=2) + "\n")

    def remaining(self, maximum=30):
        remaining = min(maximum, self.deadline - time.monotonic())
        assert remaining > 0, "retained jobs integration exceeded original 120-second workflow bound"
        return remaining

    def cli(self, name, arguments, expected=0):
        return self.command(name, [self.sail, *arguments], expected)

    def command(self, name, command, expected=0):
        before = time.monotonic()
        process = subprocess.Popen(command, cwd=self.project, env=self.environment,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        try:
            stdout, stderr = process.communicate(timeout=self.remaining())
        except BaseException:
            if process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            stdout, stderr = process.communicate(timeout=5)
            self.record(name, before, process.returncode, stdout, stderr)
            raise
        self.record(name, before, process.returncode, stdout, stderr)
        assert process.returncode == expected, (name, process.returncode, stderr.decode(errors="replace"))
        return json.loads(stdout) if expected == 0 else stderr.decode(errors="replace")

    def record(self, name, before, exit_code, stdout, stderr):
        (self.out / (name + ".stdout")).write_bytes(stdout)
        (self.out / (name + ".stderr")).write_bytes(stderr)
        self.operations.append({"operation": name, "elapsed_s": time.monotonic() - before,
                                "exit_code": exit_code})
        self.retain("operations", self.operations)

    def connect(self):
        self.pending.clear()
        self.mcp_errors = (self.out / "mcp.stderr").open("ab")
        self.mcp = subprocess.Popen([self.sail, "mcp"], cwd=self.project, env=self.environment,
                                    stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                    stderr=self.mcp_errors, start_new_session=True)
        self.exchange("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                      "clientInfo": {"name": "real-retained-job-acceptance", "version": "1"}})
        self.mcp.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}).encode() + b"\n")
        self.mcp.stdin.flush()

    def disconnect(self):
        if self.mcp is not None:
            process, self.mcp = self.mcp, None
            try:
                process.stdin.close()
            except (BrokenPipeError, OSError):
                pass
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)
        if self.mcp_errors is not None:
            self.mcp_errors.close()
            self.mcp_errors = None

    def exchange(self, method, parameters):
        self.request_id += 1
        message = {"jsonrpc": "2.0", "id": self.request_id, "method": method, "params": parameters}
        before = time.monotonic()
        response_deadline = before + self.remaining()
        self.mcp.stdin.write(json.dumps(message).encode() + b"\n")
        self.mcp.stdin.flush()
        # Keep surplus bytes explicitly, so select never overlooks a response
        # already read into a language-level buffered stream.
        while b"\n" not in self.pending:
            wait = min(self.remaining(), response_deadline - time.monotonic())
            assert wait > 0 and select.select([self.mcp.stdout], [], [], wait)[0], "actual MCP response exceeded 30-second command bound"
            chunk = os.read(self.mcp.stdout.fileno(), 65536)
            assert chunk, "actual MCP closed before its response"
            self.pending.extend(chunk)
            assert len(self.pending) <= 32 * 1024 * 1024, "oversized actual MCP frame"
        raw, _, surplus = self.pending.partition(b"\n")
        self.pending = bytearray(surplus)
        response = json.loads(raw)
        self.retain(f"mcp-{self.request_id:03d}", response)
        self.operations.append({"operation": "mcp-" + method, "elapsed_s": time.monotonic() - before})
        assert response.get("id") == self.request_id and "error" not in response, response
        return response["result"]

    def tool(self, name, arguments, error=False):
        result = self.exchange("tools/call", {"name": name, "arguments": arguments})
        assert bool(result["isError"]) == error, result
        return result if error else result["structuredContent"]

    def owned_processes(self):
        """Bind the actual worker by its own stdout file and kernel start time."""
        table = protocol.process_table()
        output = self.args.cache.resolve() / "projects/jobs" / self.owned_job / "stdout"
        for worker in table.values():
            try:
                directory = Path("/proc") / str(worker["pid"])
                command = (directory / "cmdline").read_bytes().split(b"\0")
                if (len(command) < 2 or command[1] != b"_repair-worker"
                        or (directory / "fd/1").resolve() != output):
                    continue
            except (OSError, RuntimeError):
                continue
            descendants = [worker]
            known = {worker["pid"]}
            while True:
                additions = [entry for entry in table.values()
                             if entry["pid"] not in known and entry["parent"] in known]
                if not additions:
                    break
                descendants.extend(additions)
                known.update(entry["pid"] for entry in additions)
            return descendants
        return []

    def require_reaped(self, observed):
        deadline = time.monotonic() + self.remaining(5)
        while True:
            table = protocol.process_table()
            alive = [entry for entry in observed if entry["pid"] in table
                     and table[entry["pid"]]["started_ticks"] == entry["started_ticks"]
                     and table[entry["pid"]]["state"] != "Z"]
            if not alive:
                return
            assert time.monotonic() < deadline, ("owned cancelled worker/children remain live", alive)
            time.sleep(.02)

    def cancel_owned(self):
        if self.owned_job is None:
            return
        response, elapsed, _ = protocol.call(protocol.endpoint(self.args.socket),
                                            {"operation": "cancel", "id": self.owned_job, "superseded": False})
        self.retain("owned-cancel-cleanup", {"response": response, "elapsed_ms": elapsed})
        assert response["response"] == "status" and response["value"]["state"] == "cancelled", response
        self.owned_job = None

    def run(self):
        cargo = self.environment.get("SAIL_CARGO", "cargo")
        metadata = self.command("cargo-metadata", [cargo, "metadata", "--format-version", "1", "--no-deps"])
        package_id = package_identity(metadata, self.args.package, self.args.target, self.args.kind)
        # Explicit read over the original five-second framed protocol is kept
        # separately from CLI/MCP process startup and serialization overhead.
        response, elapsed, size = protocol.call(protocol.endpoint(self.args.socket),
                                               {"operation": "result", "id": self.args.job})
        self.retain("initial-rpc", {"response": response, "elapsed_ms": elapsed, "response_bytes": size})
        assert response["response"] == "result", response
        original, target, function = selected(response["value"], package_id, self.args.target,
                                             self.args.kind, self.args.function)
        self.summary.update(input_revision=original["input_revision"],
                            context_sha256=original["context_sha256"], run_id=original["run_id"],
                            target={key: target[key] for key in ("package_id", "target", "kind")})
        status = self.cli("retained-status", ["jobs", "status", self.args.job, "--output", "json"])
        assert status["value"]["state"] == "completed" and status["value"]["currentness"] == "current", status
        missing = "missing-acceptance-" + str(os.getpid())
        failure = self.cli("missing-cli-job", ["jobs", "result", missing, "--output", "json"], expected=2)
        assert "SAIL" in failure, "missing durable job must return a stable product error"
        baseline = "integration-" + self.args.job + "-" + str(os.getpid())
        saved = self.cli("baseline-save", ["baseline", "save", baseline, "--job", self.args.job, "--output", "json"])
        assert saved["response"] == "baseline" and saved["value"]["result"]["input_revision"] == original["input_revision"]
        compared = self.cli("baseline-self-compare", ["compare", self.args.job, "--baseline", baseline, "--output", "json"])
        assert compared["value"]["compatible"] and all(item["change"] == "unchanged" for item in compared["value"]["differences"])
        immutable = self.cli("baseline-overwrite-refused", ["baseline", "save", baseline, "--job", self.args.job, "--output", "json"], expected=2)
        assert "SAIL0905" in immutable, "saved baseline must be immutable"
        self.connect()
        result = self.tool("verification_result", {"job_id": self.args.job})
        assert result["value"]["result"] == original
        explained = self.tool("explain_failure", {"job_id": self.args.job, "package_id": target["package_id"],
                              "target": self.args.target, "kind": self.args.kind, "function": self.args.function})
        assert explained["value"]["function"] == function
        self.tool("verification_result", {"job_id": missing}, error=True)
        self.tool("explain_failure", {"job_id": self.args.job, "package_id": target["package_id"],
                  "target": self.args.target, "kind": self.args.kind, "function": missing}, error=True)
        first = self.tool("verification_events", {"job_id": self.args.job})
        assert first["events"] and first["gap"] is None, first
        cursor = first["next"]
        mismatched = self.tool("verification_events", {"job_id": self.args.job, "stream_id": "wrong-stream", "after": 0})
        assert mismatched["gap"] is not None, "cursor identity mismatch must retain an explicit gap"
        self.disconnect()
        self.connect()
        resumed = self.tool("verification_events", {"job_id": self.args.job, **cursor})
        assert resumed["gap"] is None and resumed["next"]["stream_id"] == cursor["stream_id"]
        assert all(event["sequence"] > cursor["after"] for event in resumed["events"])
        duplicate = self.tool("verification_events", {"job_id": self.args.job, **cursor})
        assert duplicate == resumed, "reconnecting the same completed cursor must preserve exact durable observations"
        comparison = self.tool("compare_verification", {"after_job": self.args.job, "baseline": baseline})
        assert comparison == compared
        # Force affects only the explicitly owned submission, avoiding accidental
        # coalescing with the externally supplied completed proof job.
        self.disconnect()
        self.environment["SAIL_MANAGED_FORCE"] = "1"
        self.connect()
        selection = {"project_dir": str(self.project), "package": self.args.package,
                     "kind": self.args.kind, "function": self.args.function, "engine": "interactive"}
        if self.args.kind == "bin":
            selection["target"] = self.args.target
        submitted = self.tool("start_repair_validation", selection)
        self.owned_job = submitted["id"]
        assert self.owned_job != self.args.job, "force must not cancel an existing completed proof"
        while True:
            owned = self.tool("verification_status", {"job_id": self.owned_job})["value"]
            observed = self.owned_processes() if owned["state"] == "running" else []
            if observed:
                break
            assert owned["state"] in ("queued", "running"), owned
            self.remaining()
            time.sleep(.05)
        self.retain("owned-live-processes", observed)
        cancelled = self.tool("cancel_verification", {"job_id": self.owned_job})
        assert cancelled["value"]["state"] == "cancelled", cancelled
        cancelled_result = self.tool("verification_result", {"job_id": self.owned_job})["value"]
        assert cancelled_result["job"]["state"] == "cancelled" and cancelled_result["result"] is None
        self.require_reaped(observed)
        self.summary["cancelled_owned_job"] = self.owned_job
        self.owned_job = None
        retained = self.tool("verification_result", {"job_id": self.args.job})["value"]
        assert retained["job"]["state"] == "completed" and retained["result"] == original
        self.summary["passed"] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for field in ("sail", "project", "cache", "socket", "out"):
        parser.add_argument("--" + field, type=Path, required=True)
    for field in ("job", "package", "target", "function", "environment-label"):
        parser.add_argument("--" + field, required=True)
    parser.add_argument("--kind", choices=("lib", "bin"), required=True)
    args = parser.parse_args()
    assert Path("/proc").is_dir(), "real owned-process integration requires Linux kernel process identities"
    acceptance = Acceptance(args)
    cleanup_errors = []
    try:
        acceptance.run()
    finally:
        failure_type, failure, _ = sys.exc_info()
        original_failure = failure_type is not None
        if original_failure:
            acceptance.summary["failure"] = {"type": failure_type.__name__, "message": str(failure)[:2000]}
        for cleanup in (acceptance.cancel_owned, acceptance.disconnect):
            try:
                cleanup()
            except (OSError, subprocess.SubprocessError, AssertionError, ValueError) as failure:
                cleanup_errors.append(str(failure))
        acceptance.summary.update(elapsed_s=time.monotonic() - acceptance.started,
                                  operations=acceptance.operations, cleanup_errors=cleanup_errors)
        if cleanup_errors:
            acceptance.summary["passed"] = False
        try:
            acceptance.retain("summary", acceptance.summary)
        except OSError as failure:
            cleanup_errors.append("summary retention: " + str(failure))
        if cleanup_errors:
            print("Owned acceptance cleanup/evidence failed: " + repr(cleanup_errors), file=sys.stderr)
            if not original_failure:
                raise RuntimeError("owned acceptance cleanup failed: " + repr(cleanup_errors))
    print("Actual retained repair CLI/MCP: Current full evidence, immutable baseline, cursor reconnect, exact errors and owned cancellation passed")


if __name__ == "__main__":
    main()
