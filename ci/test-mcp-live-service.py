#!/usr/bin/env python3
"""Installed MCP/live Linux job-service transport smoke, without proof claims.

The service and worker are real Sail processes. A running worker is stopped and
cancelled before proving anything; no compiler, CLI, RPC, or reply is substituted.
Withdrawing the owned executable pathname makes CLI fallback impossible during
the persistent-read checks. This supplements the existing verifier acceptance.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time


def digest(path):
    result = hashlib.sha256()
    with Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def stop(process):
    if process is None or process.poll() is not None:
        return
    # Owned processes can be stopped deliberately by this smoke. Resume before
    # graceful shutdown; kill the complete owned group if the grace expires.
    os.killpg(process.pid, signal.SIGCONT)
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)


def exact(stream, length):
    result = bytearray()
    while len(result) < length:
        data = stream.recv(length - len(result))
        assert data, "live service returned an incomplete frame"
        result.extend(data)
    return bytes(result)


def rpc(endpoint, request, timeout=5):
    body = json.dumps({"version": 1, "body": request}).encode()
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(timeout)
        stream.connect(str(endpoint))
        stream.sendall(struct.pack(">I", len(body)) + body)
        length = struct.unpack(">I", exact(stream, 4))[0]
        assert length <= 32 * 1024 * 1024, "oversized service frame"
        envelope = json.loads(exact(stream, length))
        assert envelope["version"] == 1, envelope
        return envelope["body"]


def start_time(pid):
    try:
        return Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()[19]
    except FileNotFoundError:
        return None


def children(pid):
    result = set()
    for task in Path(f"/proc/{pid}/task").glob("*/children"):
        try:
            result.update(int(value) for value in task.read_text().split())
        except FileNotFoundError:
            pass
    return result


def socket_count(pid):
    result = 0
    for descriptor in Path(f"/proc/{pid}/fd").iterdir():
        try:
            result += os.readlink(descriptor).startswith("socket:[")
        except FileNotFoundError:
            pass
    return result


class Mcp:
    def __init__(self, executable, cwd, env, evidence):
        self.stderr = (evidence / "mcp.stderr").open("wb")
        self.process = subprocess.Popen(
            [str(executable), "mcp"], cwd=cwd, env=env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr,
            start_new_session=True)
        self.next_id = 1
        self.buffer = bytearray()
        self.messages = []

    def send(self, message):
        self.process.stdin.write(json.dumps(message).encode() + b"\n")
        self.process.stdin.flush()

    def read(self, deadline):
        assert time.monotonic() < deadline, "installed MCP response deadline expired"
        while b"\n" not in self.buffer:
            remaining = deadline - time.monotonic()
            assert remaining > 0, "installed MCP response deadline expired"
            assert select.select([self.process.stdout], [], [], remaining)[0], "MCP response timed out"
            data = os.read(self.process.stdout.fileno(), 65536)
            assert data, "installed MCP exited before a complete response"
            self.buffer.extend(data)
        line, _, self.buffer = self.buffer.partition(b"\n")
        message = json.loads(line)
        self.messages.append(message)
        assert time.monotonic() < deadline, "installed MCP response deadline expired"
        return message

    def request(self, method, params, timeout=30):
        identity = self.next_id
        self.next_id += 1
        deadline = time.monotonic() + timeout
        self.send({"jsonrpc": "2.0", "id": identity, "method": method, "params": params})
        while True:
            message = self.read(deadline)
            if message.get("id") == identity:
                assert "error" not in message, message
                return message["result"]

    def tool(self, name, job_id):
        result = self.request("tools/call", {"name": name, "arguments": {"job_id": job_id}})
        assert result["resultType"] == "complete" and not result.get("isError"), result
        document = result["structuredContent"]
        assert json.loads(result["content"][1]["text"]) == document, result
        return document

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                stop(self.process)
        self.stderr.close()


def run(arguments):
    assert sys.platform.startswith("linux"), "this real process/socket smoke requires Linux"
    supplied = Path(arguments.sail).resolve(strict=True)
    shim = supplied.with_name("sail-z3").resolve(strict=True)
    evidence = Path(arguments.out).resolve()
    evidence.mkdir(parents=True, exist_ok=True, mode=0o700)
    summary = {"schema_version": 1, "scope": "installed-mcp-live-job-service-transport",
               "sail_sha256": digest(supplied), "shim_sha256": digest(shim),
               "script_sha256": digest(__file__),
               "execution": arguments.execution,
               "proof_qualification": False, "latency_qualification": False,
               "checks": {}, "passed": False}
    def retain():
        (evidence / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    retain()
    with tempfile.TemporaryDirectory(prefix="sail-live-mcp-") as temporary:
        root = Path(temporary).resolve()
        executable = root / "sail"
        shutil.copy2(supplied, executable)
        shutil.copy2(shim, root / "sail-z3")
        project = root / "project"
        (project / "src").mkdir(parents=True)
        (project / "Cargo.toml").write_text(
            '[package]\nname="mcp_service_smoke"\nversion="0.1.0"\nedition="2021"\n'
            '[package.metadata.verus]\nverify=true\n[workspace]\n')
        (project / "src/lib.rs").write_text("pub fn unproved() {}\n")
        cache = root / "cache"
        cache.mkdir()
        cache_socket = root / "daemon.sock"
        suffix = hashlib.sha256(os.fsencode(str(cache_socket))).hexdigest()[:8]
        endpoint = root / f"jobs-v1-proof-v1-{suffix}.sock"
        env = dict(os.environ, SAIL_CACHE_DIR=str(cache), SAIL_DAEMON_SOCKET=str(cache_socket))
        daemon = None
        mcp = None
        worker = None
        worker_start = None
        daemon_stderr = (evidence / "daemon.stderr").open("wb")
        def start_daemon(binary):
            process = subprocess.Popen(
                [str(binary), "_serve-jobs", "--socket", str(cache_socket),
                 "--cache", str(cache), "--idle-timeout", "0"],
                cwd=project, env=env, stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL, stderr=daemon_stderr, start_new_session=True)
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline and process.poll() is None:
                try:
                    assert rpc(endpoint, {"operation": "list", "project_dir": None},
                               timeout=max(.001, deadline-time.monotonic()))["response"] == "list"
                    return process
                except (OSError, AssertionError):
                    time.sleep(.02)
            stop(process)
            raise AssertionError("owned real job service did not start within five seconds")
        try:
            daemon = start_daemon(executable)
            submission = {"mode": {"kind": "stock"}, "project_dir": str(project), "args": [],
                          "force": False, "environment": env, "input_revision": None}
            submitted = rpc(endpoint, {"operation": "submit", "submission": submission})
            assert submitted["response"] == "status", submitted
            job_id = submitted["value"]["id"]
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                for pid in children(daemon.pid):
                    try:
                        argv = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
                        if argv[:2] == [os.fsencode(executable), b"verify"]:
                            identity = start_time(pid)
                            if identity is None:
                                continue
                            assert os.getpgid(pid) == pid, "real worker does not own its process group"
                            os.killpg(pid, signal.SIGSTOP)
                            assert start_time(pid) == identity, "worker identity changed during stop"
                            worker = pid
                            worker_start = identity
                            break
                    except (ProcessLookupError, FileNotFoundError):
                        pass
                if worker is not None:
                    break
                time.sleep(.001)
            assert worker is not None, "real worker exited before the bounded stop control"
            mcp = Mcp(executable, project, env, evidence)
            mcp.request("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                      "clientInfo": {"name": "live-service-smoke", "version": "1"}})
            mcp.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
            executable.rename(root / "sail-withdrawn")
            status = mcp.tool("verification_status", job_id)
            initial = mcp.tool("verification_result", job_id)
            assert status["response"] == "status" and status["value"]["id"] == job_id, status
            assert status["value"]["state"] == "running", status
            assert initial["response"] == "result" and initial["value"]["job"]["id"] == job_id, initial
            assert initial["value"]["result"] is None, initial
            assert not children(mcp.process.pid), "readonly MCP unexpectedly spawned a CLI child"
            summary["checks"]["direct_read_without_executable_or_children"] = True
            cancelled = rpc(endpoint, {"operation": "cancel", "id": job_id, "superseded": False})
            assert cancelled["value"]["state"] == "cancelled", cancelled
            after = mcp.tool("verification_result", job_id)
            assert after["value"]["job"]["id"] == job_id, after
            assert after["value"]["job"]["state"] == "cancelled", after
            assert initial != after and after["value"]["result"] is None, after
            assert mcp.tool("verification_status", job_id)["value"]["state"] == "cancelled"
            summary["checks"]["fresh_same_id_frame_changes_running_to_cancelled"] = True
            stop(daemon)
            unavailable = mcp.request("tools/call", {"name": "verification_result",
                                                      "arguments": {"job_id": job_id}})
            assert unavailable["resultType"] == "complete" and unavailable["isError"], unavailable
            assert json.loads(unavailable["content"][1]["text"]) == unavailable["structuredContent"], unavailable
            # The unchanged CLI fallback can report a spawn error or Linux's
            # missing-program exit127. Neither contains a retained response,
            # result, job, model, currentness, or other proof evidence.
            assert unavailable["structuredContent"] in (
                {"error": "service_unavailable"}, {"exit_code": 127}), unavailable
            summary["checks"]["missing_service_does_not_serve_retained_frame"] = True
            daemon = start_daemon(supplied)
            restarted = mcp.tool("verification_result", job_id)
            assert restarted == after, (restarted, after)
            summary["checks"]["service_restart_preserves_identity_and_live_reads"] = True
            baseline_sockets = socket_count(mcp.process.pid)
            os.kill(daemon.pid, signal.SIGSTOP)
            identity = mcp.next_id
            mcp.next_id += 1
            mcp.send({"jsonrpc": "2.0", "id": identity, "method": "tools/call",
                      "params": {"name": "verification_result", "arguments": {"job_id": job_id}}})
            deadline = time.monotonic() + 2
            while socket_count(mcp.process.pid) <= baseline_sockets and time.monotonic() < deadline:
                time.sleep(.005)
            assert socket_count(mcp.process.pid) > baseline_sockets, "real service read never became pending"
            started = time.monotonic()
            mcp.send({"jsonrpc": "2.0", "method": "notifications/cancelled",
                      "params": {"requestId": identity, "reason": "owned transport control"}})
            mcp.request("ping", {}, timeout=1)
            deadline = started + 1
            while socket_count(mcp.process.pid) > baseline_sockets and time.monotonic() < deadline:
                time.sleep(.005)
            assert socket_count(mcp.process.pid) == baseline_sockets, "cancelled read retained its owned socket"
            summary["cancellation_cleanup_s"] = time.monotonic() - started
            assert summary["cancellation_cleanup_s"] < 1, "cancelled read exceeded the original responsiveness bound"
            os.kill(daemon.pid, signal.SIGCONT)
            assert mcp.tool("verification_result", job_id) == after
            assert not children(mcp.process.pid), "read cancellation unexpectedly spawned a CLI child"
            # Joining the actual server before draining EOF also checks replies
            # queued after the next successful read, not just the first ping.
            mcp.close()
            tail = bytes(mcp.buffer) + mcp.process.stdout.read(1024 * 1024 + 1)
            assert len(tail) <= 1024 * 1024 and (not tail or tail.endswith(b"\n")), "incomplete/oversized final MCP stream"
            mcp.messages.extend(json.loads(line) for line in tail.splitlines())
            mcp.buffer.clear()
            assert all(message.get("id") != identity for message in mcp.messages), "late cancelled MCP reply leaked"
            summary["checks"]["actual_notification_cancels_read_and_suppresses_late_response"] = True
            (evidence / "initial-running.json").write_text(json.dumps(initial, indent=2) + "\n")
            (evidence / "fresh-cancelled.json").write_text(json.dumps(after, indent=2) + "\n")
            (evidence / "mcp-messages.json").write_text(json.dumps(mcp.messages, indent=2) + "\n")
            summary["passed"] = True
            retain()
        except BaseException as failure:
            summary["failure"] = f"{type(failure).__name__}: {failure}"
            if mcp is not None:
                (evidence / "mcp-messages.json").write_text(json.dumps(mcp.messages, indent=2) + "\n")
            retain()
            raise
        finally:
            if mcp is not None:
                mcp.close()
            stop(daemon)
            if worker is not None and start_time(worker) == worker_start:
                try:
                    os.killpg(worker, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            daemon_stderr.close()
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sail")
    parser.add_argument("--out", required=True)
    parser.add_argument("--execution", required=True)
    run(parser.parse_args())
