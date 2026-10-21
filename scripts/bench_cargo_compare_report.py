"""Proof validation and timing reports for the Cargo verifier comparison."""

import copy
import hashlib
import itertools
import json
import math
import re
import statistics

MODES = ("stock", "fork", "sail")
SCENARIOS = ("fresh", "unchanged", "proof-edit", "daemon-restart")
LIBRARY_KINDS = ("lib", "rlib", "dylib", "cdylib", "staticlib")
_ORDERS = (MODES, ("fork", "sail", "stock"), ("sail", "stock", "fork"),
           ("sail", "fork", "stock"), ("stock", "sail", "fork"),
           ("fork", "stock", "sail"))
_COUNTERS = ("sessions", "gets", "hits", "puts", "memory_hits")
_NATIVE = re.compile(r"^\s*verification results::\s*(\d+) verified,\s*(\d+) errors\s*$", re.M)


def trial_order(round_index):
    """Return the balanced six-order cycle for a zero-based round."""
    return _ORDERS[round_index % len(_ORDERS)]


def _integer(value):
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _positive(value):
    return (isinstance(value, (int, float)) and not isinstance(value, bool)
            and math.isfinite(value) and value > 0)


def parse_proof(mode, stdout):
    """Parse proof evidence. Retain parse failures as report problems."""
    result = {"kind": "sail" if mode == "sail" else "native", "counts": [],
              "verified": 0, "errors": 0, "crate_counts": [],
              "scope_sha256": None, "problems": []}
    problems = result["problems"]
    if mode not in MODES:
        problems.append("Unknown verifier mode: %s" % mode)
        return result
    if not isinstance(stdout, str):
        problems.append("Proof output is not text")
        return result
    if mode != "sail":
        matches = [(int(v), int(e)) for v, e in _NATIVE.findall(stdout)]
        result["counts"] = [v for v, _ in matches]
        result["verified"] = sum(v for v, _ in matches)
        result["errors"] = sum(e for _, e in matches)
        if not matches:
            problems.append("Native verification summaries are absent")
        if any(v <= 0 or e != 0 for v, e in matches):
            problems.append("Native verification is incomplete or reports errors")
        return result
    try:
        document = json.loads(stdout)
        if not isinstance(document, dict):
            raise ValueError("Report must be an object")
        if document.get("kind") == "sail-project-result":
            if document.get("exit_code") != 0 or document.get("error") is not None:
                problems.append("Sail project reports an error")
            execution = document.get("execution", {})
            if not _positive(execution.get("verifier_invocations")):
                problems.append("Sail skipped verifier execution")
            if execution.get("reused_targets") != 0:
                problems.append("Sail reused whole-target proofs")
            targets = document.get("targets", [])
            if not targets:
                problems.append("Sail project has no target reports")
            reports = []
            plain = (document.get("no_reuse") or {}).get("verified_as") == "plain-cargo-verus"
            for target in targets:
                if target.get("reused") is not False or target.get("dependency_store") or target.get("dependency_build"):
                    problems.append("Sail target reused prior proof work")
                if not plain and target.get("complete") is not True:
                    problems.append("Sail target proof is incomplete")
                reports.append(target["report"])
        else:
            reports = [document]
        identities = []
        names = set()
        for report in reports:
            if report.get("exit_code") != 0 or report.get("error") is not None:
                problems.append("Sail report has an error")
            if report.get("mode") != "default":
                problems.append("Sail report does not use default mode")
            crates = report.get("crates", [])
            total = 0
            for crate in crates:
                name = crate["name"]
                if not isinstance(name, str) or not name or name in names:
                    problems.append("Sail crate identity is empty or duplicated")
                names.add(name)
                verus = crate.get("verus", {})
                if (not _positive(verus.get("verified")) or verus.get("errors") != 0
                        or verus.get("success") is not True
                        or verus.get("encountered_vir_error") is not False):
                    problems.append("Sail crate verification is incomplete or reports errors")
                errors = verus.get("errors")
                if _integer(errors):
                    result["errors"] += errors
                functions = crate.get("functions", [])
                if not functions:
                    problems.append("Sail crate has no function proofs")
                for function in functions:
                    raw_mode = function.get("fn_mode")
                    identity = (name, function["module"], function["function"],
                                "" if raw_mode is None else raw_mode, function["verdict"])
                    if not all(isinstance(v, str) for v in identity) or not identity[2]:
                        problems.append("Sail function identity is invalid")
                    identities.append(identity)
                    raw_success = function.get("verus_success")
                    if (function.get("verdict") != "verified"
                            or function.get("establishment") != "confirmed"
                            or (raw_success is not None and raw_success is not True)):
                        problems.append("Sail function is not confirmed verified")
                count = len(functions)
                total += count
                result["crate_counts"].append({"name": name, "functions": count})
            summary = report.get("summary", {})
            if (summary.get("functions") != total or summary.get("verified") != total
                    or any(summary.get(key) != 0 for key in ("failed", "unknown", "provisional"))):
                problems.append("Sail summary does not match complete verified functions")
        if not identities:
            problems.append("Sail report contains no proof work")
        result["crate_counts"].sort(key=lambda c: c["name"])
        result["counts"] = [c["functions"] for c in result["crate_counts"]]
        result["verified"] = sum(result["counts"])
        result["scope_sha256"] = hashlib.sha256(json.dumps(
            sorted(identities), separators=(",", ":"), ensure_ascii=True).encode("utf-8")).hexdigest()
    except (ValueError, TypeError, KeyError, AttributeError) as error:
        problems.append("Cannot parse Sail proof report: %s" % error)
    return result



