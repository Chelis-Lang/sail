#!/usr/bin/env python3
"""One focused ordinary-full wait diagnosis, never a reader acceptance retry.

Preparation freezes a private config and bounded prior sidecars without launching
processes. Run starts one owned service and one unchanged forced verification.
"""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import time
import threading
import sys
from types import SimpleNamespace

CONTEXT = "4bc68866277c17e21676a56e3eb6ce912582a6752da73feb8049763196203ad5"
REVISION = "821414360c2a5f474ff8cb6124db72606ab68366c26a240771c0b4a3cd24b947"
STAGE_WRAPPER_SHA = "27756c3dc44be3014eeb0b8525f87b22b0f5cbe6fbdd43ee22e6750dd5ec7144"
SUPPORT_SHA = "6f29f27975035ed253ec75e6cfbc8f22eaca24a9ad5256e927cf877253b34779"
OBSERVER_SHA = "c7bf44ba1c204f8730e4aa628f9be9bf2bab07fd3e95e48357fad0c16e5ee8cd"
EXCLUDED_ENV = {"SAIL_PROOF_EVENTS", "SAIL_ENGINE_EVENTS", "SAIL_ENGINE_FOCUS_REPORT", "SAIL_ENGINE",
                "SAIL_ENGINE_FUNCTION", "SAIL_ENGINE_CONTEXT", "SAIL_ENGINE_PREPARED_DIR", "SAIL_JOB_EVENTS",
                "SAIL_JOB_RPC_TIMINGS", "SAIL_JOB_GUARD_PARENT", "SAIL_MANAGED_FORCE", "SAIL_DAEMON_SOCKET",
                "SAIL_PROVENANCE_DIR", "SAIL_TRACE_DIR", "SAIL_PROJECT_SOCKET", "PWD", "OLDPWD", "SHLVL", "_"}


def bounded(path, limit):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, "rb") as source:
        if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
            raise ValueError("nonregular diagnostic input")
        data = source.read(limit + 1)
    if len(data) > limit:
        raise ValueError("diagnostic input size limit")
    return data


def sha(data):
    return hashlib.sha256(data).hexdigest()


def identity(path, limit=128 * 1024 * 1024):
    data = bounded(path, limit)
    return {"bytes": len(data), "sha256": sha(data)}


def write_json(path, value):
    with Path(path).open("x") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def validate_inherited_environment(environment):
    """Only the two baseline container selectors may be inherited."""
    unexpected = sorted(key for key in environment if key.startswith("SAIL_")
                        and key not in {"SAIL_TEST_Z3", "SAIL_VERUS_HOME"})
    if unexpected:
        raise ValueError("unapproved inherited Sail selector: " + ", ".join(unexpected))


def input_environment(environment):
    """Match the frozen runner's selector without exporting private values."""
    return {k: v for k, v in environment.items() if k not in EXCLUDED_ENV and not k.startswith("SAIL_MANAGED_")}


def deadline_expired(started, signalled):
    """A late observed completion never escapes the original absolute clock."""
    return signalled or time.monotonic() - started >= 600


def observation_due(elapsed, seen, context_ready, target_started):
    """At most five snapshots; reserve timeout capture before the 600-second edge."""
    for label, due in (("context-ready", context_ready), ("target-started", target_started),
                       ("60-seconds", elapsed >= 60), ("300-seconds", elapsed >= 300),
                       ("before-timeout", elapsed >= 599.3)):
        if due and label not in seen:
            return label
    return None


def event_rows(data, offset):
    """Consume only complete bounded JSONL rows; retain a partial tail in raw bytes."""
    result = []
    for line in data[offset:].splitlines(keepends=True):
        if not line.endswith(b"\n"):
            break
        if len(line) > 65536 or len(result) >= 2048:
            raise ValueError("event row bound")
        value = json.loads(line)
        if not isinstance(value, dict):
            raise ValueError("event row shape")
        result.append(value)
        offset += len(line)
    return offset, result


