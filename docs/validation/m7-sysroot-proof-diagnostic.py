#!/usr/bin/env python3
"""One separately granted sysroot cause diagnostic; no acceptance or warmup route."""
import argparse
import hashlib
import importlib.util
import os
import re
from pathlib import Path
import stat
import time
from types import SimpleNamespace

MANIFEST = "4b6f48d278e981f9f3ce40b5e8238ffed2409cac61f003c09fd8ee0c2c9a7c41"
FUNCTION = "human_eval_verus::human_eval_013::compute_gcd"
PACKAGE = "human_eval_verus"
GENERATED_LOCK = {"bytes": 3007, "sha256": "a38eff2ab6773bcb3f8a8b32068b6b9687cae6d168b3a20f8002578649245525"}
HELPERS = ("m7-reader-acceptance.py", "m7-repair-bench.py", "m7-retained-jobs-acceptance.py",
           "m7-job-read-bench.py", "m7-native-product.py")


def identity(path, limit=128 * 1024 * 1024):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as stream:
        assert stat.S_ISREG(os.fstat(stream.fileno()).st_mode), "nonregular input"
        count, digest = 0, hashlib.sha256()
        while chunk := stream.read(min(1024 * 1024, limit + 1 - count)):
            count += len(chunk)
            assert count <= limit, "input byte bound exceeded"
            digest.update(chunk)
    return {"bytes": count, "sha256": digest.hexdigest()}


def source_inventory(workspace):
    """Seal source bytes only; production guards still establish complete inputs."""
    rows, visits, total = {}, 0, 0
    for directory, folders, names in os.walk(workspace, followlinks=False):
        folders[:] = [name for name in folders if name not in (".git", ".claude", ".codex")
                      and Path(directory, name) != workspace / "target"]
        for name in folders:
            assert not Path(directory, name).is_symlink(), "unbound source directory alias"
        visits += len(folders) + len(names)
        assert visits <= 16384, "source traversal bound exceeded"
        for name in names:
            path = Path(directory, name)
            relative = str(path.relative_to(workspace))
            assert len(os.fsencode(relative)) <= 4096 and len(rows) < 4096
            row = identity(path, 32 * 1024 * 1024)
            total += row["bytes"]
            assert total <= 256 * 1024 * 1024, "source byte bound exceeded"
            rows[relative] = row
    return rows


def seal(args, helper):
    grant, _ = helper.native_product.load_json(args.product_grant)
    assert identity(args.product_grant)["sha256"] == args.product_grant_sha256
    assert Path(grant["product_dir"]).resolve() == args.bin_dir
    assert grant["source_commit"] == "7238da8406cd3a5d62b8c378949d36832446aed8"
    assert grant["installation_only"] is True and grant["build_exit_code"] == 0
    for name in ("sail", "sail-z3"):
        row = identity(args.bin_dir / name)
        assert row == {k: grant["artifacts"][name][k] for k in ("bytes", "sha256")}
    manifest, _ = helper.native_product.load_json(args.source_runtime / "sail-build.json")
    assert identity(args.source_runtime / "sail-build.json")["sha256"] == MANIFEST
    assert len(manifest["artifacts"]) == 13
    artifacts = {}
    for name, digest in manifest["artifacts"].items():
        path = Path(name)
        assert not path.is_absolute() and ".." not in path.parts
        artifacts[name] = identity(args.source_runtime / path)
        assert artifacts[name]["sha256"] == digest
    probe, _ = helper.native_product.load_json(args.probe_bin_dir / "freeze.json")
    probe_files = {name: identity(path) for name, path in (("job_read_bench", args.probe_bin_dir / "job_read_bench"),
                    ("python-reader-probe.py", args.python_probe))}
    for name, row in probe_files.items():
        assert row["sha256"] == probe["files"][name]["sha256"], "unused collector seal changed"
    opaque = {name: identity(getattr(args, name)) for name in (
        "original_result", "original_summary", "original_input_state", "original_runtime_identities")}
    assert opaque["original_result"]["sha256"] == args.original_result_sha256
    assert opaque["original_summary"]["sha256"] == args.original_summary_sha256
    assert opaque["original_input_state"]["sha256"] == args.original_input_state_sha256
    assert opaque["original_runtime_identities"]["sha256"] == args.original_runtime_identities_sha256
    ownership = helper.native_product.audit_products(args.bin_dir)
    assert ownership["ownership_prerequisite_met"], "product lease ownership prerequisite unavailable"
    return {"product_ownership_prerequisite": ownership, "product": {name: identity(args.bin_dir / name) for name in ("sail", "sail-z3")},
            "product_grant": identity(args.product_grant),
            "selected_manifest": identity(args.source_runtime / "sail-build.json"),
            "selected_artifacts": artifacts, "source_files": source_inventory(args.workspace),
            "original_opaque_references": opaque,
            "stock": {name: identity(args.stock_bin_dir / name) for name in ("cargo-verus", "verus", "z3")},
            "helpers": {name: identity(args.scripts_dir / name) for name in HELPERS},
            "unused_collector_bundle": identity(args.probe_bin_dir / "freeze.json"),
            "unused_collectors": probe_files, "controller": identity(__file__)}