def _crate_name(name):
    return name.replace("-", "_") if isinstance(name, str) else None


def parse_workflow_proof(mode, stdout, stderr, root_package_id, root_crate):
    """Separate new requested-root proofs from confirmed proof reuse."""
    result = {"kind": "sail" if mode == "sail" else "native", "counts": [],
              "verified": 0, "errors": 0, "crate_counts": [], "scope_sha256": None,
              "problems": [], "outcome": None, "root_verified": 0,
              "root_scope_sha256": None,
              "execution": {"verifier_invocations": 0, "reused_targets": 0}}
    issues = result["problems"]
    if (mode not in MODES or not isinstance(stdout, str) or not isinstance(stderr, str)
            or not isinstance(root_package_id, str) or not root_package_id
            or not isinstance(root_crate, str) or not root_crate):
        issues.append("Workflow proof identity or output is invalid")
        return result
    if mode != "sail":
        text = stdout + "\n" + stderr
        artifacts, finishes = [], []
        for line in text.splitlines():
            try:
                message = json.loads(line)
            except (ValueError, TypeError):
                continue
            if not isinstance(message, dict):
                continue
            if message.get("reason") == "build-finished":
                finishes.append(message.get("success"))
            if message.get("reason") != "compiler-artifact":
                continue
            target = message.get("target") or {}
            if (message.get("package_id") == root_package_id
                    and _crate_name(target.get("name")) == _crate_name(root_crate)
                    and any(kind in LIBRARY_KINDS for kind in target.get("kind", []))):
                artifacts.append(message)
        if not finishes or any(value is not True for value in finishes):
            issues.append("Successful Cargo completion evidence is absent")
        if len(artifacts) != 1 or not isinstance(artifacts[0].get("fresh"), bool):
            issues.append("Selected library Cargo artifact evidence is absent or ambiguous")
            return result
        matches = _NATIVE.findall(text)
        if matches:
            parsed = parse_proof(mode, text)
            result.update(parsed)
            result["problems"] = issues + parsed["problems"]
            result["root_verified"] = parsed["counts"][-1]
            result["execution"]["verifier_invocations"] = len(parsed["counts"])
            result["outcome"] = "verified"
            if artifacts[0]["fresh"] is not False:
                result["problems"].append("Fresh Cargo artifact contradicts new root proof work")
        else:
            result["outcome"] = "cargo-noop"
            if artifacts[0]["fresh"] is not True:
                issues.append("Cargo rebuilt the selected library without complete root proof work")
        return result
    try:
        document = json.loads(stdout)
        if not isinstance(document, dict) or document.get("kind") != "sail-project-result":
            raise ValueError("Managed Sail project report is absent")
        if document.get("exit_code") != 0 or document.get("error") is not None:
            issues.append("Sail project reports an error")
        if document.get("currentness") != "current":
            issues.append("Sail project input currentness is not confirmed")
        run_id = document.get("run_id")
        if not isinstance(run_id, str) or not run_id:
            issues.append("Current Sail run identity is absent")
        execution = document.get("execution") or {}
        result["execution"] = {key: execution.get(key) for key in
                               ("verifier_invocations", "reused_targets")}
        if not all(_integer(value) for value in result["execution"].values()):
            issues.append("Sail execution counters are invalid")
        targets = document.get("targets")
        if not isinstance(targets, list) or not targets:
            raise ValueError("Complete Sail target reports are absent")
        roots, fresh_reports, identities = [], [], set()
        reused_count = 0
        for target in targets:
            identity = (target["package_id"], _crate_name(target["target"]),
                        tuple(target["kind"]))
            if (not isinstance(identity[0], str) or not identity[0]
                    or not identity[1] or identity in identities):
                issues.append("Sail target identity is invalid or duplicated")
            identities.add(identity)
            if target.get("complete") is not True or not isinstance(target.get("reused"), bool):
                issues.append("Sail target proof is incomplete")
            if not isinstance(target.get("requested"), bool):
                issues.append("Sail target requested status is absent")
            if not isinstance(target.get("establishing_run"), str) or not target["establishing_run"]:
                issues.append("Sail target proof provenance is absent")
            parsed = parse_proof("sail", json.dumps(target["report"]))
            issues.extend(parsed["problems"])
            reused_count += target.get("reused") is True
            replay = (target.get("reused") is True or bool(target.get("dependency_store"))
                      or bool(target.get("dependency_build")))
            if not replay:
                if target.get("establishing_run") != run_id:
                    issues.append("Fresh Sail target proof belongs to another run")
                fresh_reports.append(parsed)
            root_identity = (identity[0] == root_package_id
                             and identity[1] == _crate_name(root_crate)
                             and any(kind in LIBRARY_KINDS for kind in identity[2]))
            if root_identity:
                if target.get("requested") is not True:
                    issues.append("Selected Sail root is not a requested target")
                roots.append((target, parsed, replay))
            elif target.get("requested") is True:
                issues.append("Sail requested target differs from the selected library")
        if execution.get("reused_targets") != reused_count:
            issues.append("Sail reused-target counter differs from target reports")
        if fresh_reports and not _positive(execution.get("verifier_invocations")):
            issues.append("Sail new proof work has no verifier invocation")
        if len(roots) != 1:
            issues.append("Selected Sail root proof is absent or ambiguous")
        else:
            target, root_report, replay = roots[0]
            root_crates = [crate for crate in target["report"].get("crates", [])
                           if _crate_name(crate.get("name")) == _crate_name(root_crate)]
            if len(root_crates) != 1 or len(target["report"].get("crates", [])) != 1:
                issues.append("Sail root report does not identify exactly the selected crate")
            result["root_verified"] = root_report["verified"]
            result["root_scope_sha256"] = root_report["scope_sha256"]
            result["outcome"] = "reused" if replay else "verified"
            if replay and target.get("reused") is not True:
                issues.append("Requested root used dependency replay instead of a root receipt")
        for parsed in fresh_reports:
            result["crate_counts"].extend(parsed["crate_counts"])
            result["errors"] += parsed["errors"]
        result["crate_counts"].sort(key=lambda crate: crate["name"])
        result["counts"] = [crate["functions"] for crate in result["crate_counts"]]
        result["verified"] = sum(result["counts"])
        if fresh_reports:
            result["scope_sha256"] = hashlib.sha256(json.dumps(sorted(
                parsed["scope_sha256"] for parsed in fresh_reports)).encode("utf-8")).hexdigest()
    except (ValueError, TypeError, KeyError, AttributeError) as error:
        issues.append("Cannot parse Sail workflow report: %s" % error)
    return result