def check_context(document):
    if document.get("kind") == "context-ready":
        if (document.get("context_sha256") != CONTEXT or document.get("input_revision") != REVISION
                or document.get("complete") is not True):
            raise ValueError("diagnostic context/revision/completeness mismatch")
        return True
    return False


def dependencies(config):
    base = Path(config["original_controller"])
    for name, expected in (("m7-rpc-stage-workflow.py", STAGE_WRAPPER_SHA),
                           ("m7-sysroot-proof-diagnostic.py", SUPPORT_SHA)):
        if identity(base / name, 65536)["sha256"] != expected:
            raise ValueError("frozen original dependency changed")
    if identity(config["observer"], 65536)["sha256"] != OBSERVER_SHA:
        raise ValueError("birth-bound observer changed")
    stage = load("exact_original_stage_seals", base / "m7-rpc-stage-workflow.py")
    parser = argparse.ArgumentParser()
    for name in ("bin-dir", "product-grant", "source-runtime", "source-workspace", "source-reference",
                 "scripts-dir", "stock-bin-dir", "probe-bin-dir", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    for name in ("product-grant-sha256", "source-reference-sha256", "environment-label", "product-source"):
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args(json.loads(bounded(base / "argv.json", 65536)))
    args.freeze_only = True
    for name, expected in stage.SCRIPTS.items():
        if identity(args.scripts_dir / name, 128 * 1024)["sha256"] != expected:
            raise ValueError("original imported helper changed")
    helper = stage.load("frozen_original_reader_helpers", args.scripts_dir / "m7-reader-acceptance.py")
    seals = stage.seal(args, helper)
    return stage, helper, args, seals


def context_archive(root, destination):
    """Retain bounded original lifecycle files; never copy compiler target artifacts."""
    rows = {}
    visits = total = 0
    pending = [(Path(root), Path())]
    while pending:
        directory, relative = pending.pop()
        with os.scandir(directory) as iterator:
            for entry in iterator:
                visits += 1
                if visits > 2048:
                    raise ValueError("prior context traversal limit")
                name = relative / entry.name
                if (not relative.parts and entry.name in ("target", "engine-prepared")) or (relative.parts and entry.name in ("logs", "provenance", "invocations")):
                    # Preserve lifecycle/receipt/artifact metadata, not the large
                    # already-retained historical SMT streams or compiler cache.
                    continue
                if entry.is_symlink():
                    raise ValueError("unbound prior context alias")
                if entry.is_dir(follow_symlinks=False):
                    if len(name.parts) > 6:
                        raise ValueError("prior context depth limit")
                    pending.append((Path(entry.path), name))
                elif entry.is_file(follow_symlinks=False):
                    if len(rows) >= 512:
                        raise ValueError("prior context file count limit")
                    if not relative.parts and entry.name in ("sail-cargo", "sail-verus-driver"):
                        rows[str(name)] = dict(identity(entry.path), content_retained=False,
                                               reason="large installed executable bound by immutable product")
                        continue
                    data = bounded(entry.path, 8 * 1024 * 1024)
                    total += len(data)
                    if total > 64 * 1024 * 1024:
                        raise ValueError("prior context byte limit")
                    target = destination / name
                    target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                    with target.open("xb") as output:
                        output.write(data)
                    rows[str(name)] = {"bytes": len(data), "sha256": sha(data)}
    return rows


def prepare(args):
    config = json.loads(bounded(args.config, 1024 * 1024))
    if set(config) != {"original_controller", "original_output", "observer", "output"}:
        raise ValueError("private diagnostic config shape")
    out = Path(config["output"])
    original = Path(config["original_output"])
    if out == original or out in original.parents or original in out.parents:
        raise ValueError("diagnostic output must be separate")
    stage, helper, original_args, seals = dependencies(config)
    summary = json.loads(bounded(original / "summary.json", 2 * 1024 * 1024))
    baseline = original / "baseline"
    if stage.support.source_inventory(baseline) != seals["source_files"]:
        raise ValueError("preserved baseline source changed")
    if summary["input_seals_before"] != seals:
        raise ValueError("original immutable input seals differ")
    if (original / "cache.sock").exists() or (original / "cache.sock").is_symlink():
        raise ValueError("original endpoint still present")
    validate_inherited_environment(os.environ)
    env = dict(os.environ, SAIL_CACHE_DIR=str(original / "cache"),
               SAIL_DAEMON_SOCKET=str(original / "cache.sock"), SAIL_MANAGED_FORCE="1",
               CARGO_TARGET_DIR=str(baseline / "target"),
               SAIL_CARGO_VERUS=str(original_args.stock_bin_dir / "cargo-verus"),
               VERUS_Z3_PATH=str(original_args.stock_bin_dir / "z3"))
    imported = json.loads(bounded(original / "engine-install/stdout", 65536))
    env["RUSTUP_TOOLCHAIN"] = imported["rust_toolchain"]
    for name in ("SAIL_VERUS_SOURCE_BUILD", "SAIL_VERUS_RELEASE", "SAIL_SHARED_STORE", "SAIL_FRESHNESS_TRACE",
                 "SAIL_JOB_RPC_TIMINGS", "SAIL_JOB_EVENTS"):
        if env.get(name):
            raise ValueError("unexpected diagnostic or runtime override: " + name)
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    env["SAIL_JOB_EVENTS"] = str(out / "execution/setup-stage-events.jsonl")
    project = original / "cache/projects/513807a2ea449b4679e2f07ab324d198b7270d9a7427621e837cf7f40a959680"
    context = project / CONTEXT
    ordinary = context_archive(context, out / "original-context")
    repair = context_archive(project / "7dffdcde165b452207de1ef985d19f39880c9344aa4709a71e84a29871646f15", out / "original-repair-context")
    config.update(environment=env, command=[str(original_args.bin_dir / "sail"), "verify", "--incremental",
                                          "--output", "json", "--package", "human_eval_verus", "--lib"],
                  workspace=str(baseline), context=str(context))
    write_json(out / "execution.json", config)
    input_bytes = json.dumps(sorted(input_environment(env).items()), separators=(",", ":")).encode()
    freeze = {"kind": "focused-ordinary-wait-static-preparation", "proof_launched": False,
              "controller": identity(__file__, 65536), "config": identity(out / "execution.json", 1024 * 1024),
              "observer": identity(config["observer"], 65536), "immutable_seals": seals,
              "original_summary": identity(original / "summary.json", 2 * 1024 * 1024),
              "original_context_files": ordinary, "original_repair_context_files": repair,
              "input_environment_sha256": sha(input_bytes), "input_environment_keys": sorted(input_environment(env)),
              "expected_context": CONTEXT, "expected_revision": REVISION, "deadline_s": 600,
              "snapshot_limit": 5, "snapshot_each_s": .5, "output_watch_bytes": 32 * 1024 * 1024,
              "original_force_policy": "1", "no_repair_prep_or_census": True}
    write_json(out / "static-freeze.json", freeze)
    print(json.dumps({"static_freeze": identity(out / "static-freeze.json", 2 * 1024 * 1024),
                      "original_context_files": len(ordinary), "original_repair_context_files": len(repair)}))


def terminate(actor):
    """Only the retained child handle's process group is cleanup authority."""
    errors = []
    if actor.poll() is None:
        try:
            os.killpg(actor.pid, signal.SIGINT)
            try:
                actor.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(actor.pid, signal.SIGKILL)
                actor.wait(timeout=5)
        except (OSError, subprocess.SubprocessError) as error:
            errors.append(type(error).__name__)
    return errors


class OwnedDeadline:
    """Enforce the original launch clock independently of snapshot work."""
    def __init__(self, actor, started, budget=600):
        self.done = threading.Event()
        self.expired = threading.Event()
        self.errors = []

        def wait():
            remaining = max(0, budget - (time.monotonic() - started))
            if not self.done.wait(remaining) and actor.poll() is None:
                self.expired.set()
                self.errors.extend(terminate(actor))

        self.worker = threading.Thread(target=wait, name="sail-ordinary-deadline")
        self.worker.start()

    def finish(self):
        self.done.set()
        self.worker.join()


def run(args):
    out = args.out.resolve()
    before = json.loads(bounded(out / "static-freeze.json", 2 * 1024 * 1024))
    if identity(__file__, 65536) != before["controller"]:
        raise ValueError("frozen diagnostic controller changed")
    if identity(out / "execution.json", 1024 * 1024) != before["config"]:
        raise ValueError("frozen private configuration changed")
    config = json.loads(bounded(out / "execution.json", 1024 * 1024))
    stage, helper, original_args, seals = dependencies(config)
    if seals != before["immutable_seals"] or stage.support.source_inventory(Path(config["workspace"])) != seals["source_files"]:
        raise ValueError("immutable baseline/product/runtime seals changed")
    endpoint = Path(config["environment"]["SAIL_DAEMON_SOCKET"])
    if endpoint.exists() or endpoint.is_symlink():
        raise ValueError("pre-existing service endpoint")
    execution = out / "execution"
    execution.mkdir(mode=0o700, exist_ok=False)
    observer = load("all_task_birth_observer", Path(config["observer"]))
    started = None
    actor = None
    timed_out = False
    error = None
    observations = []
    seen = set()
    events = []
    event_offset = 0
    context_ready = target_started = False
    cleanup_errors = []
    try:
        # Configuration/setup only. No request, prewarming or manual Prepare.
        with helper.bench.OwnedDaemons(execution) as service:
            service.start("ordinary", Path(config["command"][0]), Path(config["workspace"]),
                          dict(config["environment"], SAIL_JOB_RPC_TIMINGS="1"))
            with (execution / "stdout").open("xb") as stdout, (execution / "stderr").open("xb") as stderr:
                started = time.monotonic()
                actor = subprocess.Popen(config["command"], cwd=config["workspace"], env=config["environment"],
                                         stdout=stdout, stderr=stderr, start_new_session=True)
                deadline_owner = None
                try:
                    deadline_owner = OwnedDeadline(actor, started)
                    capture = observer.Capture()
                    birth = observer.process_stat(capture.read(Path('/proc') / str(actor.pid) / 'stat'))
                    if birth is None:
                        raise ValueError("owned root birth unavailable")
                    root_ticks = birth["start_ticks"]
                    while actor.poll() is None:
                        elapsed = time.monotonic() - started
                        if elapsed >= 600:
                            timed_out = True
                            raise subprocess.TimeoutExpired(config["command"], 600)
                        event_path = execution / "setup-stage-events.jsonl"
                        if event_path.exists():
                            event_offset, new_events = event_rows(bounded(event_path, 2 * 1024 * 1024), event_offset)
                            for value in new_events:
                                if len(events) >= 2048:
                                    raise ValueError("event count limit")
                                events.append(value)
                                context_ready |= check_context(value)
                                target_started |= value.get("event") == "target-started" and value.get("target") == "human_eval_verus"
                        label = observation_due(elapsed, seen, context_ready, target_started)
                        # Each observer has its original 0.5-second budget. Do not
                        # start one too late to retain the original dispatch edge.
                        if label and elapsed < 599.5:
                            seen.add(label)
                            capture = observer.Capture()
                            processes = observer.owned_processes(actor.pid, capture, expected_start_ticks=root_ticks)
                            for process in processes:
                                if not process["identity_stable"]:
                                    continue
                                pid = process["pid"]
                                raw = capture.read(Path('/proc') / str(pid) / 'schedstat', 4096)
                                process["schedstat"] = raw.decode("ascii", "replace").strip() if raw else None
                                process["exe"] = capture.link(Path('/proc') / str(pid) / 'exe')
                                after = observer.process_stat(capture.read(Path('/proc') / str(pid) / 'stat'))
                                process["supplement_identity_stable"] = after is not None and after["start_ticks"] == process["start_ticks"] and after["parent"] == process["parent"]
                            observations.append({"label": label, "elapsed_s": elapsed, "processes": processes,
                                                 "limited": capture.limited, "errors": capture.errors})
                        if max((execution / "stdout").stat().st_size, (execution / "stderr").stat().st_size) > 32 * 1024 * 1024:
                            raise ValueError("partial stream size limit")
                        time.sleep(min(.02, max(0, 600 - (time.monotonic() - started))))
                    if deadline_expired(started, deadline_owner.expired.is_set()):
                        timed_out = True
                        raise subprocess.TimeoutExpired(config["command"], 600)
                    if actor.returncode != 0:
                        raise subprocess.CalledProcessError(actor.returncode, config["command"])
                    event_path = execution / "setup-stage-events.jsonl"
                    if event_path.exists():
                        event_offset, new_events = event_rows(bounded(event_path, 2 * 1024 * 1024), event_offset)
                        for value in new_events:
                            if len(events) >= 2048:
                                raise ValueError("event count limit")
                            events.append(value)
                            context_ready |= check_context(value)
                            target_started |= value.get("event") == "target-started" and value.get("target") == "human_eval_verus"
                    if not context_ready:
                        raise ValueError("completed invocation lacks matching context observation")
                finally:
                    if deadline_owner is not None:
                        deadline_owner.finish()
                        cleanup_errors.extend(deadline_owner.errors)
                        timed_out |= deadline_owner.expired.is_set()
                    cleanup_errors.extend(terminate(actor))

    except BaseException as failure:
        error = {"type": type(failure).__name__, "timeout_s": failure.timeout if isinstance(failure, subprocess.TimeoutExpired) else None}
        raise
    finally:
        if actor is not None:
            cleanup_errors.extend(terminate(actor))
        after = None
        seals_equal = False
        try:
            _, _, _, after = dependencies(config)
            seals_equal = after == before["immutable_seals"] and stage.support.source_inventory(Path(config["workspace"])) == after["source_files"]
        except BaseException as failure:
            cleanup_errors.append("postseals: " + type(failure).__name__)
        if not seals_equal:
            cleanup_errors.append("immutable seals differ")
        output_files = {}
        for name in ("stdout", "stderr", "setup-stage-events.jsonl"):
            try:
                path = execution / name
                if path.exists():
                    output_files[name] = identity(path, 32 * 1024 * 1024)
            except (OSError, ValueError) as failure:
                cleanup_errors.append("output seal: " + type(failure).__name__)
        record = {"kind": "isolated-focused-ordinary-wait-diagnostic", "acceptance": False,
                  "deadline_s": 600, "force": "1", "expected_context": CONTEXT, "expected_revision": REVISION,
                  "timed_out": timed_out, "error": error, "exit_code": actor.returncode if actor else None,
                  "elapsed_inclusive_s": time.monotonic() - started if started else None,
                  "context_observed": context_ready, "selected_target_started": target_started,
                  "observations": observations, "snapshots_missed_or_not_due": sorted(set(("context-ready", "target-started", "60-seconds", "300-seconds", "before-timeout")) - seen),
                  "event_rows": len(events), "cleanup_errors": cleanup_errors,
                  "immutable_seals_equal": seals_equal, "immutable_seals_after": after,
                  "output_files": output_files}
        try:
            write_json(execution / "terminal.json", record)
        except OSError as failure:
            cleanup_errors.append("terminal archive: " + type(failure).__name__)
            if error is None:
                raise
            # Preserve TimeoutExpired/the original operation failure.
            print("terminal archive failed: " + type(failure).__name__, file=sys.stderr)
        if cleanup_errors and error is None:
            raise RuntimeError("diagnostic cleanup or postseal failure")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="operation", required=True)
    prep = commands.add_parser("prepare")
    prep.add_argument("--config", type=Path, required=True)
    execute = commands.add_parser("run")
    execute.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    {"prepare": prepare, "run": run}[args.operation](args)


if __name__ == "__main__":
    main()