def bind_source(actual, original):
    """Keep the exact template plus the already present, explicitly sealed lockfile."""
    expected = dict(original)
    assert "Cargo.lock" not in expected, "unexpected original template lockfile"
    expected["Cargo.lock"] = GENERATED_LOCK
    assert actual == expected, "original template or bound generated lockfile differs"


def freeze_product(run, args, helper):
    """Use the installation grant without modifying sealed product directories."""
    grant, _ = helper.native_product.load_json(args.product_grant)
    setup = helper.native_product.audit_products(run.bin)
    run.summary["product_ownership_prerequisite"] = setup
    assert setup["ownership_prerequisite_met"], "product lease ownership prerequisite unavailable"
    probe, _ = helper.native_product.load_json(run.probe_dir / "freeze.json")
    for name, path in (("job_read_bench", run.probe_dir / "job_read_bench"),
                       ("python-reader-probe.py", args.python_probe)):
        assert identity(path)["sha256"] == probe["files"][name]["sha256"], "unused collector seal changed"
    run.summary.update(product_grant=grant, collector_freeze=probe,
                       controller_scripts={name: identity(args.scripts_dir / name) for name in HELPERS})
    run.save()


def archive_timers(run, helper):
    """Read only exact owned regular timer sidecars after worker and service cleanup."""
    evidence, total = {}, 0
    for job in run.owned:
        with helper.owned_job(run.cache, job, run.job_scopes[job]) as (directory, _, layout):
            names = sorted(name for name in os.listdir(directory)
                           if re.fullmatch(r"events\.setup-[0-9]+-[0-9]+\.jsonl", name))
            assert len(names) <= 8, "owned timer file count exceeded"
            for name in names:
                with helper.owned_file(directory, name) as stream:
                    payload = stream.read(256 * 1024 + 1)
                total += len(payload)
                assert len(payload) <= 256 * 1024 and total <= 1024 * 1024, "owned timer bytes exceeded"
                destination = run.out / "worker-timers" / job / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                with destination.open("xb") as stream:
                    stream.write(payload)
                evidence[job + "/" + name] = {
                    "bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest(), "layout": layout}
    run.summary["worker_timer_archives"] = evidence


def proof_once(run, helper, manifest, package_id):
    """Reuse owned transport/lifecycle; one submission, no reproof/admission loop."""
    assert run.args.setup_timeout == 600 and not run.owned
    with helper.Service(run.out, False) as service:
        service.start("diagnostic", run.bin / "sail", run.source, run.env)
        run.daemon = service.actors["diagnostic"][0]
        try:
            job = run.submit("exact-repair-diagnostic", run.source)
            document = run.wait("exact-repair-diagnostic", job, manifest, package_id)
            helper.bench.write_json(run.out / "observed-full-result.json", document)
            batch = document["repair"]["batch"]
            assert batch["manifest_sha256"] == MANIFEST
            full = batch["full"]
            helper.same_scope(full, package_id, PACKAGE, "lib")
            run.summary["observation"] = {
                "job": document["job"], "accepted": document["repair"]["accepted"],
                "batched": batch["batched"], "compiler_invocations": batch["compiler_invocations"],
                "selection_error": batch.get("selection_error"),
                "context_sha256": full["context_sha256"], "input_revision": full["input_revision"],
                "currentness": full["currentness"], "notes": full["notes"],
                "execution": full.get("execution"),
                "targets": [{"target": target["target"], "complete": target["complete"],
                             "reused": target["reused"], "summary": target["report"]["summary"]}
                            for target in full["targets"]]}
            run.summary["diagnostic_completed"] = True
        finally:
            run.finish_jobs()
    assert not run.summary["cleanup_errors"], run.summary["cleanup_errors"]
    assert not run.summary["archival_errors"], run.summary["archival_errors"]


def main():
    parent_began = time.monotonic()
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("bin-dir", "source-runtime", "workspace", "out", "scripts-dir", "stock-bin-dir",
                 "probe-bin-dir", "python-probe", "product-grant", "original-result", "original-summary", "original-input-state",
                 "original-runtime-identities"):
        parser.add_argument("--" + name, type=Path, required=True)
    for name in ("product-grant-sha256", "original-result-sha256", "original-summary-sha256", "original-input-state-sha256",
                 "original-runtime-identities-sha256"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--freeze-only", action="store_true", help="Seal inputs without executing product, metadata or proof")
    args = parser.parse_args()
    assert os.name == "posix" and Path("/proc/self/status").exists(), "Linux diagnostic only"
    for name, value in vars(args).items():
        if isinstance(value, Path):
            setattr(args, name, value.resolve())
    assert not args.out.exists() and args.out != args.workspace
    assert args.workspace not in args.out.parents and args.out not in args.workspace.parents
    spec = importlib.util.spec_from_file_location("frozen_cause_controller", args.scripts_dir / HELPERS[0])
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    before = seal(args, helper)
    summary, _ = helper.native_product.load_json(args.original_summary)
    original_seals = summary["launch_freeze"]["input_seals_before"]
    bind_source(before["source_files"], original_seals["source_only_template"])
    assert before["stock"] == original_seals["stock_runtime"], "original stock compiler/solver bytes differ"
    old, _ = helper.native_product.load_json(args.original_result)
    reference = old["value"]
    assert old["response"] == "result"
    assert Path(reference["job"]["project_dir"]).resolve() == args.workspace
    assert reference["job"]["args"] == ["--package", PACKAGE, "--lib"]
    assert reference["repair"]["batch"]["manifest_sha256"] == MANIFEST
    if args.freeze_only:
        args.out.mkdir(parents=True, mode=0o700, exist_ok=False)
        helper.bench.write_json(args.out / "launch-freeze.json", {
            "kind": "diagnostic-input-seal-only", "acceptance": False,
            "inputs": before, "arguments": {name: str(value) if isinstance(value, Path) else value
                                               for name, value in vars(args).items()},
            "original_job": reference["job"]["id"], "proofs_launched": 0,
            "source_difference": {"existing_generated_Cargo.lock": GENERATED_LOCK,
                "historical_lockfile_hash_retained": False, "original_template_files": 273}})
        return 0
    run = helper.Run(SimpleNamespace(out=args.out, bin_dir=args.bin_dir, source_workspace=args.workspace,
        source_runtime=args.source_runtime, probe_bin_dir=args.probe_bin_dir, python_probe=args.python_probe,
        prepare_contexts=False, keep_daemon=False, setup_timeout=600, lease_inventory=None,
        package=PACKAGE, target=PACKAGE, kind="lib", function=FUNCTION,
        environment_label="Linux amd64 Docker emulation; single cause diagnostic, independent concurrent work"))
    run.summary.update(kind="single-sysroot-cause-diagnostic", passed=False, diagnostic_completed=False,
        acceptance=False, input_seals_before=before,
        scope="One selected-b346 cause investigation; no warmup, retry, collectors or M7 qualification",
        original_reference={"job": reference["job"]["id"],
            "context_sha256": reference["repair"]["batch"]["full"]["context_sha256"],
            "input_revision": reference["repair"]["batch"]["full"]["input_revision"],
            "notes": reference["repair"]["batch"]["full"]["notes"]},
        context_difference="New executable and cache paths necessarily change context identity; no old authority imported",
        source_difference={"existing_generated_Cargo.lock": GENERATED_LOCK,
            "historical_lockfile_hash_retained": False, "original_template_files": 273},
        setup_contract="One exact repair diagnostic, no ordinary or repair preparation",
        collectors_attempted=False)
    run.env.update(SAIL_CARGO_VERUS=str(args.stock_bin_dir / "cargo-verus"),
                   VERUS_Z3_PATH=str(args.stock_bin_dir / "z3"),
                   SAIL_JOB_EVENTS=str(run.out / "setup-stage-events.jsonl"))
    for name in ("SAIL_VERUS_SOURCE_BUILD", "SAIL_VERUS_RELEASE", "SAIL_SHARED_STORE", "SAIL_FRESHNESS_TRACE"):
        assert not run.env.get(name), "unexpected diagnostic runtime/trace override: " + name
    failure = None
    try:
        freeze_product(run, args, helper)
        imported = run.command("engine-install", [str(run.bin / "sail"), "engine", "install", "--from",
            str(args.source_runtime), "--output", "json"], run.source, timeout=30)
        assert imported["manifest_sha256"] == MANIFEST
        run.env["RUSTUP_TOOLCHAIN"] = imported["rust_toolchain"]
        metadata = run.command("exact-scope-metadata", ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            run.source, env=dict(run.env, CARGO_TARGET_DIR=str(run.source / "target")), timeout=30)
        package_id = helper.retained.package_identity(metadata, PACKAGE, PACKAGE, "lib")
        proof_once(run, helper, MANIFEST, package_id)
    except BaseException as error:
        failure = error
        run.summary["error"] = {"type": type(error).__name__, "message": str(error)}
        raise
    finally:
        try:
            archive_timers(run, helper)
            after = seal(args, helper)
            run.summary.update(input_seals_after=after, immutable_inputs_unchanged=after == before)
            assert after == before, "diagnostic immutable input seals changed"
        except BaseException as error:
            run.summary["sealing_error"] = {"type": type(error).__name__, "message": str(error)}
            if failure is None:
                raise
        finally:
            run.summary["inclusive_elapsed_s"] = time.monotonic() - parent_began
            run.save()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