def _cache_problems(row):
    issues = row["problems"]
    before, after, delta = (row.get(name) or {} for name in
                            ("cache_before", "cache_after", "cache_delta"))
    if not _positive(before.get("pid")) or before.get("pid") != after.get("pid"):
        issues.append("Daemon PID is absent or changed during the command")
    counters = set(_COUNTERS) | ((set(before) | set(after) | set(delta)) - {"pid", "state"})
    for counter in sorted(counters):
        a, b, d = before.get(counter), after.get(counter), delta.get(counter)
        if not all(_integer(value) for value in (a, b, d)) or b - a != d:
            issues.append("Invalid daemon counter: %s" % counter)
    for snapshot in (before, after):
        state = snapshot.get("state") or {}
        if not all(_integer(state.get(key)) for key in
                   ("memory_entries", "disk_entries", "pending_writes")):
            issues.append("Daemon cache state is absent or invalid")
    if "state" in delta:
        issues.append("Daemon cache state is not a counter delta")
    for snapshot in (before, after, delta):
        hits, gets, memory = (snapshot.get(key) for key in ("hits", "gets", "memory_hits"))
        if (all(_integer(value) for value in (hits, gets, memory))
                and not memory <= hits <= gets):
            issues.append("Daemon hit counters are inconsistent")
    if row.get("mode") != "sail" and any(delta.get(key) != 0 for key in counters):
        issues.append("Native invocation made daemon requests")


