#!/usr/bin/env python3
"""Original descriptor-bound readers and corrected retained workflow.

This local-only wrapper imports frozen, unchanged controllers and verifies the
current product byte/installation record before the already authorized run. The
record binds identities; genuine Current/live producer checks admit the workload.
See m7-descriptor-reader-workflow-plan.md.
"""
import argparse
import io
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import time
from types import SimpleNamespace

import hashlib
import stat
import re
SCRIPTS = {
    "m7-reader-acceptance.py": "84966d77cd679653b96ab9f05019d51f805538f3d4dd509c1591da1679b7634d",
    "m7-repair-bench.py": "3931f922af8da8317a857955915754e1d43e7c905b6171f381d0b6096352de33",
    "m7-retained-jobs-acceptance.py": "207b45ef034b6c1e947dbb0efdb87368b62d5ef8a4c42ab5768413670f775a11",
    "m7-job-read-bench.py": "3d5dfa5d47bbf7377c2d6c79c8ae9618eae6c4a080d66d31bb858fbb8e611720",
    "m7-native-product.py": "f2d52accdb735a14de0e8ae09f6297271391f652db45622320a133fed23f0ff0",
}
PROBES = {"job_read_bench": "f1632772daacc28e4bb5e7f7747e0e048fce7973aeff31b6ed65c5bbbd0e0b4d",
          "python-reader-probe.py": SCRIPTS["m7-job-read-bench.py"]}
CATALOGUE = {"bytes": 5510, "sha256": "6ffc129735220bf53511ac6bba818c5c3c7b61bca92d59c17292a807979b18d6"}
SUPPORT_SHA = "6f29f27975035ed253ec75e6cfbc8f22eaca24a9ad5256e927cf877253b34779"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Reuse only bounded identity/inventory/timer archival routines from the already
# reviewed diagnostic. None of its proof or product setup functions is called.
support = load("reader_owned_seals", Path(__file__).with_name("m7-sysroot-proof-diagnostic.py"))


def seal(args, helper):
    assert support.identity(support.__file__)["sha256"] == SUPPORT_SHA
    assert support.identity(args.product_grant)["sha256"] == args.product_grant_sha256
    grant, _ = helper.native_product.load_json(args.product_grant)
    assert grant["source_commit"] == args.product_source and grant["exit_code"] == 0
    assert grant["installation_only"] is True
    assert Path(grant["product_dir"]).resolve() == args.bin_dir
    product = {name: support.identity(args.bin_dir / name) for name in ("sail", "sail-z3")}
    for name, row in product.items():
        assert row == {key: grant["artifacts"][name][key] for key in ("bytes", "sha256")}
    ownership = helper.native_product.audit_products(args.bin_dir)
    assert ownership["ownership_prerequisite_met"], "product lease ownership unavailable"
    assert support.identity(args.source_reference)["sha256"] == args.source_reference_sha256
    reference, _ = helper.native_product.load_json(args.source_reference)
    expected = reference["input_seals_before"]["source_files"]
    actual = support.source_inventory(args.source_workspace)
    assert len(expected) == 274 and expected["Cargo.lock"] == support.GENERATED_LOCK
    assert not (args.source_workspace / "target").exists() and actual == expected
    manifest, _ = helper.native_product.load_json(args.source_runtime / "sail-build.json")
    assert support.identity(args.source_runtime / "sail-build.json")["sha256"] == support.MANIFEST
    assert len(manifest["artifacts"]) == 13
    artifacts = {}
    for name, digest in manifest["artifacts"].items():
        path = Path(name)
        assert not path.is_absolute() and ".." not in path.parts
        artifacts[name] = support.identity(args.source_runtime / path)
        assert artifacts[name]["sha256"] == digest
    credentials = ownership["credentials"]
    runtime_owners = {name: {"uid": (args.source_runtime / name).stat().st_uid,
                             "gid": (args.source_runtime / name).stat().st_gid}
                      for name in artifacts}
    assert all(row["uid"] == credentials["fsuid"] or credentials["cap_lease"]
               for row in runtime_owners.values()), "selected artifact lease ownership unavailable"
    scripts = {name: support.identity(args.scripts_dir / name) for name in SCRIPTS}
    assert all(row["sha256"] == SCRIPTS[name] for name, row in scripts.items())
    probes = {name: support.identity(args.probe_bin_dir / name) for name in PROBES}
    assert all(row["sha256"] == PROBES[name] for name, row in probes.items())
    catalogue = support.identity(args.scripts_dir.parent / "corpus/repair-edits.json")
    assert catalogue == CATALOGUE
    stock = {name: support.identity(args.stock_bin_dir / name) for name in ("cargo-verus", "verus", "z3")}
    assert stock == reference["input_seals_before"]["stock"]
    frozen = args.bin_dir / "freeze.json"
    # The sealed installation may have no controller metadata. Static sealing
    # can bind its byte record; execution still uses original Run.freeze.
    assert args.freeze_only or frozen.is_file(), "original controller freeze metadata required before launch"
    return {"product": product, "product_grant": support.identity(args.product_grant),
            "product_freeze": support.identity(frozen) if frozen.exists() else None,
            "product_ownership": ownership, "source_files": actual,
            "source_reference": support.identity(args.source_reference),
            "selected_manifest": support.identity(args.source_runtime / "sail-build.json"),
            "selected_artifacts": artifacts, "scripts": scripts, "probes": probes,
            "selected_artifact_ownership_prerequisite": runtime_owners,
            "collector_freeze": support.identity(args.probe_bin_dir / "freeze.json"),
            "stock": stock, "catalogue": catalogue, "wrapper": support.identity(__file__),
            "seal_support": support.identity(support.__file__)}


