#!/usr/bin/env python3
"""Installed standalone repair against the actual pinned runtime, never a stub."""
import argparse
import json
import os
from pathlib import Path
import select
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("sail", "stock", "runtime", "out"):
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args()
    out = Path(args.out).absolute()
    out.mkdir(parents=True)
    work = out / "sources"
    work.mkdir()
    source = work / "repair_case.rs"
    env = dict(os.environ, SAIL_CACHE_DIR=str(out / "cache"),
               SAIL_VERUS_BUILD=args.runtime, SAIL_BACKGROUND_PREPARE="0")
    records = []

    def call(label, command, code=0):
        result = subprocess.run(command, cwd=work, env=env, capture_output=True, timeout=180)
        (out / (label + ".stdout")).write_bytes(result.stdout)
        (out / (label + ".stderr")).write_bytes(result.stderr)
        records.append({"label": label, "command": command, "exit_code": result.returncode})
        assert result.returncode == code, (label, result.returncode, result.stderr.decode(errors="replace"))
        return result

    def repair(label, function, code=0):
        result = call(label, [args.sail, "repair-check", source.name, "--function", function,
                             "--engine", "interactive", "--output", "json"], code)
        return json.loads(result.stdout)

    passing = "use vstd::prelude::*;\nverus! { proof fn selected() { assert(true); } proof fn other() { assert(true); } }\nfn main() {}\n"
    source.write_text(passing)
    initial = json.loads(call("initial", [args.sail, "verify", source.name, "--output", "json"]).stdout)
    functions = [item["function"] for target in initial["targets"]
                 for crate in target["report"]["crates"] for item in crate["functions"]
                 if item["function"].endswith("::selected")]
    assert len(functions) == 1, initial
    function = functions[0]
    positive = repair("positive", function)
    assert positive["accepted"] and positive["batch"]["batched"], positive
    assert positive["batch"]["compiler_invocations"] == 1, positive
    full = positive["batch"]["full"]
    assert full["currentness"] == "current" and full["targets"][0]["complete"], full
    focus = positive["batch"]["focus"]
    assert not any(target["complete"] for target in focus["targets"]), focus
    assert all(item["verdict"] == "provisional" for target in focus["targets"]
               for crate in target["report"]["crates"] for item in crate["functions"]), focus

    source.write_text(passing.replace("proof fn other() { assert(true); }", "proof fn other() { assert(false); }"))
    call("stock-other-fails", [args.stock, source.name], 1)
    negative = repair("other-fails", function, 1)
    assert not negative["accepted"], negative
    assert negative["batch"]["full"]["exit_code"] == 1, negative
    assert negative["batch"]["focus"] is not None, negative
    source.write_text(passing)
    missing = repair("missing-selector", function + "_absent", 1)
    assert not missing["accepted"] and missing["batch"]["selection_error"], missing

    # A retained full-phase job is independently reread after source changes.
    submitted = json.loads(call("submit", [args.sail, "repair-check", source.name,
                                          "--function", function, "--engine", "interactive",
                                          "--background", "--output", "json"]).stdout)
    deadline = time.monotonic() + 180
    while True:
        retained = json.loads(call("job-result", [args.sail, "jobs", "result", submitted["id"], "--output", "json"]).stdout)["value"]
        if retained["job"]["state"] == "completed":
            break
        assert time.monotonic() < deadline, retained
        time.sleep(0.1)
    assert retained["repair"]["accepted"], retained
    source.write_text(passing.replace("assert(true)", "assert(false)", 1))
    stale = json.loads(call("historical", [args.sail, "jobs", "result", submitted["id"], "--output", "json"]).stdout)["value"]
    assert stale["job"]["currentness"] != "current", stale
    assert not stale["repair"]["accepted"], stale
    source.write_text(passing)
    with (out / "mcp.stderr").open("wb") as errors:
        mcp = subprocess.Popen([args.sail, "mcp"], cwd=work, env=env,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=errors, text=True)
        try:
            def rpc(identity, method, params):
                mcp.stdin.write(json.dumps({"jsonrpc": "2.0", "id": identity,
                                           "method": method, "params": params}) + "\n")
                mcp.stdin.flush()
                assert select.select([mcp.stdout], [], [], 30)[0], "MCP response timed out"
                response = json.loads(mcp.stdout.readline())
                (out / f"mcp-{identity}.json").write_text(json.dumps(response, indent=2) + "\n")
                assert response.get("id") == identity and "error" not in response, response
                return response["result"]
            rpc(1, "initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                  "clientInfo": {"name": "standalone-repair-acceptance", "version": "1"}})
            mcp.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
            mcp.stdin.flush()
            arguments = {"source_file": source.name, "function": function, "engine": "interactive"}
            started = rpc(2, "tools/call", {"name": "start_repair_validation", "arguments": arguments})
            assert not started["isError"], started
            mcp_job = started["structuredContent"]["id"]
            invalid = rpc(3, "tools/call", {"name": "start_repair_validation",
                                           "arguments": dict(arguments, package="mixed", kind="lib")})
            assert invalid["isError"], invalid
            deadline = time.monotonic() + 180
            while True:
                value = json.loads(call("mcp-job-result", [args.sail, "jobs", "result", mcp_job,
                                                          "--output", "json"]).stdout)["value"]
                if value["job"]["state"] == "completed":
                    break
                assert time.monotonic() < deadline, value
                time.sleep(0.1)
            assert value["repair"]["accepted"] and value["repair"]["batch"]["batched"], value
            assert value["repair"]["batch"]["full"]["currentness"] == "current", value
        finally:
            mcp.stdin.close()
            try:
                mcp.wait(timeout=10)
            except subprocess.TimeoutExpired:
                mcp.kill()
                mcp.wait()
    assert not (work / "Cargo.toml").exists() and not (work / "Cargo.lock").exists()
    (out / "records.json").write_text(json.dumps(records, indent=2) + "\n")
    print("PASS: actual standalone repair CLI/MCP focus/full, failure, selector, job and stale-evidence controls")


if __name__ == "__main__":
    main()