def _full_proof_problems(row):
    proof = row.get("proof") or {}
    counts = proof.get("counts", [])
    if (proof.get("kind") != ("sail" if row.get("mode") == "sail" else "native")
            or not counts or not all(_integer(count) and count > 0 for count in counts)
            or proof.get("verified") != sum(counts) or proof.get("errors") != 0):
        row["problems"].append("Complete successful proof work is absent")


def _workflow_problems(row):
    proof = row.get("proof") or {}
    issues = row["problems"]
    outcome, mode = proof.get("outcome"), row.get("mode")
    if (proof.get("kind") != ("sail" if mode == "sail" else "native")
            or outcome not in (("verified", "reused") if mode == "sail"
                              else ("verified", "cargo-noop"))
            or proof.get("errors") != 0):
        issues.append("Successful workflow proof evidence is absent")
    execution = proof.get("execution") or {}
    if not all(_integer(execution.get(key)) for key in ("verifier_invocations", "reused_targets")):
        issues.append("Workflow execution evidence is absent")
    counts = proof.get("counts", [])
    if (not all(_integer(count) and count > 0 for count in counts)
            or proof.get("verified") != sum(counts)):
        issues.append("Workflow proof work counts are inconsistent")
    if outcome == "verified":
        if (not _positive(proof.get("root_verified")) or not counts
                or not _positive(execution.get("verifier_invocations"))):
            issues.append("Fresh requested-root verification is absent")
    if outcome == "cargo-noop" and (counts or proof.get("root_verified") != 0
                                    or execution.get("verifier_invocations") != 0):
        issues.append("Cargo no-op incorrectly reports new proof work")
    if mode == "sail":
        if not _positive(proof.get("root_verified")) or not proof.get("root_scope_sha256"):
            issues.append("Confirmed Sail root proof scope is absent")
        if outcome == "reused" and not _positive(execution.get("reused_targets")):
            issues.append("Sail root reuse lacks a confirmed receipt")
    if row.get("phase") == "warmup" or row.get("scenario") == "proof-edit":
        if outcome != "verified":
            issues.append("Baseline or proof edit did not freshly verify the requested root")


