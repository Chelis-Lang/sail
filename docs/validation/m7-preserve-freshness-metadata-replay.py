#!/usr/bin/env python3
"""Replay the complete component scope using independently sealed tested images.

No image is rebuilt or installed here. The only compiler command is the original
all-target strict check. The lead supplies exclusive target ownership and an
independent before/after holder, source-mode and protected-artifact audit.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import time


def controls():
    path = Path(__file__).with_name("m7-preserve-freshness-metadata-controls.py")
    spec = importlib.util.spec_from_file_location("metadata_controls", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def verify_prior(module, prior, binding, historical):
    for name, key in [("m7-preserve-freshness-metadata-controls.py", "prior_controller_sha256"),
                      ("m7-preserve-freshness-metadata-source.json", "prior_source_record_sha256"),
                      ("m7-preserve-freshness-metadata-counterfactual.patch", "counterfactual_sha256")]:
        if module.sha(historical / name) != binding[key]:
            raise ValueError("historical controller/source/patch identity mismatch")
    if module.sha(prior / "result.json") != binding["prior_result_sha256"]:
        raise ValueError("prior result identity mismatch")
    result = json.loads((prior / "result.json").read_text())
    expected = ["positive-build", "metadata", "freshness", "strict", "removed-noop-build", "removed-noop"]
    if ([p["name"] for p in result["phases"]] != expected or result["status"] != "FAIL"
            or not result["source_equal"] or not result["lock_equal"]
            or result["active_leader_pid"] is not None or result["closure_uncertainty_reasons"]
            or result["source"] != binding["source"] or result["lock_sha256"] != binding["lock_sha256"]):
        raise ValueError("prior compilation/source lineage mismatch")
    for phase, code in zip(result["phases"], [0, 0, 0, 0, 0, 101]):
        if phase["exit_code"] != code:
            raise ValueError("prior phase outcome mismatch")
    for name, digest in binding["prior_logs"].items():
        if module.sha(prior / name) != digest:
            raise ValueError("prior compiler/test log mismatch")
    negative = (prior / "removed-noop.log").read_text()
    if not module.expected_negative(101, negative):
        raise ValueError("actual prior failure does not identify exact unchanged assertion")
    selected = []
    for key, phase in [("positive_image", result["phases"][0]), ("negative_image", result["phases"][4])]:
        expected_image = binding[key]
        if phase["tested_images"] != [expected_image]:
            raise ValueError("compiled image lineage mismatch")
        path = (prior / "tested-binaries" / expected_image["name"]).resolve(strict=True)
        if not path.is_relative_to(prior) or not path.is_file():
            raise ValueError("image path outside retained prior scope")
        if (path.stat().st_size != expected_image["bytes"] or path.stat().st_mode & 0o7777 != 0o755
                or module.sha(path) != expected_image["sha256"]):
            raise ValueError("compiled image identity mismatch")
        selected.append(path)
    return selected


def run(arguments):
    module = controls()
    source = arguments.source.resolve(strict=True)
    target = arguments.target.resolve(strict=True)
    prior = arguments.prior_scope.resolve(strict=True)
    historical = arguments.prior_controller.resolve(strict=True)
    binding_path = Path(__file__).with_name("m7-preserve-freshness-metadata-replay-binding.json")
    binding = json.loads(binding_path.read_text())
    source_before = {name: module.sha(source / name) for name in binding["source"]}
    if source_before != binding["source"] or module.sha(source / "Cargo.lock") != binding["lock_sha256"]:
        raise ValueError("positive restored source or lock mismatch")
    patch = Path(__file__).with_name("m7-preserve-freshness-metadata-counterfactual.patch")
    if module.sha(patch) != binding["counterfactual_sha256"]:
        raise ValueError("original counterfactual identity mismatch")
    positive, negative = verify_prior(module, prior, binding, historical)
    strict = ["cargo", "clippy", "--locked", "-p", "sail-verus", "--release", "--all-targets", "--", "-D", "warnings"]
    if not arguments.execute:
        print(json.dumps({"status": "prepare-only", "images_verified": True,
                          "source_verified": True, "prior_failure_preserved": True,
                          "fresh_tests": ["metadata", "managed::freshness::tests::", module.CASE],
                          "strict": strict, "compiler_actor_started": False}))
        return
    output = arguments.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    environment = dict(os.environ)
    for name in list(environment):
        if name.startswith("SAIL_ENGINE"):
            del environment[name]
    environment.update(CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS="1", CARGO_INCREMENTAL="0")
    owner = module.OwnedCommands()

    def interrupt(number, _frame):
        owner.uncertain("replay signal; descendant closure requires independent audit")
        for value in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(value, signal.SIG_IGN)
        raise InterruptedError("replay signal " + str(number))

    for value in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(value, interrupt)
    result = {"schema": 1, "status": "RUNNING", "scope": "new full component replay",
              "binding_sha256": module.sha(binding_path), "prior_scope_status": "FAIL unchanged",
              "compiled_images_reused": [binding["positive_image"], binding["negative_image"]],
              "warm_build_or_acceptance_credit": False, "actor_closure_certified": False,
              "source": binding["source"], "lock_sha256": binding["lock_sha256"], "phases": []}

    def save():
        result["owned_group_starts"] = list(owner.starts)
        result["active_leader_pid"] = owner.active.pid if owner.active else None
        result["spawn_closure_uncertain"] = owner.spawning
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")

    def phase(name, argv, cap, counts=None, negative_case=False):
        owner.require_closed()
        row = {"name": name, "argv": argv, "limit_seconds": cap, "exit_code": None}
        result["phases"].append(row)
        save()
        path = output / (name + ".log")
        started = time.monotonic()
        try:
            with path.open("xb") as log:
                row["exit_code"] = owner.execute(argv, source, environment, log, cap)
        finally:
            row["elapsed_seconds"] = time.monotonic() - started
            row["log_sha256"] = module.sha(path) if path.is_file() else None
            save()
        text = path.read_text(errors="replace")
        row["pass"] = module.expected_negative(row["exit_code"], text) if negative_case else row["exit_code"] == 0
        if counts is not None:
            summary = re.search(r"test result: ok\. ([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored;", text)
            row["pass"] = row["pass"] and summary is not None and tuple(map(int, summary.groups())) == counts
        save()
        if not row["pass"]:
            raise ValueError("unexpected fresh replay outcome: " + name)

    try:
        phase("metadata", [str(positive), "metadata", "--nocapture", "--test-threads=1"], 180, counts=(8, 0, 0))
        phase("freshness", [str(positive), "managed::freshness::tests::", "--nocapture", "--test-threads=1"], 180, counts=(30, 0, 1))
        phase("strict", strict, 600)
        phase("removed-noop", [str(negative), module.CASE, "--exact", "--nocapture", "--test-threads=1"], 180, negative_case=True)
        phase("restored-positive", [str(positive), module.CASE, "--exact", "--nocapture", "--test-threads=1"], 180, counts=(1, 0, 0))
        result["status"] = "PASS"
    except BaseException as error:
        result["status"] = "FAIL"
        result["error_kind"] = type(error).__name__
    finally:
        result["closure_uncertainty_reasons"] = list(owner.reasons)
        result["process_group_closure_uncertain"] = bool(owner.reasons)
        if owner.reasons or owner.active is not None or owner.spawning:
            result["source_equal"] = None
            result["lock_equal"] = None
            result["images_equal"] = None
        else:
            result["source_equal"] = {name: module.sha(source / name) for name in source_before} == source_before
            result["lock_equal"] = module.sha(source / "Cargo.lock") == binding["lock_sha256"]
            try:
                verify_prior(module, prior, binding, historical)
                result["images_equal"] = True
            except (OSError, ValueError):
                result["images_equal"] = False
            if not all(result[key] for key in ["source_equal", "lock_equal", "images_equal"]):
                result["status"] = "FAIL"
        save()
    raise SystemExit(0 if result["status"] == "PASS" else 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source", "target", "prior-scope", "prior-controller", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--execute", action="store_true")
    run(parser.parse_args())


if __name__ == "__main__":
    main()
