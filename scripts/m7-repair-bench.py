#!/usr/bin/env python3
"""Local-only M6/M7 repair acceptance over the frozen real-project catalogue.

Uses only Python's standard library. Retains raw results and never substitutes
a timed-out or incomplete invocation for a verdict. See docs/bench/m7-repair-acceptance.md.
"""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import signal
import statistics
import subprocess
import time

REPO = Path(__file__).resolve().parent.parent
CATALOGUE = REPO / "corpus/repair-edits.json"
FROZEN_HASH = "6ffc129735220bf53511ac6bba818c5c3c7b61bca92d59c17292a807979b18d6"
spec = importlib.util.spec_from_file_location("m7_job_protocol", REPO / "scripts/m7-job-read-bench.py")
protocol = importlib.util.module_from_spec(spec)
spec.loader.exec_module(protocol)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def catalogue():
    if sha(CATALOGUE) != FROZEN_HASH:
        raise ValueError("SAIL0940: frozen repair catalogue changed")
    return json.loads(CATALOGUE.read_text())


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def checked(args, cwd=None, env=None):
    result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, text=True)
    if result.returncode:
        raise ValueError("SAIL0940: command failed: " + repr(args) + "\n" + result.stderr)
    return result.stdout


def source_tree(cache, project):
    root = cache / "corpus" / (project["id"] + "-" + project["commit"][:12])
    actual = checked(["git", "-C", str(root), "rev-parse", "HEAD"]).strip()
    if actual != project["commit"]:
        raise ValueError("SAIL0940: corpus checkout has wrong commit: " + str(root))
    return root


def prepare(args):
    data = catalogue()
    source = args.verus_source.resolve()
    if checked(["git", "-C", str(source), "rev-parse", "HEAD"]).strip() != data["verus_commit"]:
        raise ValueError("SAIL0940: Verus source has wrong commit")
    vstd = source / "source/vstd"
    version = re.search(r'^version = "([^"\n]+)"', (vstd / "Cargo.toml").read_text(), re.M).group(1)
    for project in data["projects"]:
        fetched = source_tree(args.cache.resolve(), project)
        dest = args.scratch.resolve() / project["id"]
        if dest.exists():
            raise ValueError("SAIL0940: preparation destination already exists: " + str(dest))
        # Clone committed objects, never copy a prior experiment's worktree edits.
        dest.parent.mkdir(parents=True, exist_ok=True)
        checked(["git", "clone", "--local", "--no-hardlinks", "--quiet", str(fetched), str(dest)])
        checked(["git", "-C", str(dest), "checkout", "--quiet", "--detach", project["commit"]])
        if project["cargo_wrapper"]:
            (dest / "Cargo.toml").write_text(
                '[package]\nname = "human_eval_verus"\nversion = "0.0.0"\nedition = "2021"\n'
                '[lib]\npath = "tasks/lib.rs"\n[package.metadata.verus]\nverify = true\n'
                '[workspace]\n[dependencies]\nvstd = { path = ' + json.dumps(str(vstd)) + ' }\n'
            )
        else:
            manifest = dest / "Cargo.toml"
            text = manifest.read_text()
            text, count = re.subn(
                r'(vstd = \{ version = ")=[^"\n]+', r'\g<1>=' + version, text
            )
            if count != 1 or "[patch.crates-io]" in text:
                raise ValueError("SAIL0940: unexpected Vest dependency manifest")
            text += '\n[patch.crates-io]\nvstd = { path = ' + json.dumps(str(vstd)) + ' }\n'
            manifest.write_text(text)
        env = dict(os.environ, RUSTUP_TOOLCHAIN="1.98.1")
        checked(["cargo", "metadata", "--format-version", "1"], cwd=dest, env=env)
        originals = {}
        versions = {}
        for cycle in project["cycles"]:
            name = cycle["file"]
            originals.setdefault(name, (dest / name).read_text())
            before = originals[name] if cycle["from"] == "original" else versions[cycle["from"]]
            if before.count(cycle["find"]) != 1:
                raise ValueError("SAIL0940: edit does not match exactly once: " + cycle["id"])
            versions[cycle["id"]] = before.replace(cycle["find"], cycle["replace"])
        write_json(dest / ".sail-m7-workload.json", {
            "catalogue_sha256": FROZEN_HASH, "project": project["id"],
            "commit": project["commit"], "originals": originals, "versions": versions,
        })
        print("prepared " + str(dest), flush=True)