def _restart_problems(row, previous):
    issues = row["problems"]
    before = row.get("cache_before") or {}
    prior = previous.get("cache_after") or {} if previous else {}
    state = before.get("state") or {}
    restart = row.get("restart") or {}
    old_disk = restart.get("old_disk_entries")
    if (not _positive(restart.get("old_pid")) or restart.get("old_pid") == before.get("pid")
            or restart.get("old_pid") != prior.get("pid")):
        issues.append("Restart did not replace the preceding owned daemon")
    if (not _integer(old_disk) or old_disk <= 0 or state.get("disk_entries") != old_disk
            or not _integer((prior.get("state") or {}).get("disk_entries"))
            or old_disk < (prior.get("state") or {}).get("disk_entries", 0)):
        issues.append("Restart did not retain the private disk cache")
    old_snapshot = restart.get("old_snapshot") or {}
    old_state = old_snapshot.get("state") or {}
    if (old_snapshot.get("pid") != restart.get("old_pid")
            or old_state.get("disk_entries") != old_disk or old_state.get("pending_writes") != 0):
        issues.append("Restart lacks the drained old daemon disk state")
    if any(not _integer(old_snapshot.get(key)) or not _integer(prior.get(key))
           or old_snapshot[key] < prior[key] for key in _COUNTERS):
        issues.append("Old daemon counters regressed before restart")
    if state.get("memory_entries") != 0 or state.get("pending_writes") != 0:
        issues.append("Restart initial cache memory or pending writes are not empty")
    if any(value != 0 for key, value in before.items() if key not in ("pid", "state")):
        issues.append("Restart initial daemon counters are not zero")
    delta = row.get("cache_delta") or {}
    hits, memory = delta.get("hits"), delta.get("memory_hits")
    if not all(_integer(value) for value in (hits, memory)) or hits - memory <= 0:
        issues.append("Restart sample has no retained disk cache hits")
    if not _positive(row.get("startup_seconds")):
        issues.append("Restart daemon startup time is absent or invalid")
    elif _positive(row.get("wall_seconds")) and row["startup_seconds"] > row["wall_seconds"]:
        issues.append("Restart wall time excludes daemon startup")