def readers_once(run, args):
    """Original full admission, live worker and Python-then-Rust fail-fast path."""
    run.run()
    assert run.summary["passed"] is True
    assert set(run.summary["censuses"]) == {"python-census", "rust-census"}
    run.summary["original_reader_passed"] = True


def retained_once(run, args):
    """Delegate only after both original census admissions pass."""
    readers_once(run, args)
    run.summary["retained_attempted"] = True
    run.summary["retained_harness_sha256"] = SCRIPTS["m7-retained-jobs-acceptance.py"]
    run.save()
    job = run.summary["proofs"]["baseline"]["job"]
    command = ["python3", str(args.scripts_dir / "m7-retained-jobs-acceptance.py"),
               "--sail", str(run.bin / "sail"), "--project", str(run.out / "baseline"),
               "--cache", str(run.cache), "--socket", str(run.socket),
               "--out", str(run.out / "retained-raw"), "--job", job,
               "--package", support.PACKAGE, "--target", support.PACKAGE,
               "--kind", "lib", "--function", support.FUNCTION,
               "--environment-label", run.args.environment_label]
    # Original harness clocks are whole120/CLI30/MCP5; this owned process
    # safety ceiling includes its original cleanup and is not a new RPC budget.
    run.command("retained-integration", command, run.out / "baseline", timeout=145, json_output=False)
    with (run.out / "retained-raw/summary.json").open("rb") as stream:
        retained, _ = support_original_json(stream)
    assert retained["passed"] is True
    run.summary.update(retained_functional_passed=True, passed=True)


def inherited_selectors(environment):
    """Reject unrecorded Sail overrides before constructing the original Run."""
    allowed = {"SAIL_TEST_Z3", "SAIL_VERUS_HOME"}
    unexpected = sorted(name for name in environment if name.startswith("SAIL_") and name not in allowed)
    assert not unexpected, "unexpected inherited Sail selector names: " + str(unexpected)
    return sorted(name for name in allowed if name in environment)


def observed_service(helper):
    """Inject observation only into this owned daemon's copied environment."""
    original = helper.Service
    class ObservedService(original):
        def start(self, mode, binary, workspace, environment):
            assert "SAIL_JOB_RPC_TIMINGS" not in environment
            observed = dict(environment, SAIL_JOB_RPC_TIMINGS="1")
            began = time.time_ns()
            result = super().start(mode, binary, workspace, observed)
            self.rpc_launch = {"pid": self.actors[mode][0].pid,
                               "before_unix_ns": began, "after_unix_ns": time.time_ns()}
            helper.rpc_launch = self.rpc_launch
            return result
    return ObservedService


