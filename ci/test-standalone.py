#!/usr/bin/env python3
"""Installed standalone CLI/job/watch/MCP acceptance; uses actual pinned Verus."""
import argparse
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("sail", "stock", "runtime", "out"):
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args()
    out = Path(args.out).absolute()
    out.mkdir(parents=True, exist_ok=True)
    work = out / "sources"
    work.mkdir()
    environment = dict(os.environ, SAIL_CACHE_DIR=str(out / "cache"), SAIL_VERUS_BUILD=args.runtime)
    environment.pop("SAIL_MANAGED_FORCE", None)
    records = []

    def call(label, command, code=0, env=None):
        result = subprocess.run(command, cwd=work, env=env or environment,
                                capture_output=True, timeout=180)
        (out / (label + ".stdout")).write_bytes(result.stdout)
        (out / (label + ".stderr")).write_bytes(result.stderr)
        records.append({"label": label, "command": command, "exit_code": result.returncode})
        assert result.returncode == code, (label, result.returncode, result.stderr.decode(errors="replace"))
        return result

    def verify(label, code=0, extra=(), env=None, source_file="proof.rs"):
        result = call(label, [args.sail, "verify", source_file, "--output", "json", *extra], code, env)
        document = json.loads(result.stdout)
        assert document["kind"] == "sail-project-result", document
        assert len(document["targets"]) == 1, document
        assert document["targets"][0]["kind"] == ["standalone"], document
        return document

    passing = "use vstd::prelude::*;\nverus! { proof fn selected() { assert(true); } }\nfn main() {}\n"
    failing = passing.replace("assert(true)", "assert(false)")
    source = work / "proof.rs"
    source.write_text(passing)
    plain = call("plain-pass", [args.stock, "proof.rs"])
    actual = call("human-pass", [args.sail, "verify", "proof.rs"])
    assert actual.stdout == plain.stdout and actual.stderr == plain.stderr, "human output differs"
    first = verify("first-json")
    assert first["targets"][0]["complete"], first
    # An initial external-input discovery may require a second fresh run.
    settled = verify("settled")
    replay = verify("replay")
    assert replay["currentness"] == "current", replay
    assert replay["execution"]["verifier_invocations"] == 0, replay
    assert replay["targets"][0]["reused"], replay
    establishing = replay["targets"][0]["establishing_run"]
    source.write_text(failing)
    bad = verify("failed", 1)
    assert not bad["targets"][0]["complete"] and not bad["targets"][0]["reused"], bad
    source.write_text(passing)
    returned = verify("returned")
    assert returned["execution"]["verifier_invocations"] == 0, returned
    assert returned["targets"][0]["establishing_run"] == establishing, returned
    forced = verify("forced", env=dict(environment, SAIL_MANAGED_FORCE="1"))
    assert forced["execution"]["verifier_invocations"] == 1, forced

    included = "use vstd::prelude::*;\nverus! { proof fn selected() { assert(include_bytes!(\"value.txt\") == \"yes\"); } }\nfn main() {}\n"
    # A length-based include is supported without requiring string reasoning.
    included = included.replace('include_bytes!("value.txt") == "yes"', 'include_bytes!("value.txt").len() == 3')
    (work / "value.txt").write_text("yes")
    source.write_text(included)
    verify("include-first")
    verify("include-settle")
    stat = (work / "value.txt").stat()
    (work / "value.txt").write_text("changed")
    os.utime(work / "value.txt", ns=(stat.st_atime_ns, stat.st_mtime_ns))
    changed = verify("include-changed", 1)
    assert changed["execution"]["verifier_invocations"] == 1, changed
    source.write_text(passing)

    source.write_text("use vstd::prelude::*; mod helper; verus! { proof fn selected() { assert(helper::value() == 1); } } fn main() {}\n")
    helper = work / "helper.rs"
    helper.write_text("use vstd::prelude::*; verus! { pub open spec fn value() -> nat { 1 } }\n")
    verify("module-first")
    verify("module-settle")
    helper.write_text(helper.read_text().replace("{ 1 }", "{ 2 }"))
    assert verify("module-changed", 1)["execution"]["verifier_invocations"] == 1
    source.write_text("use vstd::prelude::*; verus! { proof fn selected() { let data = include_bytes!(\"binary.dat\"); assert(data.len() == 1); } } fn main() {}\n")
    (work / "binary.dat").write_bytes(b"\xff")
    binary = verify("binary-include")
    assert binary["currentness"] == "current" and binary["targets"][0]["complete"], binary
    assert not binary["targets"][0]["reused"], binary
    binary_repeat = verify("binary-repeat")
    assert binary_repeat["currentness"] == "current" and binary_repeat["targets"][0]["reused"], binary_repeat
    assert binary_repeat["execution"]["verifier_invocations"] == 0, binary_repeat
    assert binary_repeat["targets"][0]["establishing_run"] == binary["targets"][0]["establishing_run"]
    binary_stat = (work / "binary.dat").stat()
    (work / "binary.dat").write_bytes(b"\xfe")
    os.utime(work / "binary.dat", ns=(binary_stat.st_atime_ns, binary_stat.st_mtime_ns))
    binary_changed = verify("binary-restored-mtime")
    assert binary_changed["currentness"] == "current" and not binary_changed["targets"][0]["reused"], binary_changed
    assert binary_changed["execution"]["verifier_invocations"] == 1, binary_changed
    (work / "binary.dat").write_bytes(b"")
    binary_plain = call("binary-empty-stock", [args.stock, "proof.rs"], 1)
    binary_empty = verify("binary-empty", 1)
    assert not binary_empty["targets"][0]["complete"] and not binary_empty["targets"][0]["reused"], binary_empty
    assert binary_empty["execution"]["verifier_invocations"] == 1, binary_empty
    assert (out / "binary-empty.stderr").read_bytes() == binary_plain.stderr
    source.write_text(passing)
    partial = verify("partial", extra=("--", "--verify-root", "--verify-function", "selected"))
    assert not partial["targets"][0]["complete"], partial

    submitted = json.loads(call("submit", [args.sail, "verify", "proof.rs", "--background", "--output", "json"]).stdout)
    job_id = submitted.get("id", submitted.get("job", {}).get("id"))
    assert job_id, submitted
    for n in range(180):
        result = call("job-status", [args.sail, "jobs", "status", job_id, "--output", "json"])
        status = json.loads(result.stdout)
        status = status.get("value", status)
        if status["state"] in ("completed", "infrastructure_error", "cancelled", "superseded"):
            break
        time.sleep(0.5)
    assert status["state"] == "completed", status
    result = json.loads(call("job-result", [args.sail, "jobs", "result", job_id, "--output", "json"]).stdout)
    result = result.get("value", result)
    assert result["result"]["targets"][0]["complete"], result
    assert result["result"]["currentness"] == "current", result
    # #684: a service job's checks say how each answer was established.
    checks = [check
              for target in result["result"]["targets"]
              for krate in target["report"]["crates"]
              for function in krate["functions"]
              for obligation in (function.get("obligations") or [])
              for check in obligation["checks"]]
    assert checks and all(check["source"] in ("solved", "cache") for check in checks), checks
    source.write_text(failing)
    historical = json.loads(call("job-after-edit", [args.sail, "jobs", "result", job_id, "--output", "json"]).stdout)
    historical = historical.get("value", historical)
    assert historical["job"]["currentness"] == "historical", historical
    source.write_text(passing)

    (work / "0").write_text(passing)
    (work / "00").write_text(failing)
    source.write_text('include!(env!("SAIL_MANAGED_FORCE"));\n')
    call("plain-tracked-control-env", [args.stock, "proof.rs"], env=dict(environment, SAIL_MANAGED_FORCE="0"))
    verify("tracked-control-env", env=dict(environment, SAIL_MANAGED_FORCE="0"))
    verify("tracked-control-env-settle", env=dict(environment, SAIL_MANAGED_FORCE="0"))
    changed_env = verify("tracked-control-env-changed", 1, env=dict(environment, SAIL_MANAGED_FORCE="00"))
    assert changed_env["execution"]["verifier_invocations"] == 1, changed_env

    alias_a, alias_b = out / "alias-a", out / "alias-b"
    alias_a.mkdir()
    alias_b.mkdir()
    (alias_a / "proof.rs").write_text(passing)
    (alias_b / "proof.rs").write_text(failing)
    alias = out / "selected"
    alias.symlink_to(alias_a, target_is_directory=True)
    verify("external-alias-first", source_file="../selected/proof.rs")
    verify("external-alias-settle", source_file="../selected/proof.rs")
    alias.unlink()
    alias.symlink_to(alias_b, target_is_directory=True)
    retargeted = verify("external-alias-retargeted", 1, source_file="../selected/proof.rs")
    assert retargeted["execution"]["verifier_invocations"] == 1, retargeted

    source.write_text(passing)
    outside_watch = call("watch-external-source", [args.sail, "watch", "../selected/proof.rs", "--output", "jsonl"], 2)
    assert b"working directory" in outside_watch.stderr

    # Inline modules can resolve outside cwd without a source file in each
    # intervening directory. A new alternative there must invalidate reuse.
    external = out / "external-inline"
    leaf = external / "inner" / "leaf"
    leaf.mkdir(parents=True)
    module_text = "use vstd::prelude::*; verus! { pub proof fn chosen() { assert(true); } }\n"
    (leaf / "mod.rs").write_text(module_text)
    source.write_text('#[path = "../external-inline"]\nmod outside { pub mod inner { pub mod leaf; } }\nfn main() {}\n')
    # Keep harness evidence names stable while proving actual receipt reuse.
    phases = ("inline", "directory-alternative", "directory-link", "broken-source-link")
    for stream in ("stdout", "stderr"):
        (out / ("watch-external-inputs." + stream)).touch()
    for phase in phases:
        for suffix in ("stock-before", "first", "settle", "replay", "stock-after", "ambiguous"):
            for stream in ("stdout", "stderr"):
                (out / (phase + "-" + suffix + "." + stream)).touch()

    def module_receipt(phase):
        call(phase + "-stock-before", [args.stock, "proof.rs"])
        verify(phase + "-first")
        verify(phase + "-settle")
        assert verify(phase + "-replay")["execution"]["verifier_invocations"] == 0

    def module_ambiguity(phase):
        plain = call(phase + "-stock-after", [args.stock, "proof.rs"], 1)
        changed = verify(phase + "-ambiguous", 1)
        assert changed["execution"]["verifier_invocations"] == 1 and not changed["targets"][0]["reused"], changed
        assert (out / (phase + "-ambiguous.stderr")).read_bytes() == plain.stderr

    alternative = external / "inner" / "leaf.rs"
    module_receipt("inline")
    outside_watch = call("watch-external-inputs", [args.sail, "watch", "proof.rs", "--output", "jsonl"], 2)
    assert b"working directory" in outside_watch.stderr
    alternative.write_text(module_text)
    module_ambiguity("inline")
    (leaf / "mod.rs").unlink()
    module_receipt("directory-alternative")
    (leaf / "mod.rs").write_text(module_text)
    module_ambiguity("directory-alternative")
    (leaf / "mod.rs").unlink()
    leaf.rmdir()
    linked_directory = out / "unread-module-directory"
    linked_directory.mkdir()
    leaf.symlink_to(linked_directory, target_is_directory=True)
    module_receipt("directory-link")
    (leaf / "mod.rs").write_text(module_text)
    module_ambiguity("directory-link")
    alternative.unlink()
    missing_source = out / "unread-link-target" / "missing-module.rs"
    missing_source.parent.mkdir()
    alternative.symlink_to(missing_source)
    module_receipt("broken-source-link")
    missing_source.write_text(module_text)
    module_ambiguity("broken-source-link")
    source.write_text(passing)

    ambient_events = out / "ambient-engine-events.jsonl"
    ambient = verify("ambient-engine-disabled", env=dict(environment, SAIL_ENGINE="interactive", SAIL_ENGINE_FUNCTION="missing", SAIL_ENGINE_EVENTS=str(ambient_events), SAIL_SOLVER_MODE="accept-first-proof"))
    assert ambient.get("engine") is None and ambient["targets"][0]["complete"] and not ambient_events.exists(), ambient

    explicit_logs = str(out / "explicit-logs")
    call("plain-log-directory", [args.stock, "proof.rs", "--log-dir", explicit_logs])
    assert verify("explicit-log-directory", extra=("--", "--log-dir", explicit_logs))["execution"]["verifier_invocations"] == 1

    unsupported_solver = call("unsupported-solver", [args.sail, "verify", "proof.rs", "--", "-V", "cvc5"], 2)
    assert b"pinned Z3" in unsupported_solver.stderr

    # Reach an actual blocked solver check before cancellation.
    source.write_text("use vstd::prelude::*; verus! { proof fn selected(x: int) ensures x + 1 > x {} } fn main() {}\n")
    marker = out / "solver-blocked"
    proxy = out / "blocking-z3"
    proxy.write_text("#!" + sys.executable + "\n" + r'''import os, sys, subprocess, threading, time
if any(arg in ("--version", "-version") for arg in sys.argv[1:]):
    os.execv(os.environ["SAIL_TEST_REAL_Z3"], [os.environ["SAIL_TEST_REAL_Z3"], *sys.argv[1:]])
child = subprocess.Popen([os.environ["SAIL_TEST_REAL_Z3"], *sys.argv[1:]], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
def forward():
    while True:
        data = os.read(child.stdout.fileno(), 65536)
        if not data: return
        os.write(sys.stdout.fileno(), data)
threading.Thread(target=forward, daemon=True).start()
for line in sys.stdin.buffer:
    if b"(check-sat" in line:
        open(os.environ["SAIL_TEST_SOLVER_BLOCKED"], "wb").close()
        while True: time.sleep(1)
    child.stdin.write(line)
    child.stdin.flush()
child.stdin.close()
sys.exit(child.wait())
''')
    proxy.chmod(0o755)
    cancel_env = dict(environment, VERUS_Z3_PATH=str(proxy), SAIL_TEST_REAL_Z3=str(Path(args.stock).with_name("z3")), SAIL_TEST_SOLVER_BLOCKED=str(marker))
    submitted = json.loads(call("cancel-submit", [args.sail, "verify", "proof.rs", "--background", "--output", "json"], env=cancel_env).stdout)
    cancel_id = submitted["id"]
    deadline = time.monotonic() + 45
    while not marker.exists() and time.monotonic() < deadline:
        time.sleep(0.05)
    assert marker.exists(), "cancel test never reached actual solver check"
    cancelled = json.loads(call("cancel", [args.sail, "jobs", "cancel", cancel_id, "--output", "json"]).stdout)
    cancelled = cancelled.get("value", cancelled)
    assert cancelled["state"] == "cancelled", cancelled
    cancelled_result = json.loads(call("cancel-result", [args.sail, "jobs", "result", cancel_id, "--output", "json"]).stdout)
    cancelled_result = cancelled_result.get("value", cancelled_result)
    assert cancelled_result["result"] is None, cancelled_result
    assert verify("cancel-restart", env=dict(environment, SAIL_MANAGED_FORCE="1"))["targets"][0]["complete"]

    refusal = call("watch-output-refusal", [args.sail, "watch", "proof.rs", "--", "--compile"], 2)
    assert b"verify-only" in refusal.stderr, refusal.stderr
    watch_log = (out / "watch.jsonl").open("wb")
    watch_err = (out / "watch.stderr").open("wb")
    watch = subprocess.Popen([args.sail, "watch", "proof.rs", "--output", "jsonl"], cwd=work,
                             env=environment, stdout=watch_log, stderr=watch_err)
    try:
        def completed_count():
            lines = (out / "watch.jsonl").read_text().splitlines()
            return len({entry["job"]["id"] for entry in map(json.loads, lines) if entry.get("job", {}).get("state") == "completed"})
        deadline = time.monotonic() + 180
        while completed_count() == 0 and time.monotonic() < deadline:
            assert watch.poll() is None, "watch exited"
            time.sleep(0.2)
        count = completed_count()
        assert count, "watch never completed initial verification"
        source.write_text(failing)
        deadline = time.monotonic() + 180
        while completed_count() <= count and time.monotonic() < deadline:
            assert watch.poll() is None, "watch exited after save"
            time.sleep(0.2)
        assert completed_count() > count, "watch missed an edit"
        count = completed_count()
        source.unlink()
        time.sleep(0.5)
        temporary = work / "proof.rs.new"
        temporary.write_text(passing)
        temporary.replace(source)
        deadline = time.monotonic() + 180
        while completed_count() <= count and time.monotonic() < deadline:
            assert watch.poll() is None, "watch exited after delete/recreate"
            time.sleep(0.2)
        assert completed_count() > count, "watch missed atomic recreation"

    finally:
        watch.send_signal(2)
        try:
            watch.wait(timeout=15)
        except subprocess.TimeoutExpired:
            watch.kill()
            watch.wait()
        watch_log.close()
        watch_err.close()
    source.write_text(passing)

    mcp = subprocess.Popen([args.sail, "mcp"], cwd=work, env=environment,
                           stdin=subprocess.PIPE, stdout=subprocess.PIPE, bufsize=0,
                           stderr=(out / "mcp.stderr").open("wb"))
    try:
        def rpc(identity, method, params):
            mcp.stdin.write((json.dumps({"jsonrpc":"2.0", "id":identity, "method":method, "params":params}) + "\n").encode())
            mcp.stdin.flush()
            deadline = time.monotonic() + 180
            while time.monotonic() < deadline:
                ready, _, _ = select.select([mcp.stdout], [], [], 1)
                if not ready:
                    continue
                raw = mcp.stdout.readline()
                assert raw, "MCP exited"
                message = json.loads(raw)
                if message.get("id") == identity:
                    return message
            raise AssertionError("MCP response timed out")
        initialized = rpc(1, "initialize", {"protocolVersion":"2025-06-18", "capabilities":{}, "clientInfo":{"name":"standalone-acceptance", "version":"1"}})
        assert "result" in initialized, initialized
        mcp.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
        mcp.stdin.flush()
        response = rpc(2, "tools/call", {"name":"verify", "arguments":{"source_file":"proof.rs"}})
        (out / "mcp-verify.json").write_text(json.dumps(response, indent=2) + "\n")
        assert not response.get("error") and not response["result"].get("isError"), response
        summary = response["result"]["structuredContent"]
        assert summary["currentness"] == "current" and summary["counts"]["verified"] > 0 and summary["counts"]["failed"] == 0, summary
        full = rpc(3, "tools/call", {"name":"verify", "arguments":{"source_file":"proof.rs", "detail":"full"}})
        (out / "mcp-verify-full.json").write_text(json.dumps(full, indent=2) + "\n")
        assert not full.get("error") and not full["result"].get("isError"), full
        document = full["result"]["structuredContent"]["project_result"]
        assert document["targets"][0]["complete"] and document["currentness"] == "current", document

    finally:
        mcp.terminate()
        try:
            mcp.wait(timeout=10)
        except subprocess.TimeoutExpired:
            mcp.kill()
            mcp.wait()
    assert not (work / "Cargo.toml").exists() and not (work / "Cargo.lock").exists(), "generated Cargo project"
    (out / "summary.json").write_text(json.dumps({"calls":records, "status":"passed"}, indent=2) + "\n")
    print("standalone CLI, reuse, failure/fix/undo, include, jobs, watch and MCP passed")


if __name__ == "__main__":
    main()
