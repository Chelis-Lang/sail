"""Read the realistic edit-loop workload into the release matrix."""
from collections import Counter
import json
import math
from pathlib import Path
import statistics

CLASSES = {"arith", "compare", "constant", "logic", "statement", "invariant",
           "spec-weaken", "spec-strengthen", "rare-region"}
PROJECTS = {"vest/vest_lib": "vest", "verified-ironkv/ironsht": "ironkv",
            "verified-storage/capybaraKV/capybarakv": "capybarakv",
            "human-eval-verus/tasks/lib.rs": "human-eval"}
IDENTITIES = ("sail_sha256", "stock_cargo_verus_sha256", "runtime_manifest_sha256",
              "corpus_sha256", "vstd_lock_sha256")


def analyse(directory, matrix):
    root = Path(directory)
    header = json.loads((root / "header.json").read_text())
    samples = [json.loads(line) for line in (root / "samples.jsonl").read_text().splitlines()
               if line.strip()]
    issues = []
    for key in IDENTITIES:
        if not matrix.get(key) or header.get(key) != matrix[key]:
            issues.append(f"{key} missing or different")
    if not matrix.get("platform") or header.get("platform") != matrix["platform"]:
        issues.append("platform missing or different")
    if header.get("generator") != "gen-mutants 2":
        issues.append("requires the corrected version 2 generator")
    if header.get("vstd_mode") != "registry" or header.get("oracle_vstd_mode") != "registry":
        issues.append("requires registry preparation for the oracle and workload")
    if not isinstance(header.get("oracle_runs"), int) or header["oracle_runs"] < 3:
        issues.append("requires three stock oracle runs")
    rounds = header.get("rounds", 0)
    if not isinstance(rounds, int) or rounds < 3 or rounds != matrix.get("rounds"):
        issues.append("missing or incompatible measured rounds")
        rounds = 0
    unit = header.get("unit", "")
    project = PROJECTS.get(unit, unit)
    if unit not in PROJECTS:
        issues.append("unsupported mutation unit")
    edits = header.get("mutants", [])
    if header.get("oracle_kept") != len(edits):
        issues.append("the complete qualified catalogue must be measured")
    if not edits or len(set(edits)) != len(edits):
        issues.append("missing or duplicate selected mutants")
    expected = {(r, "mutant", edit) for r in range(rounds) for edit in edits}
    expected |= {(r, phase, "original") for r in range(rounds)
                 for phase in ("cold", "revert", "revert-forced")}
    observed = Counter((s.get("round"), s.get("phase"), s.get("id")) for s in samples)
    if set(observed) != expected or any(n != 1 for n in observed.values()):
        issues.append("missing, duplicate or unexpected steps")
    if {s.get("class") for s in samples if s.get("phase") == "mutant"} != CLASSES:
        issues.append("all nine mutation classes must be measured")
    if any(s.get("stock_exit") is None or s.get("stock_exit") != s.get("sail_exit")
           or s.get("judgement") != "same" or s.get("false_verified")
           or s.get("expected_mismatch") or s.get("daemon_error") or s.get("provenance_error")
           or (s.get("phase") in ("revert", "revert-forced") and s.get("own_answers") is not True)
           for s in samples):
        issues.append("comparison, oracle, provenance or revert check failed")
    for r in range(rounds):
        pids = {s.get("daemon_pid") for s in samples if s.get("round") == r}
        if len(pids) != 1 or None in pids:
            issues.append(f"round {r} did not retain one daemon")

    totals = {"stock": [], "sail": []}
    slower = []
    for edit in edits:
        times = {arm: [s.get(arm + "_s") for s in samples
                       if s.get("phase") == "mutant" and s.get("id") == edit]
                 for arm in totals}
        if all(len(xs) == rounds and all(valid_time(x) for x in xs) for xs in times.values()):
            stock, sail = (statistics.median(times[arm]) for arm in totals)
            if sail > stock:
                slower.append((edit, stock, sail))
        else:
            issues.append(f"{edit}: missing or invalid timing")
    for r in range(rounds):
        entries = [s for s in samples if s.get("round") == r and s.get("phase") == "mutant"]
        if len(entries) == len(edits) and all(valid_time(s.get(arm + "_s"))
                                             for s in entries for arm in totals):
            for arm in totals:
                totals[arm].append(sum(s[arm + "_s"] for s in entries))
    row = dict(project=project, scenario=f"mutation-loop ({len(edits)} edits)",
               n=(len(totals["stock"]), len(totals["sail"])),
               complete=len(totals["stock"]) == rounds and rounds >= 3,
               failures=[], vstd_proof="see mutation setup", valid=not issues,
               performance="unmeasured", slower_mutants=slower, mutation_issues=issues,
               mutation_source=str(root))
    if all(totals.values()):
        stock, sail = (statistics.median(totals[arm]) for arm in totals)
        row.update(stock_med=stock, sail_med=sail,
                   stock_rng=(min(totals["stock"]), max(totals["stock"])),
                   sail_rng=(min(totals["sail"]), max(totals["sail"])),
                   ratio=stock / sail, delta_s=sail - stock,
                   performance="faster" if sail < stock else "equal" if sail == stock else "slower: review")
    row["pass"] = row["valid"] and row["performance"] in ("faster", "equal") and not slower
    return row


def valid_time(value):
    return isinstance(value, (int, float)) and math.isfinite(value) and value > 0
