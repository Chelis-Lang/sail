#!/usr/bin/env python3
"""Measure native temporal reuse using D.I.V.E.'s pinned S7 fixture envelopes.

The fixtures mode reads the five S7 models and their pinned oracle goldens.
Each measurement invocation runs one engine and one repetition.
The scenarios and the 100-check loop each start with a fresh private cache.
"""

import argparse
from contextlib import contextmanager
from copy import deepcopy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time

LOOP_MODELS = ["DieHard", "EWD840", "MCRealTimeHourClock", "MCLiveInternalMemory"]


def fixtures(args):
    if not args.dive or not args.examples:
        raise ValueError("fixture generation requires --dive and --examples checkouts")
    source = args.dive / "scripts/perf/release/sail/sail_temporal.py"
    spec = importlib.util.spec_from_file_location("s7_fixtures", source)
    upstream = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(upstream)
    upstream.EX = str(args.examples)
    upstream.GOLD = str(args.dive / "conformance/golden/tlc-2026.10.06.014338/examples")
    upstream.FIX = str(args.fixtures)
    args.fixtures.mkdir(parents=True, exist_ok=True)
    if any(args.fixtures.iterdir()):
        raise ValueError("fixture output must be empty")
    for name, module, cfg in upstream.CHECKS:
        if name in [*LOOP_MODELS, "MCLamportMutex"]:
            gold = upstream.golden(module, cfg)
            op = {
                "kind": "check",
                "max_states": max(1000, 2 * gold["distinct"]),
                "max_depth": 100000,
            }
            upstream.write_fixture(
                name, module, cfg, upstream.request(module, cfg, op), "check", gold
            )


def digest(value):
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def job(model):
    return model["input"]["job"]


def edited(model, file, old, new):
    result = deepcopy(model)
    files = job(result)["request"]["model"]["files"]
    if files[file].count(old) != 1:
        raise ValueError(f"expected a unique edit in {file}: {old!r}")
    files[file] = files[file].replace(old, new)
    return result


def sequence(fixtures, suite):
    names = [*LOOP_MODELS, "MCLamportMutex"]
    original = {}
    gold = {}
    for name in names:
        original[name] = json.loads((fixtures / name / "envelope.json").read_text())
        golden = json.loads((fixtures / name / "fixture.json").read_text())["golden"]
        gold[name] = (
            "holds" if golden["verdict"] == "success" else "refuted",
            golden["distinct"],
        )

    def step(label, name, model=None, expected=None):
        return label, name, model or original[name], expected or gold[name]

    dh, ewd, clock, memory = LOOP_MODELS
    mutex = "MCLamportMutex"
    if suite == "audit":
        fix = edited(
            original[dh], dh + ".tla", "NotSolved == big # 4", "NotSolved == big # 6"
        )
        return [
            step("fresh-refutation", dh),
            step("unchanged-refutation", dh),
            step("fix", dh, fix, ("holds", 16)),
            step("unchanged-holds", dh, fix, ("holds", 16)),
            step("revert-refutation", dh),
        ]
    if suite == "scenarios":
        mutex_edit = edited(
            original[mutex],
            mutex + ".cfg",
            "TypeOK BoundedNetwork Mutex",
            "TypeOK Mutex",
        )
        prop_edit = edited(
            original[memory], memory + ".cfg", "\nPROPERTY Liveness\n", "\n"
        )
        valid_fail = edited(
            original[dh],
            dh + ".tla",
            "NotSolved == big # 4",
            "NotSolved == big # 4 /\\ small <= 3",
        )
        fix = edited(
            original[dh], dh + ".tla", "NotSolved == big # 4", "NotSolved == big # 6"
        )
        return [
            step("first-holds", mutex),
            step("unchanged-holds", mutex),
            step("invariant-edit", mutex, mutex_edit, ("holds", gold[mutex][1])),
            step("revert-holds", mutex),
            step("first-temporal", memory),
            step("property-edit", memory, prop_edit, ("holds", gold[memory][1])),
            step("first-refutation", dh),
            step("unchanged-refutation", dh),
            step("edit-preserves-refutation", dh, valid_fail, ("refuted", None)),
            step("fix", dh, fix, ("holds", None)),
            step("revert-refutation", dh),
        ]

    result = []
    for batch in range(20):
        index = batch // 4
        name = LOOP_MODELS[batch % 4]
        base = original[name]
        if name == dh:
            variant = edited(
                base,
                dh + ".tla",
                "NotSolved == big # 4",
                f"NotSolved == big # {index + 6}",
            )
            expected = ("holds", None)
        elif name == ewd:
            variant = edited(
                base,
                ewd + ".tla",
                "TerminationDetection == terminationDetected => terminated",
                f"TerminationDetection == (terminationDetected => terminated) /\\ N <= {index + 3}",
            )
            expected = gold[name]
        elif name == clock:
            old = "ErrorTemporal == []((now # 4) => <>[](now # 4))"
            value = [1, 2, 3, 5, 6][index]
            variant = edited(
                base, clock + ".tla", old, old.replace(" # 4", f" # {value}")
            )
            expected = ("holds" if value == 6 else "refuted", None)
        else:
            # Shrink one or more finite sets to explore a genuinely different graph.
            variant = base
            for flag, label, a, others in [
                (1, "Proc", "p1", "p2"),
                (2, "Adr", "a1", "a2, a3"),
                (4, "Val", "v1", "v2"),
            ]:
                if (index + 1) & flag:
                    variant = edited(
                        variant,
                        memory + ".cfg",
                        f"{label} = {{{a}, {others}}}",
                        f"{label} = {{{a}}}",
                    )
            expected = ("holds", None)
        result += [
            step("base", name),
            step("repeat-base", name),
            step("edit", name, variant, expected),
            step("repeat-edit", name, variant, expected),
            step("undo", name),
        ]
    return result