def actionable_event(event):
    event = event.get("input", event)
    return event.get("phase") == "focus" and event.get("observation", {}).get("kind") in (
        "diagnostic", "function_observed", "obligation_observed", "phase_finished"
    )


class ProofObserver:
    """Observe the public journal; the daemon owns its private producer path."""
    def __init__(self, env):
        self.address = protocol.endpoint(Path(env["SAIL_DAEMON_SOCKET"]))
        self.job = None
        self.cursor = {"stream_id": "", "after": 0}
        self.reads = 0
        self.next_read = 0

    def poll(self, stderr, directory):
        now = time.monotonic()
        if now < self.next_read:
            return False
        self.next_read = now + 0.1
        if self.job is None:
            match = re.search(r"^Repair validation job ([^\s]+)$", stderr.read_text(errors="replace"), re.M)
            if match is None:
                return False
            self.job = match.group(1)
            self.cursor["stream_id"] = self.job
        response, wall, _ = protocol.call(self.address, {
            "operation": "proof_events", "id": self.job, "cursor": self.cursor, "limit": 64,
        })
        if response["response"] != "proof_events":
            raise ValueError("SAIL0941: unexpected proof observation response")
        page = response["value"]
        self.reads += 1
        if page["events"] or page.get("gap") or page["terminal"] or self.reads == 1:
            write_json(directory / ("proof-page-%06d.json" % self.reads),
                       {"read_ms": wall, "page": page})
        self.cursor = page["next"]
        return any(actionable_event(event) for event in page["events"])


def invoke(command, workspace, env, directory, timeout):
    directory.mkdir(parents=True, exist_ok=False)
    observer = ProofObserver(env) if command[1] == "repair-check" else None
    timer = Path("/usr/bin/time")
    timed = platform.system() == "Linux" and timer.is_file()
    args = [str(timer), "-v", "-o", str(directory / "time.txt")] + command if timed else command
    started = time.monotonic()
    actionable = None
    with (directory / "stdout.json").open("wb") as out, (directory / "stderr.txt").open("wb") as err:
        child = subprocess.Popen(args, cwd=workspace, env=env, stdout=out, stderr=err, start_new_session=True)
        try:
            while child.poll() is None:
                elapsed = time.monotonic() - started
                if elapsed > timeout:
                    # Foreground repair owns a daemon job in another process
                    # group. Let its interrupt handler cancel that owned job
                    # before forcibly ending an unresponsive CLI process.
                    os.killpg(child.pid, signal.SIGINT)
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(child.pid, signal.SIGKILL)
                        child.wait()
                    write_json(directory / "timeout.json", {"timed_out": True, "deadline_s": timeout,
                                                            "elapsed_s": time.monotonic() - started,
                                                            "exit_code": child.returncode})
                    raise ValueError("SAIL0941: repair workload timed out: " + str(directory))
                if actionable is None and observer is not None and observer.poll(directory / "stderr.txt", directory):
                    actionable = (time.monotonic() - started) * 1000
                time.sleep(0.01)
        except BaseException:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            raise
    wall = (time.monotonic() - started) * 1000
    report = json.loads((directory / "stdout.json").read_text())
    rss = None
    if timed:
        match = re.search(r"Maximum resident set size \(kbytes\): (\d+)", (directory / "time.txt").read_text())
        if match is None:
            raise ValueError("SAIL0941: missing GNU time RSS")
        rss = int(match.group(1)) * 1024
    return {"wall_ms": wall, "first_actionable_ms": actionable, "max_rss_bytes": rss,
            "proof_reads": observer.reads if observer is not None else None,
            "exit_code": child.returncode, "report": report}


def selected_report(project):
    if project.get("error") or project.get("exit_code") is None:
        raise ValueError("SAIL0942: invocation produced an infrastructure error")
    targets = [target for target in project["targets"] if target["requested"]]
    if len(targets) != 1:
        raise ValueError("SAIL0942: benchmark must select exactly one target")
    report = targets[0]["report"]
    if report.get("error"):
        raise ValueError("SAIL0942: selected target has an infrastructure error")
    functions = sorted((function["function"], function["verdict"], function["tier"])
                       for crate in report["crates"] for function in crate["functions"])
    # Diagnostics compare in original order, including verbatim text and spans.
    return {"exit_code": project["exit_code"], "functions": functions,
            "summary": report["summary"], "diagnostics": report["diagnostics"]}


