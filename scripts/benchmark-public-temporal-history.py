#!/usr/bin/env python3
"""Check pinned public source edits with an installed Sail CLI and its staged worker."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import time
from urllib.request import urlopen


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def encode(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()


def fetch_sources(cache, suite):
    snapshots = {}
    for revision, sources in suite["revisions"].items():
        require(len(revision) == 40 and all(c in "0123456789abcdef" for c in revision), "unpinned revision")
        row = {}
        for name, expected in sources.items():
            file = cache / expected
            if not file.exists():
                url = f'https://raw.githubusercontent.com/{suite["repository"]}/{revision}/{suite["directory"]}/{name}'
                with urlopen(url, timeout=30) as stream:
                    data = stream.read()
                require(sha(data) == expected, f"download identity differs for {url}")
                with tempfile.NamedTemporaryFile(dir=cache, delete=False) as pending:
                    pending.write(data)
                os.replace(pending.name, file)
            data = file.read_bytes()
            require(sha(data) == expected, f"cached input identity differs for {name}")
            row[name] = data.decode()
        snapshots[revision] = row
    return snapshots


def jobs(suite, snapshots, states, depth, project):
    rows = []
    for revision, all_files in snapshots.items():
        for name, model in suite["models"].items():
            inputs = all_files if project else {key: all_files[key] for key in model["files"]}
            request = {"version": 1, "model": {"root": model["root"], "config": model["config"], "files": inputs},
                       "operation": {"kind": "check", "max_states": states, "max_depth": depth}}
            rows.append((name, revision, request, sha(encode(request))))
    return rows


def invoke(sail, arguments, request, wall_ms):
    data = encode(request)
    start = time.monotonic_ns()
    proc = subprocess.run([sail, "runtime", *arguments, "--file", "-"], input=data,
                          capture_output=True, timeout=wall_ms / 1000 + 10, check=False)
    elapsed_us = (time.monotonic_ns() - start) // 1000
    require(proc.returncode in (0, 1), (proc.returncode, proc.stderr[-1800:]))
    return json.loads(proc.stdout), elapsed_us, proc.returncode


def decision(response, expected_digest):
    require(response["request_sha256"] == expected_digest, "native response references different bytes")
    report = response["native_report"] or {}
    stats = report.get("statistics") or {}
    validations = response["validations"]
    require(response["outcome"] != "refuted" or
            (len(validations) > 0 and len(validations) == len(response["itf_traces"])), "missing validations")
    return {"outcome": response["outcome"], "completion": response["completion"],
            "inventory": response["requested_checks"], "message": response["message"],
            "verdict": report.get("verdict"), "property": report.get("property"),
            "counts": {key: stats[key] for key in ("initial_generated", "generated", "distinct", "depth", "left_on_queue")
                       if key in stats}, "trace_sha256": [sha(encode(t)) for t in response["itf_traces"]],
            "validations": [v["result"] if "result" in v else v.get("validation") for v in validations]}


def ordinary(sail, rows, cache, worker, wall_ms, arm):
    samples, decisions, prior = [], [], {}
    for index, (name, revision, request, digest) in enumerate(rows):
        job = {"version": 1, "input": {"kind": "temporal", "job": {"obligation_id": name + "-" + revision,
               "requirement_ids": [], "wall_time_ms": wall_ms, "request": request}}}
        directory = cache / f"fresh-{index}" if arm == "fresh" else cache
        previous = prior.get(name)
        arguments = ["execute", "--cache", directory]
        if arm == "recheck" and previous:
            arguments = ["recheck", "--cache", directory, "--previous", previous[0]]
        answer, elapsed, status = invoke(sail, arguments, job, wall_ms)
        if answer.get("route") == "unchanged-loaded-program":
            require(arm == "recheck" and previous and status == 0 and answer["outcome"] == "holds" and
                    answer["obligation_id"] == name + "-" + revision and answer["request_sha256"] == digest and
                    answer["previous_request_sha256"] == previous[1] and answer["worker_sha256"] == worker and
                    bool(answer["previous_receipt_key"]) and previous[2]["outcome"] == "holds" and
                    not ({"counts", "depth", "statistics", "native_report", "response"} & answer.keys()),
                    "unbound proof or unsolicited statistics")
            loaded = [n for n in answer["loaded_modules"] if not n.startswith("<standard>/")]
            require(request["model"]["root"] in loaded, "missing selected root")
            require(all(request["model"]["files"][key] == previous[3]["model"]["files"][key] for key in loaded),
                    "loaded program changed")
            samples.append([elapsed, answer["route"]])
            decisions.append({"projected_from": previous[1], "loaded": loaded})
            continue
        if arguments[0] == "recheck":
            require(answer["route"] == "current-check", "unexpected recheck evidence type")
            answer = answer["response"]
        r = answer["output"]["result"]
        require(r["worker_sha256"] == worker and r["request_sha256"] == digest and
                r["outcome"] == "completed" and (arm != "fresh" or not r["cache_hit"]), "unbound or nonterminal result")
        current = decision(r["response"], digest)
        require(status == int(current["outcome"] not in ("holds", "refuted")), "wrong ordinary exit")
        decisions.append(current)
        samples.append([elapsed, "exact-receipt" if r["cache_hit"] else "fresh"])
        previous_file = cache / f"request-{index}.json"
        previous_file.write_bytes(encode(job))
        prior[name] = (previous_file, digest, current, request)
    return samples, decisions


def edit_session(sail, rows, worker, wall_ms):
    batch = {"version": 1, "wall_time_ms": wall_ms, "checks": [r for _, _, r, _ in rows]}
    out, wall, status = invoke(sail, ["edit-session"], batch, wall_ms)
    require(out["outcome"] == "completed" and out["worker_sha256"] == worker and
            len(out["entries"]) == len(rows), "partial or unbound edit session")
    decisions, samples = [], []
    for (_, _, _, digest), entry in zip(rows, out["entries"], strict=True):
        require(entry["request_sha256"] == digest, "wrong session input")
        evidence = entry["evidence"]
        route = evidence["route"]
        require(route == "fresh", f"unimplemented independent evidence comparison for {route}")
        decisions.append(decision(evidence["response"], digest))
        samples.append([entry["elapsed_us"], route])
    require(status == int(any(d["outcome"] not in ("holds", "refuted") for d in decisions)), "wrong session exit")
    return samples, decisions, wall


def two_writer_control(sail, suite, snapshots, worker, wall_ms):
    """Replay a real source fix that discriminates model atoms from strings."""
    revisions = ("45c1cfd7ed40d9287b1c2ac51a05a7cb750e0521", "c757b12b7a4de6fa39523db541133fece9ced416")
    replacements = (("MaxPublished = 10", "MaxPublished = 2"), ("Writers = { w }", "Writers = { w1, w2 }"),
                    ("Readers = { r1, r2, r3 }", "Readers = { r1 }"), ("Size = 4", "Size = 2"))
    model = suite["models"]["SPMC"]

    def execute(request, cache, expected, cached=False):
        job = {"version": 1, "input": {"kind": "temporal", "job": {"obligation_id": "two-writer",
               "requirement_ids": [], "wall_time_ms": wall_ms, "request": request}}}
        answer, elapsed, status = invoke(sail, ["execute", "--cache", cache], job, wall_ms)
        result = answer["output"]["result"]
        digest = sha(encode(request))
        response = result["response"]
        require(result["worker_sha256"] == worker and result["request_sha256"] == digest and
                response["request_sha256"] == digest and result["cache_hit"] == cached and
                result["outcome"] == "completed" and response["outcome"] == expected and
                status == int(expected == "rejected"), "unexpected worker, request, outcome, receipt or exit")
        return response, {"request_sha256": digest, "outcome": expected, "exit": status, "wall_us": elapsed}

    rows = {}
    with tempfile.TemporaryDirectory() as cache:
        for revision in revisions:
            files = {name: snapshots[revision][name] for name in model["files"]}
            cfg = files[model["config"]]
            for before, after in replacements:
                require(cfg.count(before) == 1, f"unexpected configuration for {revision}: {before}")
                cfg = cfg.replace(before, after)
            files[model["config"]] = cfg
            request = {"version": 1, "model": {"root": model["root"], "config": model["config"], "files": files},
                       "operation": {"kind": "check", "max_states": 5000, "max_depth": 100}}
            before_fix = revision == revisions[0]
            response, row = execute(request, cache, "refuted" if before_fix else "rejected")
            rows[revision] = row
            report = response["native_report"]
            row.update(verdict=report["verdict"], property=report["property"], statistics=report.get("statistics"))
            if not before_fix:
                row["message"] = report["message"]
                require(report["verdict"] == "assumption-failure" and "is false" in row["message"] and
                        not response["itf_traces"], "source correction did not reject the two-writer constants")
                continue
            require(report["property"] == "NoDataRaces" and
                    [report["statistics"][k] for k in ("distinct", "generated", "depth")] == [4, 4, 3],
                    "wrong violated property, depth or counts")
            trace, = response["itf_traces"]
            require(trace.get("value_encoding") == "sail-model-values-v1" and len(trace["states"]) == 3,
                    "missing three-state model-valued witness")
            row["trace_sha256"] = sha(encode(trace))
            row["states"] = len(trace["states"])
            validation = {**request, "operation": {"kind": "validate", "trace": trace}}
            _, row["validate"] = execute(validation, cache, "valid")
            hit, row["exact_receipt"] = execute(request, cache, "refuted", cached=True)
            require(hit["itf_traces"] == [trace], "receipt changed the witness")

            def change_atom(value):
                if isinstance(value, dict) and set(value) == {"#sail-model"}:
                    return value["#sail-model"], True
                if isinstance(value, (list, dict)):
                    for key in (range(len(value)) if isinstance(value, list) else value):
                        value[key], changed = change_atom(value[key])
                        if changed:
                            return value, True
                return value, False

            _, changed = change_atom(trace["states"][0])
            require(changed, "missing initial model atom for the same-spelling mutation")
            _, row["same_spelling_string"] = execute(validation, cache, "invalid")
    return {"wall_ms": wall_ms, "max_states": 5000, "max_depth": 100, "runs": rows}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--sail", required=True, type=Path)
    p.add_argument("--input-cache", type=Path, required=True)
    p.add_argument("--manifest", type=Path, default=Path(__file__).with_name("temporal-history-inputs.json"))
    p.add_argument("--two-writer-control", action="store_true", help="check the pinned Disruptor counterexample and the next source correction")
    p.add_argument("--trials", type=int, default=5)
    p.add_argument("--states", type=int, default=200000)
    p.add_argument("--depth", type=int, default=128)
    p.add_argument("--wall-ms", type=int, default=300000)
    args = p.parse_args()
    require(min(args.trials, args.states, args.depth, args.wall_ms) > 0 and args.wall_ms <= 300000, "invalid limits")
    sail = args.sail.resolve()
    manifest = json.loads(args.manifest.read_bytes())
    args.input_cache.mkdir(parents=True, exist_ok=True)
    worker_path = sail.with_name("sail-runtime-worker")
    with worker_path.open("rb") as f:
        worker = hashlib.file_digest(f, "sha256").hexdigest()
    require(worker == json.loads(sail.with_name("sail-runtime-worker.json").read_bytes())["sha256"],
            "installed worker failed its identity check")
    with sail.open("rb") as f:
        cli = hashlib.file_digest(f, "sha256").hexdigest()
    if args.two_writer_control:
        suite = manifest["disruptor"]
        evidence = two_writer_control(sail, suite, fetch_sources(args.input_cache, suite), worker, args.wall_ms)
        print(json.dumps({"format": "sail-disruptor-witness-v1", "sail_sha256": cli, "worker_sha256": worker,
                          "manifest_sha256": sha(args.manifest.read_bytes()),
                          "system": [platform.system(), platform.release(), platform.machine()], **evidence}, indent=2))
        return
    output = {"format": "sail-public-history-v1", "sail_sha256": cli, "worker_sha256": worker,
              "manifest_sha256": sha(args.manifest.read_bytes()), "system": [platform.system(), platform.release(),
              platform.machine()], "limits": [args.states, args.depth, args.wall_ms], "suites": {}}
    for label, suite in manifest.items():
        snapshots = fetch_sources(args.input_cache, suite)
        for scope in (("selected", "project") if len(suite["models"]) > 1 else ("selected",)):
            rows = jobs(suite, snapshots, args.states, args.depth, scope == "project")
            measured = {"sequence": [[n, rev, dg] for n, rev, _, dg in rows], "trials": []}
            for i in range(args.trials):
                record, results = {}, {}
                arms = ("fresh", "receipts", "session", "recheck")
                for arm in arms[i % len(arms):] + arms[:i % len(arms)]:
                    with tempfile.TemporaryDirectory() as tmp:
                        if arm == "session":
                            values, results[arm], total = edit_session(sail, rows, worker, args.wall_ms)
                        else:
                            values, results[arm] = ordinary(sail, rows, Path(tmp), worker, args.wall_ms, arm)
                            total = sum(v[0] for v in values)
                        record[arm] = {"total_us": total, "steps": values}
                oracle = results["fresh"]
                require(oracle == results["receipts"] == results["session"], f"{label}: unequal evidence")
                for index, (actual, reference) in enumerate(zip(results["recheck"], oracle, strict=True)):
                    if "projected_from" not in actual:
                        require(actual == reference, "ordinary fallback differs")
                        continue
                    name = rows[index][0]
                    expected = suite["models"][name]
                    require(set(actual["loaded"]) == set(expected["files"]) - {expected["config"]},
                            "loaded file inventory disagrees with the input manifest")
                    require(reference["outcome"] == "holds" and
                            any(previous[-1] == actual["projected_from"] and oracle[j] == reference
                                for j, previous in enumerate(rows[:index])), "projection disagrees with fresh checking")
                require(measured.setdefault("decisions", oracle) == oracle, "unstable reference")
                measured["trials"].append(record)
            output["suites"][label + "-" + scope] = measured
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