@contextmanager
def daemon(binary, root, audit):
    if binary is None:
        yield None, None
        return
    cache = root / "cache"
    cache.mkdir(mode=0o700)
    sock = cache / "sail.sock"
    env = {**os.environ, "SAIL_CACHE_DIR": str(cache), "SAIL_DAEMON_SOCKET": str(sock)}
    with (root / "daemon.log").open("w") as log:
        argv = [binary, "serve", "--socket", sock]
        if audit:
            argv = [
                "strace",
                "-f",
                "-e",
                "trace=execve",
                "-s",
                "256",
                "-o",
                root / "exec.log",
                *argv,
            ]
        server = subprocess.Popen(
            argv, env=env, stdout=log, stderr=log, start_new_session=True
        )
        try:
            end = time.monotonic() + 30
            while not sock.exists():
                if server.poll() is not None or time.monotonic() >= end:
                    raise RuntimeError(
                        f"daemon did not start: {(root / 'daemon.log').read_text()[-1500:]}"
                    )
                time.sleep(0.01)
            yield sock, env
        finally:
            if server.poll() is None:
                os.killpg(server.pid, signal.SIGTERM)
            server.wait(timeout=10)


def timed(argv, cwd, env=None):
    start = time.perf_counter_ns()
    p = subprocess.run(
        argv, cwd=cwd, env=env, capture_output=True, text=True, timeout=310
    )
    return p, (time.perf_counter_ns() - start) / 1e6


def tlc(args, root, envelope):
    model = job(envelope)["request"]["model"]
    with tempfile.TemporaryDirectory(dir=root) as model_dir:
        for name, contents in model["files"].items():
            (Path(model_dir) / name).write_text(contents)
        p, ms = timed(
            [
                args.java,
                "-XX:+UseParallelGC",
                "-jar",
                args.jar,
                "-workers",
                "1",
                "-config",
                model["config"],
                model["root"],
            ],
            model_dir,
        )
    if (
        p.returncode == 0
        and "Model checking completed. No error has been found." in p.stdout
    ):
        outcome = "holds"
    elif p.returncode in (11, 12, 13) and re.search(
        r"Error: (Invariant .* is violated|Deadlock reached|Temporal property .* was violated|Temporal properties were violated)",
        p.stdout,
    ):
        outcome = "refuted"
    else:
        raise ValueError(f"TLC rc={p.returncode}: {p.stdout[-1500:]} {p.stderr[-500:]}")
    counts = re.findall(r"([0-9,]+) distinct states found", p.stdout)
    if not counts:
        raise ValueError("TLC did not return a distinct-state count")
    return dict(ms=ms, outcome=outcome, distinct=int(counts[-1].replace(",", "")))