def warm_repair_state(doc, allow_setup):
    """Only an explicit first-run declaration transition permits one retry."""
    batch = doc.get("batch", {})
    full = batch.get("full", {})
    if not batch.get("batched") or not batch.get("manifest_sha256") or batch.get("focus") is None:
        raise ValueError("SAIL0942: repair warmup did not use the real source engine")
    if doc.get("selection_error") or full.get("error") or full.get("exit_code") != 0:
        raise ValueError("SAIL0942: original repair warmup did not verify")
    targets = [target for target in full.get("targets", []) if target.get("requested")]
    if len(targets) != 1 or not targets[0].get("complete"):
        raise ValueError("SAIL0942: original repair warmup has incomplete full evidence")
    if doc.get("accepted") is True and full.get("currentness") == "current":
        return "ready"
    if (allow_setup and doc.get("accepted") is False
            and full.get("currentness") in ("unknown", "historical")
            and "Build-script input declarations were established during this run" in full.get("notes", [])):
        return "declarations-established"
    raise ValueError("SAIL0942: repair warmup lacks Current accepted full evidence")


def checked_preparation(batch):
    records = batch.get("preparation")
    if not records or not any(record.get("enabled") is True for record in records):
        raise ValueError("SAIL0942: final engine preparation telemetry is unavailable")
    budget = 512 * 1024 * 1024
    for record in records:
        if (record.get("version") != 1
                or not 0 < record["memory_budget_bytes"] <= budget
                or not 0 < record["disk_budget_bytes"] <= budget
                or not 0 <= record["peak_retained_bytes"] <= record["memory_budget_bytes"]):
            raise ValueError("SAIL0942: preparation observation exceeds the accepted budget")
    return records


def distribution(values):
    observed = [value for value in values if value is not None]
    return {"observed": len(observed), "median": statistics.median(observed) if observed else None,
            "p95": protocol.p95(observed) if observed else None,
            "max": max(observed) if observed else None}