def _scenario_summary(name, metadata, samples, rounds, global_problems):
    problems = list(global_problems)
    problems.extend((metadata.get("scenario_problems") or {}).get(name, []))
    expected = {(phase, rnd, mode) for phase, numbers in
                (("warmup", range(1, rounds + 1) if name == "proof-edit" else [0]),
                 ("measured", range(1, rounds + 1)))
                for rnd in numbers for mode in MODES}
    indexed = {}
    for row in samples:
        key = (row.get("phase"), row.get("round"), row.get("mode"))
        indexed.setdefault(key, []).append(row)
        issues = row["problems"]
        if key not in expected:
            issues.append("Unexpected sample phase, round, or mode")
        if row.get("exit_code") != 0:
            issues.append("Invocation did not exit successfully")
        if not _positive(row.get("wall_seconds")):
            issues.append("Wall time must be finite and positive")
        issues.extend((row.get("proof") or {}).get("problems", []))
        expected_source = "edited" if name == "proof-edit" and key[0] == "measured" else "base"
        if row.get("source_state") != expected_source:
            issues.append("Sample source state differs from the scenario phase")
        source_checks = row.get("source_checks") or {}
        if any(source_checks.get(phase) != [] for phase in ("before", "after")):
            issues.append("Successful source checks before and after the command are absent")
        edit = metadata.get("edit") or {}
        if edit or name == "proof-edit":
            hash_key = {"base": "before_sha256", "edited": "after_sha256"}.get(row.get("source_state"))
            if hash_key is None or row.get("edit_sha256") != edit.get(hash_key):
                issues.append("Sample edit hash differs from its declared source state")
        if name in ("fresh", "daemon-restart"):
            _full_proof_problems(row)
        else:
            _workflow_problems(row)
        _cache_problems(row)
    for key in sorted(expected - set(indexed)):
        problems.append("Missing sample: %s/%s/%s/%s" % ((name,) + key))
    for key, matches in indexed.items():
        if len(matches) != 1:
            problems.append("Duplicate sample: %s/%s/%s/%s" % ((name,) + key))
            for row in matches:
                row["problems"].append("Duplicate sample")

    def one(phase, rnd, mode):
        matches = indexed.get((phase, rnd, mode), [])
        return matches[0] if len(matches) == 1 else None

    if name == "proof-edit":
        contexts = {}
        for rnd in range(1, rounds + 1):
            for mode in MODES:
                baseline = one("warmup", rnd, mode)
                if baseline is None:
                    continue
                context = baseline.get("artifact_context")
                if not isinstance(context, str) or not context:
                    baseline["problems"].append("Proof-edit baseline artifact context is absent")
                elif context in contexts:
                    baseline["problems"].append("Proof-edit baseline reused another pair artifact context")
                    contexts[context]["problems"].append("Proof-edit baseline artifact context is not independent")
                else:
                    contexts[context] = baseline
        for phase in ("warmup", "measured"):
            for rnd in range(1, rounds + 1):
                stock = one(phase, rnd, "stock")
                fork = one(phase, rnd, "fork")
                if stock is None or fork is None:
                    continue
                if ((stock.get("proof") or {}).get("root_verified")
                        != (fork.get("proof") or {}).get("root_verified")):
                    for row in (stock, fork):
                        row["problems"].append(
                            "Native requested-root counts differ for the same source state")

    if name in ("fresh", "daemon-restart"):
        stock, sail = one("warmup", 0, "stock"), one("warmup", 0, "sail")
        native_reference = stock.get("proof") or {} if stock else {}
        sail_reference = sail.get("proof") or {} if sail else {}
        for row in samples:
            proof = row.get("proof") or {}
            if row.get("mode") != "sail":
                if sorted(proof.get("counts", [])) != sorted(native_reference.get("counts", [])):
                    row["problems"].append("Native count scope differs from the stock warmup")
            else:
                crates = proof.get("crate_counts", [])
                if (not crates or crates != sail_reference.get("crate_counts")
                        or not proof.get("scope_sha256")
                        or proof.get("scope_sha256") != sail_reference.get("scope_sha256")):
                    row["problems"].append("Sail function scope differs from the Sail warmup")
                if len(crates) != len(native_reference.get("counts", [])):
                    row["problems"].append("Sail crate count differs from the native report count")
                if row.get("phase") == "measured" and not _positive(
                        (row.get("cache_delta") or {}).get("gets")):
                    row["problems"].append("Measured Sail sample has no cache lookups")
    for rnd in range(1, rounds + 1):
        for mode in MODES:
            row = one("measured", rnd, mode)
            if row is None:
                continue
            baseline = one("warmup", rnd if name == "proof-edit" else 0, mode)
            if baseline is None or baseline["problems"]:
                row["problems"].append("Successful sample baseline is absent")
                continue
            if name not in ("unchanged", "proof-edit"):
                continue
            context = row.get("artifact_context")
            if (not isinstance(context, str) or not context
                    or context != baseline.get("artifact_context")):
                row["problems"].append("Workflow artifact context differs from its own baseline")
            proof, reference = row.get("proof") or {}, baseline.get("proof") or {}
            if proof.get("outcome") != "cargo-noop":
                if ((name != "proof-edit" or mode == "sail")
                        and proof.get("root_verified") != reference.get("root_verified")):
                    row["problems"].append("Requested-root count differs from its own baseline")
                if mode == "sail" and proof.get("root_scope_sha256") != reference.get("root_scope_sha256"):
                    row["problems"].append("Requested-root function scope differs from its own baseline")

    previous = None
    first_sail = True
    for row in samples:
        if row.get("mode") != "sail":
            continue
        before = row.get("cache_before") or {}
        if first_sail:
            if row.get("phase") != "warmup":
                row["problems"].append("First Sail sample is not a warmup")
            if any(value != 0 for key, value in before.items() if key not in ("pid", "state")):
                row["problems"].append("Sail warmup did not start with zero private cache counters")
            if any((before.get("state") or {}).get(key) != 0 for key in
                   ("memory_entries", "disk_entries", "pending_writes")):
                row["problems"].append("Sail warmup did not start with an empty private cache")
            first_sail = False
        elif name == "daemon-restart" and row.get("phase") == "measured":
            _restart_problems(row, previous)
        else:
            prior = previous.get("cache_after") or {}
            if before.get("pid") != prior.get("pid"):
                row["problems"].append("Warm Sail daemon PID changed")
            counters = (set(before) | set(prior)) - {"pid", "state"}
            if any(not _integer(before.get(key)) or not _integer(prior.get(key))
                   or before[key] < prior[key] for key in counters):
                row["problems"].append("Warm Sail cache counters regressed between samples")
        previous = row
    # Baseline cache checks above can invalidate an earlier baseline.
    for rnd in range(1, rounds + 1):
        for mode in MODES:
            row = one("measured", rnd, mode)
            baseline = one("warmup", rnd if name == "proof-edit" else 0, mode)
            if (row is not None and baseline is not None and baseline["problems"]
                    and "Successful sample baseline is absent" not in row["problems"]):
                row["problems"].append("Successful sample baseline is absent")
    for index, row in enumerate(samples):
        if row["problems"]:
            problems.append("Sample %d (%s/%s/%s/%s): %s" % (
                index, name, row.get("phase"), row.get("round"), row.get("mode"),
                ", ".join(str(problem) for problem in row["problems"])))
    result = {"valid": not problems, "problems": problems, "expected_samples": len(expected),
              "observed_samples": len(samples), "aggregates": None, "paired_speedups": None}
    if problems:
        return result
    aggregates = {}
    for mode in MODES:
        values = [one("measured", rnd, mode)["wall_seconds"] for rnd in range(1, rounds + 1)]
        aggregates[mode] = {"samples": values, "median_seconds": statistics.median(values),
                            "min_seconds": min(values), "max_seconds": max(values),
                            "stdev_seconds": statistics.stdev(values) if len(values) > 1 else None}
    pairs = {}
    for baseline, candidate in itertools.combinations(MODES, 2):
        ratios = [{"round": rnd, "ratio": one("measured", rnd, baseline)["wall_seconds"] /
                   one("measured", rnd, candidate)["wall_seconds"]} for rnd in range(1, rounds + 1)]
        pairs[baseline + "_over_" + candidate] = {
            "baseline": baseline, "candidate": candidate, "rounds": ratios,
            "median_ratio": statistics.median(row["ratio"] for row in ratios)}
    result["aggregates"], result["paired_speedups"] = aggregates, pairs
    return result