def archive_rpc(run, helper):
    """Read only the owned shutdown export; never call an endpoint or admit a proof."""
    roots = [run.cache / layout for layout in ("projects/jobs", "results/projects/jobs")]
    paths = [root / "rpc-timings.json" for root in roots if (root / "rpc-timings.json").exists()]
    assert len(paths) == 1, "owned recorder export absent or ambiguous"
    descriptor = os.open(paths[0], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as stream:
        assert stat.S_ISREG(os.fstat(stream.fileno()).st_mode), "nonregular recorder export"
        payload = stream.read(256 * 1024 + 1)
    assert len(payload) <= 256 * 1024, "recorder export oversized"
    value, _ = support_original_json(io.BytesIO(payload))
    assert value["kind"] == "diagnostic-job-rpc-stages" and value["capacity"] == 256
    assert isinstance(value["rows"], list) and len(value["rows"]) <= 256
    assert value["daemon_pid"] == run.daemon.pid == helper.rpc_launch["pid"]
    destination = run.out / "rpc-timings.json"
    with destination.open("xb") as stream:
        stream.write(payload)
    run.summary["rpc_stage_archive"] = {
        "bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest(),
        "rows": len(value["rows"]), "started": value["started"],
        "dropped": value["dropped"], "overwritten": value["overwritten"],
        "origin_unix_ms": value["origin_unix_ms"], "daemon_launch": helper.rpc_launch,
        "layout": str(paths[0].parent.relative_to(run.cache))}


def support_original_json(stream):
    """Bound one non-authoritative lifecycle document, rejecting duplicate keys."""
    def unique(pairs):
        result = {}
        for key, value in pairs:
            assert key not in result, "duplicate lifecycle field"
            result[key] = value
        return result
    payload = stream.read(32 * 1024 * 1024 + 1)
    assert len(payload) <= 32 * 1024 * 1024
    return json.loads(payload, object_pairs_hook=unique), payload


def close_owned(run, helper):
    """Retain and close only the Popen handle owned by the unchanged controller."""
    child = run.daemon
    if child is None or child.poll() is not None:
        return
    began = time.monotonic()
    helper.bench.signal_owned(child, signal.SIGINT)
    forced = False
    try:
        child.wait(timeout=5)
    except subprocess.TimeoutExpired:
        forced = True
        helper.bench.signal_owned(child, signal.SIGKILL)
        child.wait(timeout=5)
    run.summary["retained_service_cleanup"] = {
        "pid": child.pid, "exit_code": child.returncode, "forced": forced,
        "wall_s": time.monotonic() - began}
    assert not forced and child.returncode == 0, "kept service did not close normally"


def execute(run, args, helper, before, began, reseal):
    """Preserve original failure even if archival, final seals or cleanup also fail."""
    failure = None
    try:
        retained_once(run, args)
    except BaseException as error:
        failure = error
        run.summary.update(passed=False, error={"type": type(error).__name__, "message": str(error)})
        raise
    finally:
        issues = []
        for stage, action in (("service_cleanup", lambda: close_owned(run, helper)),
                              ("rpc_stage_archive", lambda: archive_rpc(run, helper)),
                              ("worker_timers", lambda: support.archive_timers(run, helper)),
                              ("immutable_seals", lambda: check_after(run, before, reseal))):
            try:
                action()
            except BaseException as error:
                issues.append({"stage": stage, "type": type(error).__name__, "message": str(error)})
        run.summary["outer_finalization_errors"] = issues
        run.summary["inclusive_elapsed_s"] = time.monotonic() - began
        if issues:
            run.summary["passed"] = False
        run.save()
        if issues and failure is None:
            raise RuntimeError("owned workflow finalization failed: " + str(issues))


def check_after(run, before, reseal):
    after = reseal()
    run.summary.update(input_seals_after=after, immutable_inputs_unchanged=before == after)
    assert after == before, "immutable workflow inputs changed"


def main():
    began = time.monotonic()
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("bin-dir", "product-grant", "source-runtime", "source-workspace",
                 "source-reference", "scripts-dir", "stock-bin-dir", "probe-bin-dir", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    for name in ("product-grant-sha256", "source-reference-sha256", "environment-label", "product-source"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--freeze-only", action="store_true")
    args = parser.parse_args()
    inherited_selectors(os.environ)
    assert re.fullmatch(r"[0-9a-f]{40}", args.product_source), "exact production source required"
    assert not os.environ.get("SAIL_JOB_RPC_TIMINGS"), "daemon-only observation required"
    assert not os.environ.get("CI") and Path("/proc/self/status").exists(), "local Linux only"
    for name, value in vars(args).items():
        if isinstance(value, Path):
            setattr(args, name, value.resolve())
    assert not args.out.exists(), "new output required"
    assert args.source_workspace not in args.out.parents and args.out not in args.source_workspace.parents
    # Verify all imported original helpers before executing their module code.
    for name, digest in SCRIPTS.items():
        assert support.identity(args.scripts_dir / name)["sha256"] == digest
    helper = load("original_current_readers", args.scripts_dir / "m7-reader-acceptance.py")
    helper.Service = observed_service(helper)
    before = seal(args, helper)
    if args.freeze_only:
        print(json.dumps(before, sort_keys=True))
        return 0
    run = helper.Run(SimpleNamespace(out=args.out, bin_dir=args.bin_dir,
        source_workspace=args.source_workspace, source_runtime=args.source_runtime,
        probe_bin_dir=args.probe_bin_dir, python_probe=args.probe_bin_dir / "python-reader-probe.py",
        prepare_contexts=True, keep_daemon=True, setup_timeout=600, lease_inventory=None,
        package=support.PACKAGE, target=support.PACKAGE, kind="lib", function=support.FUNCTION,
        environment_label=args.environment_label))
    run.summary.update(kind="descriptor-bound-original-reader-retained-workflow", input_seals_before=before,
        original_reader_passed=False, retained_functional_passed=False, retained_attempted=False,
        scope="Original descriptor product46 reader gates then corrected retained functional scope; diagnostic stages only, no source physical, 2x or overall M7 qualification")
    run.env.update(SAIL_CARGO_VERUS=str(args.stock_bin_dir / "cargo-verus"),
                   VERUS_Z3_PATH=str(args.stock_bin_dir / "z3"),
                   SAIL_JOB_EVENTS=str(run.out / "setup-stage-events.jsonl"))
    for name in ("SAIL_VERUS_SOURCE_BUILD", "SAIL_VERUS_RELEASE", "SAIL_SHARED_STORE", "SAIL_FRESHNESS_TRACE"):
        assert not run.env.get(name), "unexpected runtime or tracing override: " + name
    run.save()
    execute(run, args, helper, before, began, lambda: seal(args, helper))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
