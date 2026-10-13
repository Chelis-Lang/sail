#!/usr/bin/env python3
"""Real optional-engine CLI/MCP acceptance; prebuilt engine, no source build."""

import argparse
import json
import os
from pathlib import Path
import platform
import select
import signal
import subprocess
import sys
import tempfile
import time

GOOD = "proof fn selected(x: bool) ensures x || !x { }\nproof fn other(x: bool) ensures x == x { }"
REGRESSION = "proof fn selected(x: bool) ensures x == x { }\nproof fn other(x: bool) ensures x { }"
LATE = "proof fn selected() { }\n#[verifier::external_body] fn late() { let mut x = 0u64; let a = &mut x; let b = &mut x; *a = *b; }"
# The engine, like the stock release, lacks Verus's `singular` feature
# (corpus/repros/engine-without-singular): this target falls back to stock (SAIL3124).
RING = "proof fn selected(x: int, y: int) by (integer_ring) ensures (x + y) * (x + y) == x * x + 2 * x * y + y * y { }"


def execute(command, cwd, environment, timeout):
    process = subprocess.Popen(command, cwd=cwd, env=environment,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired as failure:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        failure.stdout, failure.stderr = process.communicate(timeout=10)
        raise
    return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def stop_owned(process, timeout, close_stdin=False):
    """Reap an owned session, including children when graceful shutdown stalls."""
    if close_stdin:
        try:
            process.stdin.close()
        except (BrokenPipeError, OSError):
            pass
    elif process.poll() is None:
        try:
            process.terminate()
        except ProcessLookupError:
            pass
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=timeout)