def sail(args, root, envelope, sock, env):
    file = root / "envelope.json"
    file.write_text(json.dumps(envelope))
    p, ms = timed(
        [args.sail, "runtime", "execute", "--socket", sock, "--file", file], root, env
    )
    if p.returncode != 0:
        raise ValueError(
            f"Sail rc={p.returncode}: {p.stdout[-1000:]} {p.stderr[-500:]}"
        )
    doc = json.loads(p.stdout)
    result = doc.get("output", doc)["result"]
    response = result.get("response") or {}
    if (
        result["outcome"] != "completed"
        or response.get("request_sha256") != result["request_sha256"]
    ):
        raise ValueError(f"incomplete Sail response: {str(result)[-800:]}")
    if (result["obligation_id"], result["requirement_ids"]) != (
        job(envelope)["obligation_id"],
        job(envelope)["requirement_ids"],
    ):
        raise ValueError("Sail returned a different job attribution")
    report = response["native_report"]
    outcome = response["outcome"]
    if outcome == "refuted":
        traces = response["native_traces"]
        validations = response["validations"]
        if (
            not traces
            or len(validations) != len(traces)
            or len(traces) != len(response["itf_traces"])
            or any(v["result"] != "valid" for v in validations)
        ):
            raise ValueError("refutation missing independently validated traces")
    if outcome == "holds":
        if report["verdict"] != "success" or report["statistics"]["left_on_queue"] != 0:
            raise ValueError("incomplete graph for Holds")
        if (
            response["completion"] == "temporal"
            and report["execution"]["capture_completed"] is not True
        ):
            raise ValueError("incomplete graph capture for temporal Holds")
    hit = result.get("cache_hit", False)
    key = result.get("cache_key")
    if hit and not re.fullmatch("[0-9a-f]{64}", key or ""):
        raise ValueError("cache hit without a content key")
    return dict(
        ms=ms,
        outcome=outcome,
        distinct=report["statistics"]["distinct"],
        cache_hit=hit,
        key=key,
        evidence=digest(response),
        request_sha256=result["request_sha256"],
    )


def worker_launches(root):
    return len(
        re.findall(
            r'execve\("[^"]*/sail-runtime-worker"', (root / "exec.log").read_text()
        )
    )


def signed_restore(args):
    cases = [
        row
        for row in sequence(args.fixtures, "audit")
        if row[0] in ("fresh-refutation", "fix")
    ]
    with tempfile.TemporaryDirectory(prefix="s7-signed-") as temp:
        root = Path(temp)
        key = root / "signing-key"
        subprocess.run(
            ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", key], check=True
        )
        trusted = (root / "signing-key.pub").read_text()
        previous = {}
        for side in ["save", "restore"]:
            scratch = root / side
            scratch.mkdir(mode=0o700)
            outputs = scratch / "outputs"
            runner = scratch / "runner"
            runner.mkdir(mode=0o700)
            with daemon(args.sail, scratch, True) as (socket, execution_env):
                env = {
                    **execution_env,
                    "GITHUB_REPOSITORY_ID": "1410434023",
                    "GITHUB_OUTPUT": str(outputs),
                    "RUNNER_TEMP": str(runner),
                    "SAIL_RECEIPT_MODE": side,
                    "SAIL_RECEIPT_PUBLIC_KEY": trusted,
                }

                def invoke(command):
                    return subprocess.run(
                        [sys.executable, "-I", args.receipt_action, command],
                        env=env,
                        check=True,
                        capture_output=True,
                        text=True,
                    )

                invoke("setup")
                stage = runner / "sail-temporal-receipts-v1"
                if side == "restore":
                    for name in ["bundle", "signature"]:
                        shutil.copy2(
                            root / "save/runner/sail-temporal-receipts-v1" / name,
                            stage / name,
                        )
                    # The cache archive preserves the producer's private artifact modes.
                    # A group-writable replacement must be a miss, even if its bytes match.
                    bundle = stage / "bundle"
                    mode = bundle.stat().st_mode & 0o777
                    bundle.chmod(mode | 0o020)
                    refused = invoke("import")
                    if "Temporal cache miss" not in refused.stdout:
                        raise ValueError("accepted group-writable signed input")
                    bundle.chmod(mode)
                    imported = invoke("import")
                    if (
                        "Imported 2 authenticated temporal receipt"
                        not in imported.stdout
                    ):
                        raise RuntimeError(imported.stdout)
                starts = 0
                for case, model, envelope, expected in cases:
                    result = sail(args, scratch, envelope, socket, execution_env)
                    count = worker_launches(scratch)
                    if result["outcome"] != expected[0] or result["cache_hit"] != (
                        side == "restore"
                    ):
                        raise ValueError((case, side, result))
                    if count - starts != int(side == "save"):
                        raise ValueError((side, case, "worker-starts", count - starts))
                    if side == "save":
                        previous[case] = result["evidence"]
                    elif result["evidence"] != previous[case]:
                        raise ValueError("restored evidence changed")
                    print(
                        json.dumps(
                            dict(
                                side=side,
                                case=case,
                                worker_launches=count - starts,
                                **result,
                            )
                        ),
                        flush=True,
                    )
                    starts = count
                if side == "restore":
                    original = cases[0][2]
                    retagged = deepcopy(original)
                    job(retagged)["obligation_id"] = "perf/ci-new-caller"
                    job(retagged)["requirement_ids"] = ["CI-CONSUMER"]
                    changed = deepcopy(original)
                    job(changed)["wall_time_ms"] -= 1
                    for case, envelope, hit in [
                        ("new-caller", retagged, True),
                        ("changed-limit", changed, False),
                    ]:
                        result = sail(args, scratch, envelope, socket, execution_env)
                        count = worker_launches(scratch)
                        if (
                            result["outcome"] != "refuted"
                            or result["cache_hit"] != hit
                            or count - starts != int(not hit)
                        ):
                            raise ValueError(
                                f"unexpected result from signed cache: {case}"
                            )
                        if hit and result["evidence"] != previous["fresh-refutation"]:
                            raise ValueError(
                                "reattributed receipt changed proof evidence"
                            )
                        print(
                            json.dumps(
                                dict(
                                    side=side,
                                    case=case,
                                    worker_launches=count - starts,
                                    **result,
                                )
                            ),
                            flush=True,
                        )
                        starts = count
                if side == "save":
                    env["SAIL_RECEIPT_SECRET"] = key.read_text()
                    invoke("export")
                    if "present=true" not in outputs.read_text():
                        raise ValueError("empty export")