def run(args):
    data = catalogue()
    if args.rounds < data["minimum_rounds"]:
        raise ValueError("SAIL0940: acceptance requires at least three rounds")
    binaries = {"m6": args.m6.resolve(), "m7": args.m7.resolve()}
    result = {"schema_version": 1, "catalogue_sha256": FROZEN_HASH,
              "baseline_commit": data["baseline_commit"], "platform": platform.platform(),
              "execution_environment": args.environment, "binary_sha256": {k: sha(v) for k, v in binaries.items()},
              "rounds": args.rounds, "warmups": [], "samples": [], "acceptance": {"complete": False}}
    args.out.mkdir(parents=True, exist_ok=False)
    write_json(args.out / "results.json", result)
    for project in data["projects"]:
        workspace = (args.scratch / project["id"]).resolve()
        frozen = json.loads((workspace / ".sail-m7-workload.json").read_text())
        if frozen["catalogue_sha256"] != FROZEN_HASH or frozen["commit"] != project["commit"]:
            raise ValueError("SAIL0940: prepared workspace has incompatible identity")
        selection = ["--package", project["package"]]
        # M6 injects --target-dir after user args; pinned cargo-verus refuses
        # that after --lib. These frozen packages contain a single library.
        metadata = json.loads(checked(["cargo", "metadata", "--no-deps", "--format-version", "1"],
                                      cwd=workspace, env=dict(os.environ, RUSTUP_TOOLCHAIN="1.98.1")))
        packages = [package for package in metadata["packages"] if package["name"] == project["package"]]
        if len(packages) != 1 or len(packages[0]["targets"]) != 1 or packages[0]["targets"][0]["kind"] != ["lib"]:
            raise ValueError("SAIL0940: baseline package selection no longer identifies only the frozen library")
        envs = {}
        for mode in binaries:
            cache = (args.out / ("cache-" + mode)).resolve()
            cache.mkdir(exist_ok=True)
            socket = "/tmp/sail-m7-" + hashlib.sha256(str(cache).encode()).hexdigest()[:12] + ".sock"
            envs[mode] = dict(os.environ, SAIL_CACHE_DIR=str(cache), SAIL_DAEMON_SOCKET=socket,
                              RUSTUP_TOOLCHAIN="1.98.1", SAIL_MANAGED_FORCE="1")
            if mode == "m7":
                install = json.loads(checked([str(binaries[mode]), "engine", "install", "--from",
                                             str(args.engine_install.resolve()), "--output", "json"],
                                            env=envs[mode]))
                write_json(args.out / "engine-install.json", install)
            # Dependency build and complete SMT-cache fill are not measured.
            warm = invoke([str(binaries[mode]), "verify", "--incremental", "--output", "json"] + selection,
                          workspace, envs[mode], (args.out / project["id"] / ("warm-" + mode)).resolve(), args.timeout)
            if selected_report(warm["report"])["exit_code"] != 0:
                raise ValueError("SAIL0942: original project did not verify during warmup")
            result["warmups"].append({"project": project["id"], "mode": mode,
                                      "kind": "dependencies-and-solver", "wall_ms": warm["wall_ms"]})
            write_json(args.out / "results.json", result)
        # Repair selection is part of context identity. A stock verify warmup
        # cannot establish the build/source declarations for those contexts.
        for index, function in enumerate(dict.fromkeys(cycle["function"] for cycle in project["cycles"])):
            # M6's focused scope also has its own context. Give both modes
            # the same original-source setup before any measured edits.
            relative = function.split("::", 1)[1]
            module, leaf = relative.rsplit("::", 1)
            warm = invoke([str(binaries["m6"]), "verify", "--incremental", "--output", "json"]
                          + selection + ["--", "--verify-only-module", module, "--verify-function", leaf],
                          workspace, envs["m6"],
                          (args.out / project["id"] / ("warm-focus-%d" % index)).resolve(), args.timeout)
            if selected_report(warm["report"])["exit_code"] != 0:
                raise ValueError("SAIL0942: original M6 focused warmup did not verify")
            result["warmups"].append({"project": project["id"], "mode": "m6", "kind": "focused-context",
                                      "function": function, "wall_ms": warm["wall_ms"]})
            write_json(args.out / "results.json", result)
            for attempt in range(2):
                warm = invoke([str(binaries["m7"]), "repair-check", "--engine", "interactive",
                               "--function", function, "--output", "json"] + selection + ["--lib"],
                              workspace, envs["m7"],
                              (args.out / project["id"] / ("warm-repair-%d-%d" % (index, attempt))).resolve(),
                              args.timeout)
                state = warm_repair_state(warm["report"], allow_setup=attempt == 0)
                result["warmups"].append({"project": project["id"], "mode": "m7", "kind": "repair-context",
                                          "function": function, "attempt": attempt, "state": state,
                                          "wall_ms": warm["wall_ms"]})
                write_json(args.out / "results.json", result)
                if state == "ready":
                    break
        for round_index in range(args.rounds):
            for cycle in project["cycles"]:
                # Preserve identical paths and cumulative failure→repair content.
                for name, text in frozen["originals"].items():
                    (workspace / name).write_text(text)
                (workspace / cycle["file"]).write_text(frozen["versions"][cycle["id"]])
                pair = {"project": project["id"], "cycle": cycle["id"], "kind": cycle["kind"],
                        "round": round_index, "expected": cycle["expected"],
                        "edited_sha256": sha(workspace / cycle["file"])}
                for mode in (["m6", "m7"] if round_index % 2 == 0 else ["m7", "m6"]):
                    directory = (args.out / project["id"] / str(round_index) / cycle["id"] / mode).resolve()
                    if mode == "m6":
                        common = [str(binaries[mode]), "verify", "--incremental", "--output", "json"] + selection
                        relative = cycle["function"].split("::", 1)[1]
                        module, function = relative.rsplit("::", 1)
                        focus = invoke(common + ["--", "--verify-only-module", module, "--verify-function", function], workspace,
                                       envs[mode], directory / "focus", args.timeout)
                        full = invoke(common, workspace, envs[mode], directory / "full", args.timeout)
                        if full["report"]["currentness"] != "current":
                            raise ValueError("SAIL0942: measured baseline full evidence is not Current")
                        pair[mode] = {"wall_ms": focus["wall_ms"] + full["wall_ms"],
                                      "first_actionable_ms": focus["wall_ms"],
                                      "max_rss_bytes": max(focus["max_rss_bytes"] or 0, full["max_rss_bytes"] or 0),
                                      "full": selected_report(full["report"])}
                    else:
                        value = invoke([str(binaries[mode]), "repair-check", "--engine", "interactive",
                                        "--function", cycle["function"], "--output", "json"] + selection + ["--lib"],
                                       workspace, envs[mode], directory, args.timeout)
                        doc = value["report"]
                        batch = doc["batch"]
                        if not batch["batched"] or not batch["manifest_sha256"] or batch["focus"] is None:
                            raise ValueError("SAIL0942: invocation did not produce a real source-engine batch")
                        if batch["full"]["currentness"] != "current":
                            raise ValueError("SAIL0942: measured full evidence is not Current")
                        if batch["compiler_invocations"] != 1:
                            raise ValueError("SAIL0942: established repair did not use one compiler driver")
                        checked_preparation(batch)
                        pair[mode] = {key: value[key] for key in ("wall_ms", "first_actionable_ms", "max_rss_bytes")}
                        pair[mode].update(full=selected_report(batch["full"]),
                                          compiler_invocations=batch["compiler_invocations"], accepted=doc["accepted"],
                                          proof_reads=value["proof_reads"],
                                          batch_metadata={key: value for key, value in batch.items()
                                                          if key not in ("focus", "full")})
                pair["same"] = pair["m6"]["full"] == pair["m7"]["full"]
                pair["expected_observed"] = ((pair["m6"]["full"]["exit_code"] == 0) == (cycle["expected"] == "verified"))
                if pair["m7"]["accepted"] != (cycle["expected"] == "verified"):
                    pair["expected_observed"] = False
                result["samples"].append(pair)
                write_json(args.out / "results.json", result)
                print(project["id"], cycle["id"], round_index, "same=" + str(pair["same"]), flush=True)
    result["acceptance"] = summarize(result)
    write_json(args.out / "results.json", result)
    print(json.dumps(result["acceptance"], indent=2))
    return 0 if result["acceptance"]["passed"] else 1


