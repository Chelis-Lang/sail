#!/usr/bin/env python3
"""Time cross-process induction and fresh exact search on the same edited safety model."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sample(command, expected):
    start = time.monotonic_ns()
    process = subprocess.run(command, capture_output=True, check=False)
    elapsed = (time.monotonic_ns() - start) / 1e6
    if process.returncode != expected:
        raise RuntimeError(f"{command!r}: {process.returncode}, {process.stderr[-2000:]!r}, {process.stdout[-2000:]!r}")
    return elapsed, json.loads(process.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("sail", "dive", "module", "config", "edited-module", "edited-config", "false-config"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--trials", type=int, default=5)
    parser.add_argument("--require-strengthening", action="store_true",
                        help="also require fresh induction on the edited invariants alone to be inconclusive")
    parser.add_argument("--time-limit", type=int, default=600, help="seconds per process")
    args = parser.parse_args()
    if min(args.trials, args.time_limit) < 1:
        parser.error("positive trials and time limit required")
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)

        def check(folder, module, cfg, code=0):
            return sample([
                args.sail.resolve(), "runtime", "symbolic-check", "--dive", args.dive.resolve(),
                "--module", module.resolve(), "--config", cfg.resolve(), "--session", folder,
                "--time-limit", str(args.time_limit),
            ], code)
        trials = []
        for i in range(args.trials):
            session = directory / f"shared-{i}"
            cold, base = check(session, args.module, args.config)
            if base["verdict"] != "success":
                raise ValueError(base)
            candidate = session / "sail-symbolic-predicate-v1.json"
            established = digest(candidate)
            def warm():
                return check(session, args.edited_module, args.edited_config)
            def fresh():
                return check(directory / f"fresh-{i}", args.edited_module, args.edited_config)
            if i % 2 == 0:
                warm_ms, proof = warm()
                fresh_ms, search = fresh()
            else:
                fresh_ms, search = fresh()
                warm_ms, proof = warm()
            if (proof.get("proof") != "inductive-safety" or any(k in proof for k in ("depth", "generated", "distinct"))
                    or search.get("verdict") != "success"):
                raise ValueError((proof, search))
            false_ms, violation = check(session, args.edited_module, args.false_config, 1)
            _, independent = check(directory / f"false-{i}", args.edited_module, args.false_config, 1)
            if (violation.get("trace_validated") is not True or independent.get("trace_validated") is not True
                    or violation.get("ending") != independent.get("ending")
                    or violation.get("depth") != independent.get("depth") or digest(candidate) != established):
                raise ValueError((violation, independent))
            direct = {}
            if args.require_strengthening:
                direct_ms, attempt = sample([
                    args.dive.resolve(), "check", args.edited_module.resolve(), "--config", args.edited_config.resolve(),
                    "--strategy", "symbolic", "--prove-inductive", "--time-limit", f"{args.time_limit}s", "--output", "json",
                ], 2)
                if attempt.get("proof") != "inconclusive":
                    raise ValueError(attempt)
                direct = dict(fresh_induction_ms=direct_ms, failed_obligation=attempt["failed_obligation"])
            trials.append(dict(**direct, establish_ms=cold, proof_ms=warm_ms, fresh_ms=fresh_ms,
                               refutation_ms=false_ms, candidate_bytes=candidate.stat().st_size,
                               fresh_distinct=search["distinct"], fresh_generated=search["generated"],
                               fresh_depth=search["depth"], violated=violation["ending"],
                               refutation_depth=violation["depth"]))
        inputs = {name: digest(path) for name, path in vars(args).items() if isinstance(path, Path)}
        print(json.dumps(dict(format="sail-symbolic-session-benchmark-v1", sha256=inputs,
                              wall_seconds=args.time_limit, trials=trials), indent=2))


if __name__ == "__main__":
    main()