def run(args):
    assert args.timeout > 0, "acceptance timeout must be positive"
    assert os.environ.get("CI", "").lower() != "true" or args.timeout <= 120, "CI proof/tool budget cannot exceed 120 seconds"
    sail = str(Path(args.sail).resolve())
    engine = Path(args.engine_build).resolve()
    source_tree = Path(args.verus_source).resolve()
    evidence = Path(args.out).resolve()
    evidence.mkdir(parents=True, exist_ok=True, mode=0o700)
    # A complete pinned source checkout supplies Cargo-inherited dependencies
    # and lint definitions. The engine itself is already built and manifest-checked.
    head = subprocess.check_output(["git", "-C", str(source_tree), "rev-parse", "HEAD"], text=True).strip()
    builtin = source_tree / "source/builtin"
    macros = source_tree / "source/builtin_macros"
    assert (builtin / "Cargo.toml").is_file() and (macros / "Cargo.toml").is_file()
    with tempfile.TemporaryDirectory(prefix="sr-", dir="/tmp") as temporary:
        root = Path(temporary)
        project = root / "project"
        (project / "src").mkdir(parents=True)
        (project / "Cargo.toml").write_text(
            '[package]\nname="repair_demo"\nversion="0.1.0"\nedition="2021"\n'
            '[package.metadata.verus]\nverify=true\nno-vstd=true\n[dependencies]\n'
            f'verus_builtin={{path={json.dumps(str(builtin))}}}\n'
            f'verus_builtin_macros={{path={json.dumps(str(macros))}}}\n')
        source = project / "src/lib.rs"
        def write(body):
            source.write_text("use verus_builtin::*;\nuse verus_builtin_macros::*;\nverus! {\n" + body + "\n}\n")
        write(GOOD)
        env = dict(os.environ, SAIL_CACHE_DIR=str(root / "cache"),
                   SAIL_DAEMON_SOCKET=str(root / "d.sock"))
        env.pop("VSTD_KIND", None)
        setup = execute([sail, "engine", "install", "--from", str(engine), "--output", "json"], project, env, 30)
        (evidence / "engine-install.stdout.json").write_bytes(setup.stdout)
        (evidence / "engine-install.stderr").write_bytes(setup.stderr)
        setup.check_returncode()
        engine_status = json.loads(setup.stdout)
        env["RUSTUP_TOOLCHAIN"] = engine_status["rust_toolchain"]
        assert head == engine_status["commit"], "fixture source does not match validated primary engine pin"
        # Resolve ordinary Cargo packages once before the 120-second proof/tool
        # budget. This is dependency setup, not a source-engine compilation.
        setup = execute(["cargo", "+" + engine_status["rust_toolchain"], "fetch"], project, env, 60)
        (evidence / "cargo-fetch.stderr").write_bytes(setup.stderr)
        setup.check_returncode()
        daemon_errors = (evidence / "daemon.stderr").open("wb")
        daemon = subprocess.Popen([sail, "serve", "--cache", env["SAIL_CACHE_DIR"],
                                   "--socket", env["SAIL_DAEMON_SOCKET"], "--idle-timeout", "0"],
                                  env=env, stdout=subprocess.DEVNULL, stderr=daemon_errors,
                                  start_new_session=True)
        started = time.monotonic()
        deadline = started + args.timeout
        summary = {"version":1,"kind":"actual-prebuilt-engine-cli-mcp-acceptance",
                   "machine":platform.platform(),"environment_label":args.environment_label,
                   "source_commit":head,"proof_tool_budget_s":args.timeout,"passed":False,
                   "fixture_vstd":"supported-no-vstd-metadata", "fixture_dependency_verification":"ordinary-full-cargo-plan","fixture_rust_toolchain":engine_status["rust_toolchain"]}
        (evidence / "summary.json").write_text(json.dumps(summary,indent=2)+"\n")
        operations = []
        progress_failures = []
        mcp = None
        mcp_errors = None
        def remaining(maximum=30):
            value = min(maximum, deadline - time.monotonic())
            assert value > 0, f"real repair CLI/MCP acceptance exceeded {args.timeout:g} seconds"
            return value
        def cli(label, arguments, expected=0):
            before = time.monotonic()
            try:
                result = execute([sail, *arguments], project, env, remaining())
            except subprocess.TimeoutExpired as failure:
                (evidence / (label + ".stdout.json")).write_bytes(failure.stdout or b"")
                (evidence / (label + ".stderr")).write_bytes(failure.stderr or b"")
                operations.append({"operation":label,"elapsed_s":time.monotonic()-before,
                                   "exit_code":None,"timed_out":True})
                (evidence / "operations.json").write_text(json.dumps(operations,indent=2)+"\n")
                raise
            (evidence / (label + ".stdout.json")).write_bytes(result.stdout)
            (evidence / (label + ".stderr")).write_bytes(result.stderr)
            operations.append({"operation": label, "elapsed_s": time.monotonic() - before,
                               "exit_code": result.returncode})
            (evidence / "operations.json").write_text(json.dumps(operations,indent=2)+"\n")
            assert result.returncode == expected, (label, result.returncode, result.stderr.decode(errors="replace"))
            return json.loads(result.stdout)
        def wait_job(job, label):
            attempt = 0
            while True:
                value = cli(f"{label}-result-{attempt}", ["jobs", "result", job, "--output", "json"])["value"]
                if value["job"]["state"] == "completed":
                    assert value["repair"] is not None and value["result"] is not None, value
                    return value
                assert value["job"]["state"] in ("queued", "running"), value
                remaining()
                attempt += 1
                time.sleep(.25)
        def submit(label, function="repair_demo::selected"):
            status = cli(label + "-submit", ["repair-check", "--engine", "interactive",
                         "--package", "repair_demo", "--lib", "--function", function,
                         "--background", "--output", "json"])
            return status["id"], wait_job(status["id"], label)
        def provenance(result, target):
            # Engine results name the engine and its manifest digest in the
            # project, report and every function, and stay default mode.
            identity = {"name": "interactive", "manifest_sha256": engine_status["manifest_sha256"]}
            assert result["engine"] == identity, result
            engine = dict(identity, established_by="engine")
            assert target["report"]["engine"] == engine and target["report"]["mode"] == "default", target
            functions = [f for c in target["report"]["crates"] for f in c["functions"]]
            assert functions and all(f["provenance"]["engine"] == engine and f["provenance"]["mode"] == "default"
                                     for f in functions), target
        def full(value):
            # A job result carries the full phase once, as `result`.
            document = value["repair"]
            assert "full" not in document["batch"], "job results must not repeat the full phase"
            result = value["result"]
            targets = [target for target in result["targets"] if target["requested"]]
            assert len(targets) == 1 and targets[0]["target"] == "repair_demo", targets
            assert targets[0]["kind"] == ["lib"], targets
            return document, result, targets[0]
        try:
            while not Path(env["SAIL_DAEMON_SOCKET"]).exists():
                assert daemon.poll() is None, "owned daemon failed before acceptance"
                remaining(5)
                time.sleep(.02)
            good_job, good = submit("success")
            document, result, target = full(good)
            cold_declarations = "Build-script input declarations were established during this run" in result["notes"]
            if cold_declarations:
                # Newly discovered declarations can leave cold evidence uncertain
                # and include dependency drivers. One subsequent observed full
                # invocation must establish current inputs; all cost is timed.
                assert result["exit_code"] == 0 and target["complete"], good
                if result["currentness"] != "current":
                    assert not document["accepted"], good
                good_job, good = submit("success-established")
                document, result, target = full(good)
            assert result["currentness"] == "current" and document["accepted"], good
            assert result["exit_code"] == 0 and target["complete"], good
            if document["batch"]["compiler_invocations"] != 1:
                # A current cold invocation can still establish dependency receipts.
                # Measure one additional forced stable batch; keep prior counters
                # in raw evidence and never retry again if this contract fails.
                prior_job = good_job
                prior_force = env.get("SAIL_MANAGED_FORCE")
                env["SAIL_MANAGED_FORCE"] = "1"
                try:
                    good_job, good = submit("success-single-driver")
                finally:
                    if prior_force is None:
                        env.pop("SAIL_MANAGED_FORCE", None)
                    else:
                        env["SAIL_MANAGED_FORCE"] = prior_force
                document, result, target = full(good)
                assert good_job != prior_job, "forced validation must start a distinct observed batch"
                assert result["currentness"] == "current" and document["accepted"], good
                assert result["exit_code"] == 0 and target["complete"], good
            assert document["batch"]["batched"] and document["batch"]["compiler_invocations"] == 1, document
            assert "engine_fault" not in document["batch"], document
            provenance(result, target)
            focus = document["batch"]["focus"]
            assert focus is not None and all(not t["complete"] for t in focus["targets"]), focus
            assert all(f["tier"] == "provisional" and f["verdict"] == "provisional"
                       for t in focus["targets"] for c in t["report"]["crates"] for f in c["functions"]), focus
            page = cli("success-events", ["jobs", "events", good_job, "--output", "json"])
            assert page["gap"] is None and page["events"], page
            assert any(e["input"]["phase"] == "focus" for e in page["events"]), page
            assert any(e["input"]["phase"] == "full" for e in page["events"]), page
            preliminary = [e for e in page["events"] if e["input"]["observation"]["kind"] == "function_observed"]
            seals = [e for e in page["events"] if e["input"]["observation"]["kind"] == "compiler_sealed"]
            if not (preliminary and seals and preliminary[0]["sequence"] < seals[-1]["sequence"]):
                # Retain a strict failed check while exercising the independent
                # final-report tools too. Older validated engines have no typed
                # function producer; they must not pass this acceptance script.
                progress_failures.append("no real provisional function observation before compiler seal")
            assert all(e["input"]["observation"]["value"]["tier"] == "provisional"
                       and e["input"]["observation"]["value"]["verdict"] == "provisional" for e in preliminary), page
            cursor = page["next"]
            resumed = cli("success-resume", ["jobs", "events", good_job, "--stream", cursor["stream_id"],
                          "--after", str(cursor["after"]), "--output", "json"])
            assert resumed["gap"] is None and all(e["sequence"] > cursor["after"] for e in resumed["events"]), resumed
            baseline = cli("baseline-save", ["baseline", "save", "before-repair", "--job", good_job, "--output", "json"])
            assert baseline["response"] == "baseline", baseline
            missing_job, missing = submit("missing", "repair_demo::missing")
            missing_document, missing_full, missing_target = full(missing)
            assert not missing_document["accepted"] and missing_document["batch"]["selection_error"]["code"] == "SAIL3122", missing
            assert missing_full["exit_code"] == 0 and missing_target["complete"], missing
            # Public foreground exit differs from the private worker/full status.
            rejected = cli("missing-foreground", ["repair-check", "--engine", "interactive",
                           "--package", "repair_demo", "--lib", "--function", "repair_demo::missing",
                           "--output", "json"], expected=1)
            assert not rejected["accepted"] and rejected["batch"]["full"]["exit_code"] == 0, rejected
            write(REGRESSION)
            failed_job, failed = submit("full-regression")
            failed_document, failed_full, failed_target = full(failed)
            assert not failed_document["accepted"] and failed_full["exit_code"] != 0, failed
            assert failed_document["batch"]["focus"]["exit_code"] == 0, failed
            package_id = failed_target["package_id"]
            explanation = cli("explain-regression", ["explain", failed_job, "--package-id", package_id,
                              "--target", "repair_demo", "--kind", "lib", "--function", "repair_demo::other",
                              "--output", "json"])
            assert explanation["response"] == "explanation", explanation
            function = explanation["value"]["function"]
            assert function["verdict"] == "failed" and function["obligations"], function
            counterexamples = [o["counterexample"] for o in function["obligations"] if o["counterexample"] is not None]
            assert counterexamples and all(model["model"].strip() for model in counterexamples), function
            assert explanation["value"]["run_diagnostics"], explanation
            compared = cli("compare-regression", ["compare", failed_job, "--baseline", "before-repair", "--output", "json"])
            assert compared["response"] == "comparison", compared
            assert any(d["function"] == "repair_demo::other" and d["change"] == "regressed"
                       for d in compared["value"]["differences"]), compared
            # Real stdio MCP, same durable jobs/evidence; no synthetic worker.
            mcp_errors = (evidence / "mcp.stderr").open("wb")
            mcp = subprocess.Popen([sail, "mcp"], cwd=project, env=env, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=mcp_errors, text=True,
                                   start_new_session=True)
            request_id = 0
            def exchange(method, parameters):
                nonlocal request_id
                request_id += 1
                message = {"jsonrpc":"2.0", "id":request_id, "method":method, "params":parameters}
                mcp.stdin.write(json.dumps(message) + "\n")
                mcp.stdin.flush()
                assert select.select([mcp.stdout], [], [], remaining())[0], "MCP response timed out"
                response = json.loads(mcp.stdout.readline())
                (evidence / f"mcp-{request_id:02d}.json").write_text(json.dumps(response) + "\n")
                assert response.get("id") == request_id and "error" not in response, response
                return response["result"]
            exchange("initialize", {"protocolVersion":"2025-11-25", "capabilities":{},
                     "clientInfo":{"name":"repair-acceptance", "version":"1"}})
            mcp.stdin.write(json.dumps({"jsonrpc":"2.0", "method":"notifications/initialized"}) + "\n")
            mcp.stdin.flush()
            def tool(name, arguments):
                result = exchange("tools/call", {"name":name, "arguments":arguments})
                assert not result["isError"], result
                return result["structuredContent"]
            mcp_page = tool("verification_events", {"job_id":good_job,
                            "stream_id":cursor["stream_id"], "after":cursor["after"]})
            assert mcp_page["next"]["after"] >= cursor["after"] and mcp_page["gap"] is None, mcp_page
            explained = tool("explain_failure", {"job_id":failed_job,"package_id":package_id,
                             "target":"repair_demo","kind":"lib","function":"repair_demo::other"})
            assert explained["value"]["function"] == function, explained
            comparison = tool("compare_verification", {"after_job":failed_job,"baseline":"before-repair"})
            assert any(d["change"] == "regressed" for d in comparison["value"]["differences"]), comparison
            requested = tool("start_repair_validation", {"project_dir":str(project),"package":"repair_demo",
                             "kind":"lib","function":"repair_demo::selected","engine":"interactive"})
            mcp_job = requested["id"]
            mcp_result = wait_job(mcp_job, "mcp-repair")
            assert not mcp_result["repair"]["accepted"] and mcp_result["repair"]["batch"]["focus"]["exit_code"] == 0, mcp_result
            write(LATE)
            late_job, late = submit("late-exec")
            late_document, late_full, late_target = full(late)
            assert not late_document["accepted"] and late_full["exit_code"] != 0 and not late_target["complete"], late
            assert late_document["batch"]["focus"] is not None, late
            assert any("E0499" in json.dumps(t["report"]["diagnostics"]) for t in late_full["targets"]), late
            write(RING)
            ring_job, ring = submit("missing-feature")
            ring_document, ring_full, ring_target = full(ring)
            assert not ring_document["accepted"] and ring_document["batch"]["engine_fault"]["code"] == "SAIL3124", ring
            assert "singular" in ring_document["batch"]["engine_fault"]["message"], ring
            assert ring_document["batch"].get("selection_error") is None, ring
            assert ring_target["report"]["engine"]["established_by"] == "stock-fallback", ring_target
            assert any(note.startswith("SAIL3124: ") for note in ring_full["notes"]), ring_full
            # The answer is the configured stock Verus's own: it lacks the feature too.
            assert ring_full["exit_code"] != 0 and "--features singular" in json.dumps(ring_target["report"]["diagnostics"]), ring
            assert not progress_failures, progress_failures
            summary["passed"] = True
        finally:
            # Stop owned actors even when the scenario failed.
            for process, timeout, close_stdin in ((mcp, 5, True), (daemon, 10, False)):
                if process is not None:
                    try:
                        stop_owned(process, timeout, close_stdin)
                    except (OSError, subprocess.SubprocessError) as failure:
                        print(f"actor cleanup: {failure}", file=sys.stderr)
            daemon_errors.close()
            if mcp_errors is not None:
                mcp_errors.close()
            summary.update(elapsed_s=time.monotonic()-started, operations=operations,
                           progress_failures=progress_failures)
            (evidence / "summary.json").write_text(json.dumps(summary,indent=2)+"\n")
        print("Real repair CLI/MCP acceptance: success with engine provenance, missing selector, full regression, late EXEC, missing Verus feature fallback, exact evidence/cursors passed", flush=True)



if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("sail", "engine-build", "verus-source", "out"):
        parser.add_argument("--" + flag, required=True)
    parser.add_argument("--timeout", type=float, default=120,
                        help="local proof/tool scenario deadline; CI cannot exceed 120 seconds")
    parser.add_argument("--environment-label", default="unspecified",
                        help="explicit native/emulated platform and load qualification")
    run(parser.parse_args())
