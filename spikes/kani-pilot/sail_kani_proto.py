#!/usr/bin/env python3
"""Thinnest "sail path" for Kani (issue 135 spike). Not product code.

    sail_kani_proto.py --root ROOT --package REL --cache DIR --harness H...
                       [--kani-arg ARG]... [--timeout SECONDS]

For each selected harness it computes a cache key, serves a stored completed
outcome on a hit, and on a miss runs direct `cargo kani --harness H --exact`
in a shadow copy of ROOT, reads Kani's `--export-json` document (unstable) and
stores the result. It prints one JSON document shaped after
`sail verify --output json` schema version 2, with `evidence_tier: "bounded"`
and per-backend attribution.

Key (namespace `kani-v1`), all exact bytes, no normalisation:
  - source digest: every file under ROOT except `target/` (path and bytes),
    which covers the package, its Cargo.lock and any patched path dependency;
  - package path within ROOT and the harness's full path;
  - Kani version, CBMC version and binary digest, Kani's Rust toolchain, host;
  - the extra Kani arguments, in order (unwind, solver, features, -Z flags).
The unwind bound written in a `#[kani::unwind]` attribute is source, so the
source digest binds it.

Only completed outcomes are stored. A timeout, a signal, a compile error, or
a log and export document that disagree, is reported as `unknown` and never
cached (open question 26).
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time

NAMESPACE = "kani-v1"


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def source_digest(root):
    """Digest of every file under root except build output, by path and bytes."""
    h = hashlib.sha256()
    count = 0
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in ("target", ".git"))
        for name in sorted(filenames):
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, root).encode()
            with open(path, "rb") as f:
                digest = hashlib.sha256(f.read()).digest()
            h.update(len(rel).to_bytes(8, "big") + rel + digest)
            count += 1
    return h.hexdigest(), count


def run_text(argv, **kw):
    return subprocess.run(argv, capture_output=True, text=True, check=False, **kw)


def tool_identity(env):
    """Exact Kani, CBMC and toolchain identities, as the tools report them."""
    kani = run_text(["cargo", "kani", "--version"], env=env).stdout.strip()
    version = kani.split()[3]
    base = env.get("KANI_HOME") or os.path.join(os.path.expanduser("~"), ".kani")
    home = os.path.join(base, f"kani-{version}")
    cbmc_bin = os.path.join(home, "bin", "cbmc")
    cbmc = run_text([cbmc_bin, "--version"], env=env).stdout.strip()
    with open(os.path.join(home, "rust-toolchain-version")) as f:
        toolchain = f.read().strip()
    rustc = run_text([os.path.join(home, "toolchain", "bin", "rustc"), "-vV"], env=env).stdout
    fields = dict(l.split(": ", 1) for l in rustc.splitlines() if ": " in l)
    with open(cbmc_bin, "rb") as f:
        cbmc_sha = hashlib.sha256(f.read()).hexdigest()
    return {
        "kani": version,
        "cbmc": cbmc,
        "cbmc_sha256": cbmc_sha,
        "toolchain": toolchain,
        "rustc_commit": fields.get("commit-hash", ""),
        "host": fields.get("host", ""),
    }


def parse(log, export):
    """One harness's Kani output -> completed outcome, or None.

    `export` is Kani's `--export-json` document for the run; `log` is the
    regular output, used only for its verdict line and digest. Both must
    agree on the verdict or nothing is concluded.
    """
    verdict = re.search(r"^VERIFICATION:- (SUCCESSFUL|FAILED)", log, re.M)
    results = (export or {}).get("verification_results", {}).get("results", [])
    if not verdict or len(results) != 1:
        return None
    result = results[0]
    if (result["status"] == "Success") != (verdict[1] == "SUCCESSFUL"):
        return None
    counts, failed = {}, []
    for c in result["checks"]:
        counts[c["status"]] = counts.get(c["status"], 0) + 1
        if c["status"] == "Failure":
            loc = c.get("location") or {}
            failed.append({
                "property_class": c.get("category"),
                "description": c.get("description"),
                "in_function": c.get("function"),
                "span": {
                    "file": loc.get("file"),
                    "line": int(loc["line"]) if loc.get("line") else None,
                    "column": int(loc["column"]) if loc.get("column") else None,
                },
            })
    if verdict[1] == "FAILED" and not failed:
        # Kani prints FAILED when CBMC or the solver itself broke (observed
        # with --solver z3, cvc5 and bitwuzla: no failed check, or every
        # check `Error`). That is a tool error, not an outcome.
        return None
    unwinding = [c for c in failed if c["property_class"] == "unwind"]
    name = result["harness_id"]
    meta = next((h for h in export.get("harness_metadata", []) if h["pretty_name"] == name), {})
    cbmc = next((c for c in export.get("cbmc", []) if c["harness_id"] == name), {})
    config = cbmc.get("configuration") or {}
    return {
        "kani_verdict": verdict[1],
        "check_counts": counts,
        "failed_checks": failed,
        "bound_insufficient": bool(unwinding),
        "property_failures": len(failed) - len(unwinding),
        "harness_span": meta.get("source"),
        "solver": config.get("solver"),
        "object_bits": config.get("object_bits"),
        "kani_duration_ms": result.get("duration_ms"),
        "check_status_digest": sha256_bytes("\n".join(
            f'{c["id"]}\t{c.get("category")}\t{c.get("function")}\t{c["status"]}'
            for c in result["checks"]).encode()),
    }


def to_report(harness, outcome, source, identity, kani_args, key):
    """Map onto schema v2's function report, with the gaps made explicit."""
    module, _, function = harness.rpartition("::")
    if outcome is None:
        verdict, tier = "unknown", None
    elif outcome["kani_verdict"] == "SUCCESSFUL":
        verdict, tier = "verified", "bounded"
    elif outcome["property_failures"] > 0:
        # A refutation found within the bound is real whatever the bound.
        verdict, tier = "failed", "bounded"
    else:
        # Only unwinding assertions failed: the bound was too small to
        # conclude. Not a refutation and not a proof.
        verdict, tier = "unknown", None
    return {
        "function": function,
        "module": module,
        "fn_mode": None,
        "verdict": verdict,
        "establishment": "confirmed" if tier else "provisional",
        "evidence_tier": tier,
        "unstable": False,
        "verus_success": None,
        "provenance": {
            "mode": "default",
            "source": source,
            "backend": f'kani@{identity["kani"]}+cbmc@{identity["cbmc"]}',
            "toolchain": identity["toolchain"],
            "cache_key": key,
        },
        # Not in schema v2 today: the qualifiers a bounded verdict needs.
        "bounds": {
            "kani_args": kani_args,
            "bound_insufficient": bool(outcome and outcome["bound_insufficient"]),
        },
        "backend_outcome": outcome,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", required=True)
    ap.add_argument("--package", required=True, help="package dir relative to root")
    ap.add_argument("--cache", required=True)
    ap.add_argument("--harness", action="append", required=True)
    ap.add_argument("--kani-arg", action="append", default=[])
    ap.add_argument("--timeout", type=float, default=600.0)
    args = ap.parse_args()

    env = dict(os.environ, RUSTC_WRAPPER="")
    started = time.monotonic()
    identity = tool_identity(env)
    digest, files = source_digest(args.root)
    store = os.path.join(args.cache, NAMESPACE)
    os.makedirs(store, exist_ok=True)
    keyed = time.monotonic()

    reports, misses = [], []
    for harness in args.harness:
        material = [NAMESPACE, digest, args.package, harness, identity, args.kani_arg]
        key = sha256_bytes(json.dumps(material, sort_keys=True).encode())
        path = os.path.join(store, key + ".json")
        if os.path.exists(path):
            with open(path) as f:
                reports.append(to_report(harness, json.load(f)["outcome"], "cache", identity,
                                         args.kani_arg, key))
        else:
            misses.append((len(reports), harness, key, path, material))
            reports.append(None)

    if misses:
        # One shadow per source digest; the user's tree is never built in.
        shadow = os.path.join(args.cache, "shadow", digest[:16])
        if not os.path.isdir(shadow):
            shutil.copytree(args.root, shadow + ".tmp",
                            ignore=shutil.ignore_patterns("target", ".git"))
            os.rename(shadow + ".tmp", shadow)
        for index, harness, key, path, material in misses:
            export_path = os.path.join(args.cache, f"export-{key[:16]}.json")
            argv = ["cargo", "kani", "--harness", harness, "--exact", "-Z", "unstable-options",
                    "--export-json", export_path, *args.kani_arg]
            outcome = None
            try:
                done = run_text(argv, cwd=os.path.join(shadow, args.package), env=env,
                                timeout=args.timeout)
                export = None
                if os.path.exists(export_path):
                    with open(export_path) as f:
                        export = json.load(f)
                    os.remove(export_path)
                outcome = parse(done.stdout, export)
                if outcome is not None:
                    outcome["exit_code"] = done.returncode
                    outcome["log_sha256"] = sha256_bytes(done.stdout.encode())
            except subprocess.TimeoutExpired:
                outcome = None
            if outcome is not None:
                with open(path + ".tmp", "w") as f:
                    json.dump({"key_material": material, "outcome": outcome}, f, sort_keys=True)
                os.rename(path + ".tmp", path)
            reports[index] = to_report(harness, outcome, "fresh", identity, args.kani_arg, key)

    json.dump({
        "schema_version": 2,
        "backend": {"name": "kani", **identity},
        "source_digest": digest,
        "source_files": files,
        "served": sum(r["provenance"]["source"] == "cache" for r in reports),
        "solved": sum(r["provenance"]["source"] == "fresh" for r in reports),
        "identity_and_key_s": round(keyed - started, 3),
        "wall_s": round(time.monotonic() - started, 3),
        "functions": reports,
    }, sys.stdout, indent=1, sort_keys=True)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