def elapsed(start):
    return (time.monotonic_ns() - start) / 1_000_000


def prepare(args):
    first_job = (args.fixtures / "MCLiveInternalMemory" / "envelope.json").resolve(
        strict=True
    )
    source = importlib.util.spec_from_file_location("installer", args.installer)
    installer = importlib.util.module_from_spec(source)
    source.loader.exec_module(installer)
    binaries = args.sail.parent
    rows = []
    for round_number in range(5):
        modes = ["disabled", "worker", "first-job"]
        for mode in modes[round_number % 3 :] + modes[: round_number % 3]:
            with tempfile.TemporaryDirectory(prefix="sail-post-promotion-") as temp:
                root = Path(temp)
                (root / "bin").symlink_to(binaries, target_is_directory=True)
                os.environ.update(
                    SAIL_CACHE_DIR=str(root / "cache"),
                    HOME=str(root),
                    XDG_CACHE_HOME=str(root / "xdg"),
                    SAIL_BACKGROUND_PREPARE="0" if mode == "disabled" else "1",
                )
                start = time.monotonic_ns()
                child = installer.prepare_temporal(
                    root, first_job if mode == "first-job" else None
                )
                handoff = elapsed(start)
                if mode == "disabled":
                    if child is not None:
                        raise ValueError("unexpected preparation job")
                else:
                    if child is None or child.wait(timeout=315) != 0:
                        raise ValueError(f"preparation failed: {mode}")
                ready = elapsed(start)
                started = time.monotonic_ns()
                command = [binaries / "sail", "runtime", "execute", "--file", first_job]
                run = subprocess.run(command, capture_output=True, timeout=315)
                wall = elapsed(started)
                run.check_returncode()
                result = json.loads(run.stdout)["output"]["result"]
                if (
                    result["outcome"] != "completed"
                    or result["response"]["outcome"] != "holds"
                ):
                    raise ValueError(f"incorrect preparation or first check: {mode}")
                if (
                    result["response"]["native_report"]["statistics"]["distinct"]
                    != 4408
                ):
                    raise ValueError(
                        "unexpected MCLiveInternalMemory distinct-state count"
                    )
                if result["cache_hit"] != (mode == "first-job"):
                    raise ValueError(f"incorrect first-check reuse: {mode}")
                row = {
                    "round": round_number + 1,
                    "mode": mode,
                    "schedule_ms": handoff,
                    "ready_since_schedule_start_ms": ready,
                    "first_check_ms": wall,
                    "cache_hit": result["cache_hit"],
                    "outcome": result["response"]["outcome"],
                    "request_sha256": result["request_sha256"],
                }
                rows.append(row)
                print(json.dumps(row), flush=True)
    if len({(r["request_sha256"], r["outcome"]) for r in rows}) != 1:
        raise ValueError("prepared requests or outcomes differ")
    for mode in modes:
        group = [r for r in rows if r["mode"] == mode]
        print(
            json.dumps(
                {
                    "mode": mode,
                    "median_ms": {
                        k: statistics.median(r[k] for r in group)
                        for k in (
                            "schedule_ms",
                            "ready_since_schedule_start_ms",
                            "first_check_ms",
                        )
                    },
                }
            )
        )


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("fixtures", type=Path)
    cli.add_argument(
        "--suite",
        choices=["fixtures", "scenarios", "loop", "audit", "signed-restore", "prepare"],
        required=True,
    )
    cli.add_argument("--sail", type=Path)
    cli.add_argument("--java", type=Path)
    cli.add_argument("--jar", type=Path)
    cli.add_argument("--expect-replay", action="store_true")
    cli.add_argument("--dive", type=Path)
    cli.add_argument("--examples", type=Path)
    cli.add_argument("--receipt-action", type=Path)
    cli.add_argument("--installer", type=Path)
    args = cli.parse_args()
    if args.suite == "fixtures":
        fixtures(args)
        return
    if bool(args.sail) == bool(args.java and args.jar) or (
        args.expect_replay and not args.sail
    ):
        cli.error("select one installed Sail binary or the pinned Java and TLC jar")
    if args.suite in ("audit", "signed-restore") and not args.sail:
        cli.error("the process-launch audit requires Sail and Linux strace")
    for option in ["sail", "java", "jar"]:
        value = getattr(args, option)
        if value is not None:
            setattr(args, option, value.resolve(strict=True))
    if args.suite == "prepare":
        if not args.installer or not args.sail:
            cli.error("prepare requires --sail and --installer scripts/install.py")
        prepare(args)
        return
    if args.suite == "signed-restore":
        if not args.receipt_action:
            cli.error(
                "signed-restore requires --receipt-action action/temporal-cache/receipts.py"
            )
        args.receipt_action = args.receipt_action.resolve(strict=True)
        signed_restore(args)
        return
    seen = {}
    keys = {}
    content_keys = {}
    with tempfile.TemporaryDirectory(prefix="s7-reuse-") as scratch:
        root = Path(scratch)
        audit = args.suite == "audit"
        launches = 0
        with daemon(args.sail, root, audit) as (sock, env):
            for i, (label, name, envelope, (expected, distinct)) in enumerate(
                sequence(args.fixtures, args.suite)
            ):
                row = (
                    sail(args, root, envelope, sock, env)
                    if args.sail
                    else tlc(args, root, envelope)
                )
                if (
                    row["outcome"] != expected
                    or distinct is not None
                    and distinct != row["distinct"]
                ):
                    raise ValueError(
                        f"{label} {name}: expected {expected}/{distinct}, got {row}"
                    )
                content = digest(job(envelope)["request"])
                if args.sail:
                    hit = row["cache_hit"]
                    if hit or args.expect_replay:
                        if hit != (content in seen):
                            raise ValueError(f"{label}: wrong hit state for this job")
                    if hit:
                        if (
                            row["evidence"] != seen[content]
                            or keys.setdefault(row["key"], content) != content
                            or content_keys.setdefault(content, row["key"])
                            != row["key"]
                        ):
                            raise ValueError(
                                "replayed evidence changed or cache key collision"
                            )
                    seen[content] = row["evidence"]
                if audit:
                    starts = worker_launches(root)
                    row["worker_launches"] = starts - launches
                    if row["worker_launches"] != (0 if row["cache_hit"] else 1):
                        raise ValueError(
                            f"unexpected worker launches in {label}: {row['worker_launches']}"
                        )
                    launches = starts
                print(
                    json.dumps(
                        dict(index=i, label=label, model=name, content=content, **row)
                    ),
                    flush=True,
                )


if __name__ == "__main__":
    main()