def summarize(metadata, rows):
    """Validate samples and calculate separate results for each scenario."""
    samples = copy.deepcopy(list(rows))
    problems = list(metadata.get("problems", []))
    rounds = metadata.get("rounds")
    if not _integer(rounds) or rounds < 1:
        problems.append("Metadata rounds must be a positive integer")
        rounds = 0
    if metadata.get("schema_version") != 2:
        problems.append("Metadata schema version must be 2")
    if list(metadata.get("modes", [])) != list(MODES):
        problems.append("Metadata modes must be stock, fork, sail")
    selected = metadata.get("scenarios", [])
    if (not isinstance(selected, list) or not selected
            or any(name not in SCENARIOS for name in selected)
            or len(set(selected)) != len(selected)):
        problems.append("Metadata scenarios are absent, invalid, or duplicated")
        selected = [name for name in SCENARIOS if name in selected] if isinstance(selected, list) else []
    if any(name in selected for name in ("unchanged", "proof-edit")):
        if not all(isinstance(metadata.get(key), str) and metadata[key]
                   for key in ("root_package_id", "root_crate")):
            problems.append("Selected library identity is absent")
    if "proof-edit" in selected:
        edit = metadata.get("edit") or {}
        if (not isinstance(edit.get("path"), str) or not edit["path"]
                or not all(isinstance(edit.get(key), str) and re.fullmatch(r"[0-9a-f]{64}", edit[key])
                           for key in ("before_sha256", "after_sha256"))
                or edit.get("before_sha256") == edit.get("after_sha256")
                or not isinstance(edit.get("spec"), dict)):
            problems.append("Captured proof edit identity is absent or invalid")
    for row in samples:
        row["problems"] = list(row.get("problems", []))
        if row.get("scenario") not in selected:
            row["problems"].append("Unexpected sample scenario")
            problems.append("Unexpected sample scenario: %s" % row.get("scenario"))
    scenarios = {name: _scenario_summary(name, metadata,
                 [row for row in samples if row.get("scenario") == name], rounds, problems)
                 for name in selected}
    all_problems = list(problems)
    for name, result in scenarios.items():
        all_problems.extend("%s: %s" % (name, issue) for issue in result["problems"])
    return {"schema_version": 2, "valid": not all_problems, "problems": all_problems,
            "metadata": copy.deepcopy(metadata), "samples": samples,
            "expected_samples": sum(value["expected_samples"] for value in scenarios.values()),
            "observed_samples": len(samples), "scenarios": scenarios}


