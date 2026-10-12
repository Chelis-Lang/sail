#!/usr/bin/env python3
"""Separate named-baseline feature smoke derived from the frozen repair fixture.

No original acceptance credit: one added hook runs actual MCP/CLI calls against
the fixture's genuine completed job before its unchanged cache/actor cleanup.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import time
import types

REPO = Path(__file__).resolve().parents[1]
FIXTURE_SHA = "9880f200f99007e048727dbe8348eb93f9e53937df601eb16f2d9fdc9ceaebd7"
HELPER_SHA = "e6d44009fa4485056fc53ab18ca58c38c2d46fd2d6de0da2cf70dbc9c3521ae5"
ANCHOR = b'            assert not progress_failures, progress_failures\n            summary["passed"] = True\n'
HOOK = b'            _baseline_smoke(sail, project, env, evidence, good_job, good, deadline)\n'
FRAME_LIMIT = 16 * 1024 * 1024  # Diagnostic fixture admission, not a product limit.
NAME = "--json"


def derive(source):
    assert source.count(ANCHOR) == 1, "original fixture hook anchor changed"
    assert hashlib.sha256(source).hexdigest() == FIXTURE_SHA, "original fixture bytes changed"
    first, last = ANCHOR.splitlines(keepends=True)
    return source.replace(ANCHOR, first + HOOK + last, 1)


def module(source, filename):
    loaded = types.ModuleType("baseline_smoke_dependency")
    loaded.__file__ = str(filename)
    exec(compile(source, str(filename), "exec"), loaded.__dict__)
    return loaded


def seal(path, maximum=256 * 1024 * 1024, deadline=None):
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            size += len(block)
            assert size <= maximum, "diagnostic artifact admission exceeded"
            assert deadline is None or time.monotonic() < deadline, "artifact seal deadline expired"
            digest.update(block)
    return {"bytes": size, "sha256": digest.hexdigest()}


def baseline(document, job, good):
    assert document["response"] == "baseline", document
    value = document["value"]
    assert value["version"] == 1 and value["name"] == NAME and value["job_id"] == job, value
    result = value["result"]
    assert result["currentness"] == "historical", result
    assert result["kind"] == "sail-project-result" and result["schema_version"] == 1, result
    original = good["result"]
    assert original["currentness"] == "current" and good["repair"]["accepted"], good
    for key in ("run_id", "workspace", "context_sha256", "input_revision", "exit_code",
                "targets", "execution", "error"):
        assert result[key] == original[key], f"baseline changed {key}"
    assert result["targets"] and all(target["complete"] for target in result["targets"]), result
    return document


def full_tool(result, expected_error=False):
    assert result["isError"] is expected_error, result
    if expected_error:
        message = json.dumps(result)
        assert "SAIL0905" in message and "baseline already exists and is immutable" in message, result
        return None
    assert result["resultType"] == "complete", result
    document = result["structuredContent"]
    assert json.loads(result["content"][1]["text"]) == document, "text/structured payload mismatch"
    return document


def addon(repair, live, sail, project, env, evidence, job, good, deadline):
    output = evidence / "baseline-addon"
    output.mkdir(mode=0o700)
    record = {"version": 1, "kind": "same-cache-real-mcp-baseline-feature-smoke",
              "original_acceptance_credit": False, "status": "FAIL", "job_id": job,
              "operations": [], "clients": [], "cleanup_errors": []}
    started = time.monotonic()
    client = None

    def remaining(maximum):
        left = deadline - time.monotonic()
        assert left > 0, "original 120-second fixture deadline expired"
        return min(maximum, left)

    class BoundedMcp(live.Mcp):
        # Original helper's complete-frame read, with diagnostic admission caps.
        def read(self, stop):
            assert time.monotonic() < stop, "MCP response deadline expired"
            assert len(self.messages) < 64, "unexpected MCP message flood"
            while b"\n" not in self.buffer:
                left = stop - time.monotonic()
                assert left > 0 and select.select([self.process.stdout], [], [], left)[0], "MCP response timed out"
                data = os.read(self.process.stdout.fileno(), 65536)
                assert data, "MCP exited before a complete response"
                self.buffer.extend(data)
                assert len(self.buffer) <= FRAME_LIMIT, "diagnostic MCP frame admission exceeded"
            line, _, self.buffer = self.buffer.partition(b"\n")
            message = json.loads(line)
            self.messages.append(message)
            assert time.monotonic() < stop, "MCP decoding deadline expired"
            return message

    def request(label, method, parameters, stop=None):
        before = time.monotonic()
        stop = min(before + 5, deadline) if stop is None else stop
        assert before < stop, "MCP response deadline expired"
        operation = {"label": label, "request": {"method": method, "params": parameters}}
        record["operations"].append(operation)
        try:
            operation["result"] = client.request(method, parameters, timeout=stop - time.monotonic())
            assert time.monotonic() < stop, "MCP response deadline expired"
            return operation["result"]
        except Exception as failure:
            operation["error"] = str(failure)
            raise
        finally:
            operation["elapsed_s"] = time.monotonic() - before

    def tool(label, name, arguments, expected_error=False):
        before = time.monotonic()
        stop = min(before + 5, deadline)
        result = full_tool(request(label, "tools/call", {"name": name, "arguments": arguments}, stop), expected_error)
        assert time.monotonic() < stop, "complete tool payload decoding deadline expired"
        record["operations"][-1]["elapsed_s"] = time.monotonic() - before
        return result

    def start(label):
        nonlocal client
        directory = output / label
        directory.mkdir(mode=0o700)
        client = BoundedMcp(sail, project, env, directory)
        record["clients"].append({"label": label, "messages": client.messages})
        request(label + "-initialize", "initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                "clientInfo": {"name": "baseline-feature-smoke", "version": "1"}})
        client.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def finish():
        nonlocal client
        client.process.stdin.close()
        assert client.process.wait(timeout=remaining(5)) == 0, "MCP normal shutdown failed"
        remaining(5)
        client.stderr.close()
        client = None

    def cli(label):
        operation = {"label": label, "argv": ["baseline", "get", "--output", "json", "--", NAME]}
        record["operations"].append(operation)
        try:
            result = repair.execute([sail, *operation["argv"]], project, env, remaining(30))
        except subprocess.TimeoutExpired as failure:
            (output / (label + ".stdout.json")).write_bytes(failure.stdout or b"")
            (output / (label + ".stderr")).write_bytes(failure.stderr or b"")
            operation["timed_out"] = True
            raise
        (output / (label + ".stdout.json")).write_bytes(result.stdout)
        (output / (label + ".stderr")).write_bytes(result.stderr)
        operation["exit_code"] = result.returncode
        assert result.returncode == 0, result.stderr
        remaining(30)
        return json.loads(result.stdout)

    try:
        start("first-client")
        tools = request("definitions", "tools/list", {})["tools"]
        definitions = {item["name"]: item for item in tools}
        assert len(definitions) == len(tools), "duplicate tool definitions"
        for name in ("save_baseline", "list_baselines", "get_baseline"):
            definition = definitions[name]
            assert definition["inputSchema"]["additionalProperties"] is False, definition
            assert "project_dir" not in definition["inputSchema"]["properties"], definition
            assert definition["annotations"]["destructiveHint"] is False, definition
            assert definition["annotations"]["readOnlyHint"] is (name != "save_baseline"), definition
        saved = baseline(tool("save", "save_baseline", {"name": NAME, "job_id": job}), job, good)
        names = tool("list", "list_baselines", {})
        assert names["response"] == "baselines" and names["value"] == sorted(set(names["value"])), names
        assert NAME in names["value"] and "before-repair" in names["value"], names
        assert tool("get", "get_baseline", {"name": NAME}) == saved
        assert cli("cli-get") == saved
        path = Path(env["SAIL_CACHE_DIR"]) / "projects/jobs/baselines" / (NAME + ".json")
        record["baseline_before"] = seal(path, 32 * 1024 * 1024, deadline)
        tool("overwrite-refusal", "save_baseline", {"name": NAME, "job_id": job}, expected_error=True)
        assert tool("get-after-refusal", "get_baseline", {"name": NAME}) == saved
        assert cli("cli-get-after-refusal") == saved
        assert seal(path, 32 * 1024 * 1024, deadline) == record["baseline_before"], "immutable baseline bytes changed"
        finish()
        start("restarted-client")
        assert tool("get-after-client-restart", "get_baseline", {"name": NAME}) == saved
        record["baseline_after"] = seal(path, 32 * 1024 * 1024, deadline)
        assert record["baseline_after"] == record["baseline_before"]
        finish()
        record["status"] = "PASS"
    finally:
        primary_failure = sys.exc_info()[0] is not None
        if primary_failure:
            record["error"] = str(sys.exc_info()[1])
        if client is not None:
            try:
                repair.stop_owned(client.process, 5, close_stdin=True)
            except Exception as failure:
                record["cleanup_errors"].append(str(failure))
            finally:
                client.stderr.close()
        if record["cleanup_errors"]:
            record["status"] = "FAIL"
        record["elapsed_s"] = time.monotonic() - started
        try:
            (output / "summary.json").write_text(json.dumps(record, indent=2) + "\n")
        except OSError as failure:
            if not primary_failure:
                raise
            print(f"baseline addon evidence failure: {failure}", file=sys.stderr)
        if record["cleanup_errors"] and not primary_failure:
            raise RuntimeError("baseline addon cleanup failed")


def run(args):
    assert args.timeout == 120, "this derivative retains the original 120-second clock"
    fixture = REPO / "ci/test-repair.py"
    helper = REPO / "ci/test-mcp-live-service.py"
    source = fixture.read_bytes()
    derived = derive(source)
    helper_source = helper.read_bytes()
    assert hashlib.sha256(helper_source).hexdigest() == HELPER_SHA, "MCP helper bytes changed"
    output = Path(args.out).resolve()
    output.mkdir(parents=True, mode=0o700, exist_ok=False)
    images = {"fixture": fixture, "helper": helper, "addon": Path(__file__),
              "sail": Path(args.sail).resolve(), "shim": Path(args.sail).resolve().with_name("sail-z3")}
    record = {"version": 1, "kind": "derived-repair-baseline-feature-validation",
              "original_acceptance_credit": False, "status": "FAIL",
              "source_before": {key: seal(path) for key, path in images.items()},
              "derived_sha256": hashlib.sha256(derived).hexdigest(),
              "proof_tool_budget_s": 120, "cli_budget_s": 30, "addon_mcp_budget_s": 5}
    (output / "derived-test-repair.py").write_bytes(derived)
    repair = module(derived, output / "derived-test-repair.py")
    live = module(helper_source, helper)
    repair._baseline_smoke = lambda *values: addon(repair, live, *values)
    try:
        repair.run(args)
        record["status"] = "PASS"
    finally:
        primary_failure = sys.exc_info()[0] is not None
        if primary_failure:
            record["error"] = str(sys.exc_info()[1])
        try:
            record["source_after"] = {key: seal(path) for key, path in images.items()}
            record["immutable_inputs_unchanged"] = record["source_before"] == record["source_after"]
        except OSError as failure:
            record["postseal_error"] = str(failure)
            record["immutable_inputs_unchanged"] = False
        if not record["immutable_inputs_unchanged"] or primary_failure:
            record["status"] = "FAIL"
        try:
            (output / "derivative-record.json").write_text(json.dumps(record, indent=2) + "\n")
        except OSError as failure:
            if not primary_failure:
                raise
            print(f"derivative record failure: {failure}", file=sys.stderr)
    assert record["immutable_inputs_unchanged"], "bound source/product bytes changed"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("sail", "engine-build", "verus-source", "out"):
        parser.add_argument("--" + flag, required=True)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--environment-label", default="unspecified")
    run(parser.parse_args())
