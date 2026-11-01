#!/usr/bin/env python3
"""Run the broader local corpus through existing comparators and benchmarks.

This orchestrator never decides that two verifier outputs are equivalent.
Conformance and direct-unit benchmarks use Sail's existing Rust comparator.
Managed timing tables remain paired with their separate conformance report.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import time
import tomllib


ROOT = Path(__file__).resolve().parent.parent
CATALOGUE = ROOT / "corpus/performance-suite.json"


def workloads():
    value = json.loads(CATALOGUE.read_text())
    if value["schema_version"] != 1:
        raise ValueError("unknown performance suite schema")
    entries = value["workloads"]
    ids = [entry["id"] for entry in entries]
    if len(ids) != len(set(ids)):
        raise ValueError("duplicate workload id")
    return entries


def manifest(entry):
    return ROOT / ("corpus/performance-candidates.toml" if entry.get("candidate")
                   else "corpus/corpus.toml")


def selection(entry):
    args = ["--corpus", str(manifest(entry)), "--tier", "deep",
            "--project", entry["project"]]
    if entry.get("filter"):
        args += ["--filter", entry["filter"]]
    return args


def file_digests(paths):
    return {str(path): hashlib.sha256(Path(path).read_bytes()).hexdigest() for path in paths}


def inputs_intact(expected):
    try:
        return file_digests(expected) == expected
    except OSError:
        return False


def git_output(*args):
    result = subprocess.run(["git", *args], cwd=ROOT, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, check=False)
    if result.returncode or result.stderr:
        raise ValueError("incomplete Git source inventory: " +
                         result.stderr.decode(errors="replace").strip())
    return result.stdout


def source_state():
    tracked_diff = git_output("diff", "--binary", "HEAD", "--")
    return {"tracked_diff_sha256": hashlib.sha256(tracked_diff).hexdigest(),
            "head": git_output("rev-parse", "HEAD").decode().strip(),
            "status": git_output("status", "--porcelain", "--untracked-files=all").decode()}


def execute(argv, output, env):
    started = time.time()
    with output.open("w") as log:
        try:
            code = subprocess.run(argv, cwd=ROOT, env=env, stdout=log,
                                  stderr=subprocess.STDOUT, check=False).returncode
        except OSError as error:
            log.write(str(error) + "\n")
            code = 127
    return {"argv": argv, "exit": code, "started_unix": started,
            "elapsed_seconds": time.time() - started,
            "log_sha256": hashlib.sha256(output.read_bytes()).hexdigest()}


def conformance_status(path):
    report = json.loads(path.read_text())
    summary = report["summary"]
    complete = (summary["units"] > 0 and summary["same"] == summary["units"]
                and summary["projects_skipped"] == 0
                and all(summary.get(key, 0) == 0 for key in
                        ("unstable", "different", "infra_error", "engine_lacks_feature")))
    passing = (complete and all(unit.get("plain") and unit["plain"]["exit_code"] == 0
                                and unit["plain"].get("verified", 0)
                                and unit["plain"].get("errors") == 0
                                for unit in report["units"]))
    return summary, complete, bool(passing)


def benchmark_complete(path, workload, rounds):
    report = json.loads(path.read_text())
    units = report["units"]
    expected = rounds + (workload == "warm")
    return bool(units) and all(
        unit["same"] == expected
        and all(unit.get(key, 0) == 0 for key in ("different", "unstable", "infra_errors"))
        and len(unit["plain"]["wall_ms"]) == expected
        and len(unit["sail"]["wall_ms"]) == rounds
        for unit in units)


def managed_complete(path, rounds):
    groups = {}
    for line in path.read_text().splitlines():
        row = line.split("\t")
        if len(row) != 22 or row[4] != "0" or not 0 < float(row[3]) < 1e9:
            return False
        key = (row[1], row[2])
        groups.setdefault(key, []).append(int(row[0]))
    required = {"cold", "unchanged", "unchanged2", "unchanged-noreplay", "noop1"}
    return (all((mode, phase) in groups for mode in ("plain", "sail") for phase in required)
            and all(sorted(values) == list(range(1, rounds + 1)) for values in groups.values()))


def run(args, entries):
    chosen = [entry for entry in entries if not args.project or entry["id"] in args.project]
    unknown = set(args.project) - {entry["id"] for entry in entries}
    if unknown or not chosen:
        raise ValueError("unknown or empty workload selection: " + ", ".join(sorted(unknown)))
    if any(entry.get("held_out") for entry in chosen) and args.phase != "intake":
        if not args.frozen_candidate:
            raise ValueError("held-out timing needs --frozen-candidate COMMIT")
    if args.phase != "intake" and not args.sail_verus_build:
        raise ValueError("timing needs --sail-verus-build for the validated Sail fork")
    if args.phase != "intake" and not args.machine:
        raise ValueError("timing needs --machine with the instance type and region")
    binaries = {}
    for name in ("sail", "sail-z3", "sail-conformance", "sail-bench"):
        path = shutil.which(name)
        if not path:
            raise ValueError(name + " is not on PATH")
        binaries[name] = {"path": path,
                          "sha256": hashlib.sha256(Path(path).read_bytes()).hexdigest()}
    source = source_state()
    if args.frozen_candidate and (args.frozen_candidate != source["head"] or source["status"]):
        raise ValueError("frozen candidate must equal the full clean source HEAD")
    input_paths = [Path(value["path"]) for value in binaries.values()]
    input_paths += [CATALOGUE, ROOT / "scripts/perf-corpus.py", ROOT / "scripts/perf-real.sh"]
    input_paths += [ROOT / "corpus" / name for name in
                    ("corpus.toml", "performance-candidates.toml", "toolchain.toml")]
    if args.sail_verus_build:
        input_paths.append(args.sail_verus_build / "sail-build.json")
    inputs = file_digests(input_paths)
    args.out.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(CATALOGUE, args.out / CATALOGUE.name)
    for name in ("corpus.toml", "performance-candidates.toml", "toolchain.toml"):
        shutil.copyfile(ROOT / "corpus" / name, args.out / name)
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("SAIL_") or key in ("SAIL_VERUS_HOME", "SAIL_CACHE_DIR")}
    pins = tomllib.loads((ROOT / "corpus/toolchain.toml").read_text())
    # Each managed arm supplies a fresh cache. Keep an inherited selector out
    # of Cargo's compiler environment and every timing arm's replay identity.
    results = []
    for entry in chosen:
        directory = args.out / entry["id"]
        directory.mkdir()
        print(entry["id"], flush=True)
        record = {"id": entry["id"], "phase": args.phase,
                  "managed_cargo_toolchain": entry.get("toolchain", pins["verus"]["rust_toolchain"]),
                  "held_out": entry.get("held_out", False), "commands": []}
        command = ["sail-conformance", "run", *selection(entry), "--jobs", "1",
                   "--shim", binaries["sail-z3"]["path"],
                   "--out", str(directory / "conformance")]
        if args.phase == "intake":
            command += ["--sail-verus", "stock", "--sail-arm", "shim"]
        else:
            command += ["--engine-build", str(args.sail_verus_build)]
        outcome = execute(command, directory / "conformance.log", env)
        record["commands"].append(outcome)
        report = directory / "conformance/report.json"
        record["comparator_passed"] = False
        record["passing_plain_baseline"] = False
        if report.exists():
            try:
                (record["conformance"], record["comparator_passed"],
                 record["passing_plain_baseline"]) = conformance_status(report)
            except (OSError, ValueError, KeyError) as error:
                record["report_refusal"] = str(error)
        # A matching failed plain run is conformance evidence, not a passing
        # project baseline. The report and its full per-side outcomes remain.
        record["status"] = "conformance-failed"
        if record["comparator_passed"] and outcome["exit"] == 0:
            record["status"] = "intake-comparison-complete"
            if entry.get("candidate") and not record["passing_plain_baseline"]:
                record["status"] = "candidate-plain-baseline-failed"
            elif args.phase != "intake":
                rounds = "6" if args.phase == "accept" else "2"
                if entry.get("managed"):
                    timing_env = dict(env, SAIL_PERF_OUT=str(directory / "managed"),
                                      SAIL_PERF_MODES="plain sail", SAIL_PERF_VSTD="registry",
                                      SAIL_PERF_VERUS_TIMES="0", SAIL_PERF_EDITS="none",
                                      SAIL_PERF_UNCHANGED="2", SAIL_PERF_NOREPLAY="1",
                                      SAIL_PERF_NOOPS="1", SAIL_PERF_MEMORY="0",
                                      SAIL_PERF_VERUS_BUILD=str(args.sail_verus_build),
                                      SAIL_PERF_MACHINE=args.machine,
                                      SAIL_PERF_VERUS_ARGS=shlex.join(entry.get("verus_args", [])))
                    if entry.get("toolchain") == "project":
                        timing_env.pop("RUSTUP_TOOLCHAIN", None)
                    else:
                        timing_env["RUSTUP_TOOLCHAIN"] = pins["verus"]["rust_toolchain"]
                    if env.get("SAIL_VERUS_HOME"):
                        timing_env["SAIL_PERF_STOCK_HOME"] = env["SAIL_VERUS_HOME"]
                    outcome = execute([str(ROOT / "scripts/perf-real.sh"), "run",
                                       entry["managed"], rounds], directory / "managed.log", timing_env)
                    record["commands"].append(outcome)
                    record["timing_scope"] = "managed-cold-unchanged-comment-only"
                    try:
                        record["samples_complete"] = managed_complete(
                            directory / "managed/results.tsv", int(rounds))
                    except (OSError, ValueError) as error:
                        record["sample_refusal"] = str(error)
                else:
                    record["timing_scope"] = "direct-verus-cold-and-query-cache-warm"
                    record["samples_complete"] = True
                    for workload in ("cold", "warm"):
                        outcome = execute(["sail-bench", workload, *selection(entry),
                                           "--shim", binaries["sail-z3"]["path"],
                                           "--sail", binaries["sail"]["path"],
                                           "--rounds", rounds, "--sail-verus-build",
                                           str(args.sail_verus_build), "--out",
                                           str(directory / f"{workload}.json")],
                                          directory / f"{workload}.log", env)
                        record["commands"].append(outcome)
                        try:
                            record["samples_complete"] &= benchmark_complete(
                                directory / f"{workload}.json", workload, int(rounds))
                        except (OSError, ValueError, KeyError) as error:
                            record["samples_complete"] = False
                            record["sample_refusal"] = str(error)
                record["status"] = ("timing-complete" if record.get("samples_complete", False)
                                    and all(command["exit"] == 0
                                                           for command in record["commands"])
                                    else "timing-failed")
        results.append(record)
        (args.out / "index.json").write_text(json.dumps({
            "schema_version": 1, "phase": args.phase,
            "machine": args.machine,
            "binaries": binaries, "source": source, "inputs_sha256": inputs,
            "frozen_candidate": args.frozen_candidate,
            "results": results}, indent=2) + "\n")
    intact = inputs_intact(inputs) and source_state() == source
    index = args.out / "index.json"
    record = json.loads(index.read_text())
    record["inputs_unchanged"] = intact
    index.write_text(json.dumps(record, indent=2) + "\n")
    return 0 if intact and all(record["status"] in ("timing-complete", "intake-comparison-complete")
                               for record in results) else 1



def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("list", "run"))
    parser.add_argument("--project", action="append", default=[])
    parser.add_argument("--phase", choices=("intake", "screen", "accept"), default="intake")
    parser.add_argument("--out", type=lambda value: Path(value).resolve())
    parser.add_argument("--sail-verus-build", type=lambda value: Path(value).resolve())
    parser.add_argument("--frozen-candidate")
    parser.add_argument("--machine")
    args = parser.parse_args()
    entries = workloads()
    if args.command == "list":
        for entry in entries:
            print(json.dumps(entry, sort_keys=True))
        return 0
    if not args.out:
        parser.error("run needs --out outside the repository")
    if args.out.is_relative_to(ROOT):
        parser.error("raw evaluation results must stay outside the repository")
    return run(args, entries)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(2)