def _cell(value):
    return str(value).replace("|", "\\|").replace("\n", " ")


def markdown_report(summary):
    """Show each scenario, all sample evidence, and all validation problems."""
    lines = ["# Cargo verifier comparison", "", "Status: **%s**" %
             ("valid" if summary["valid"] else "invalid"), "",
             "Wall times include Cargo and verification. Scenarios use separate solver caches.",
             "Counts describe new proof work. Root counts also describe confirmed receipt proofs.",
             "This benchmark does not establish formal per-function conformance.", ""]
    if summary["problems"]:
        lines.extend(["## Problems", ""])
        lines.extend("- " + _cell(problem) for problem in summary["problems"])
        lines.append("")
    descriptions = {
        "fresh": "Each command uses a fresh Cargo target. Sail retains its private solver cache.",
        "unchanged": "Commands retain Cargo artifacts after one successful baseline per mode.",
        "proof-edit": "Each edit command uses its own base-source baseline and artifact context.",
        "daemon-restart": "Each command uses a fresh Cargo target. Sail startup time is included after each private daemon restart."}
    for name, scenario in summary["scenarios"].items():
        lines.extend(["## " + name, "", descriptions[name], "",
                      "Status: **%s**. Samples: %s observed, %s expected." % (
                          "valid" if scenario["valid"] else "invalid", scenario["observed_samples"],
                          scenario["expected_samples"]), ""])
        if scenario["aggregates"] is None:
            lines.extend(["Timing aggregates and paired speedups are null because this scenario is invalid.", ""])
            continue
        lines.extend(["### Wall time", "", "| Mode | Median (s) | Minimum (s) | Maximum (s) | Sample standard deviation (s) |",
                      "| --- | ---: | ---: | ---: | ---: |"])
        for mode, stats in scenario["aggregates"].items():
            deviation = stats["stdev_seconds"]
            lines.append("| %s | %.6f | %.6f | %.6f | %s |" % (
                mode, stats["median_seconds"], stats["min_seconds"], stats["max_seconds"],
                "%.6f" % deviation if deviation is not None else "n/a"))
        lines.extend(["", "### Paired speedups", "",
                      "Ratios divide baseline time by candidate time. Values above one favor the candidate.", ""])
        for name, pair in scenario["paired_speedups"].items():
            lines.append("- %s: median %.6fx. %s" % (name, pair["median_ratio"], ", ".join(
                "Round %d: %.6fx" % (row["round"], row["ratio"]) for row in pair["rounds"])))
        lines.append("")
    lines.extend(["## Raw samples", "",
                  "Full source, proof, cache, CPU, and restart evidence is in `summary.json` and `samples.jsonl`.", "",
                  "| Scenario | Phase | Round | Mode | Outcome | New proofs | Wall (s) | Exit | Problems | Logs |",
                  "| --- | --- | ---: | --- | --- | ---: | ---: | --- | --- | --- |"])
    for row in summary["samples"]:
        proof = row.get("proof") or {}
        outcome = proof.get("outcome")
        if outcome is None and not row["problems"] and _positive(proof.get("verified")):
            outcome = "verified"
        fields = [row.get("scenario"), row.get("phase"), row.get("round"), row.get("mode"),
                  outcome, proof.get("verified"), row.get("wall_seconds"), row.get("exit_code"),
                  ", ".join(row["problems"]), "%s, %s" % (row.get("stdout"), row.get("stderr"))]
        lines.append("| " + " | ".join(_cell(value) for value in fields) + " |")
    return "\n".join(lines) + "\n"