def summarize(result):
    data = catalogue()
    summaries = {}
    complete = result.get("rounds", 0) >= data["minimum_rounds"]
    for project in data["projects"]:
        samples = [sample for sample in result["samples"] if sample["project"] == project["id"]]
        keys = {(sample["round"], sample["cycle"]) for sample in samples}
        expected = {(round_index, cycle["id"]) for round_index in range(result.get("rounds", 0))
                    for cycle in project["cycles"]}
        covered = keys == expected and len(keys) == len(samples) and bool(expected)
        complete = complete and covered
        totals = {mode: [sum(sample[mode]["wall_ms"] for sample in samples if sample["round"] == index)
                         for index in range(result.get("rounds", 0))] for mode in ("m6", "m7")}
        denominator = statistics.median(totals["m7"]) if totals["m7"] else 0
        speedup = statistics.median(totals["m6"]) / denominator if denominator > 0 else None
        summaries[project["id"]] = {
            "covered": covered, "speedup": speedup,
            "round_totals_ms": totals,
            "spread_ms": {mode: [min(values), max(values)] if values else None for mode, values in totals.items()},
            "same": all(sample.get("same") and sample.get("expected_observed") for sample in samples) and bool(samples),
            "cycle_distributions": {
                mode: {metric: distribution([sample[mode].get(metric) for sample in samples])
                       for metric in ("wall_ms", "first_actionable_ms", "max_rss_bytes")}
                for mode in ("m6", "m7")
            },
        }
    passed = complete and all(value["same"] and value["speedup"] is not None
                              and value["speedup"] >= data["minimum_median_speedup"] for value in summaries.values())
    return {"complete": complete, "passed": passed, "projects": summaries,
            "responsiveness_measured": False, "note": "Speed/correctness catalogue gate only; job-read latency is independently required."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prep = commands.add_parser("prepare")
    prep.add_argument("--cache", type=Path, required=True)
    prep.add_argument("--verus-source", type=Path, required=True)
    prep.add_argument("--scratch", type=Path, required=True)
    runner = commands.add_parser("run")
    runner.add_argument("--m6", type=Path, required=True)
    runner.add_argument("--m7", type=Path, required=True)
    runner.add_argument("--engine-install", type=Path, required=True,
                        help="Validated Linux source variant; installed into the private M7 cache")
    runner.add_argument("--scratch", type=Path, required=True)
    runner.add_argument("--out", type=Path, required=True)
    runner.add_argument("--rounds", type=int, default=3)
    runner.add_argument("--timeout", type=float, default=600)
    runner.add_argument("--environment", required=True, help="Actual native/emulated execution environment")
    report = commands.add_parser("report")
    report.add_argument("results", type=Path)
    args = parser.parse_args()
    if os.environ.get("CI"):
        raise ValueError("SAIL0940: long repair acceptance is local-only")
    if args.command == "prepare":
        prepare(args)
    elif args.command == "run":
        return run(args)
    else:
        print(json.dumps(summarize(json.loads(args.results.read_text())), indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, KeyError, OSError) as error:
        raise SystemExit("error[SAIL0940]: " + str(error))
