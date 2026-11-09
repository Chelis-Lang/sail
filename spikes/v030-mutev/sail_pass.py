#!/usr/bin/env python3
"""Spike A1: one `sail verify --output json` per mutant, in one warmed copy.

    sail_pass.py --sail SAIL --engine DIR --workspace ORACLE_WORKER_DIR \
                 --catalogue GEN.toml --out DIR [--ids ID,ID,...]

ORACLE_WORKER_DIR is a working copy the oracle prepared (`oracle-<w>/`, with
the `stock-command.json` it wrote). The original is verified first (cold,
then the cache is shared by every mutant, as a campaign would run), then each
mutant is applied, verified and restored. Kept per step: stdout (the
structured result), stderr and the exit code. `summary.json` holds, for
every function Sail does not report verified, its labels and each check's
answer, verbatim `reason-unknown` and whether it was solved or served.
"""
import argparse
import json
import os
import subprocess
import time
import tomllib
from pathlib import Path

FLAGS = {"--output-json", "--time", "--no-report-long-running"}


def step(cmd, cwd, env, log):
    log.mkdir(parents=True, exist_ok=True)
    started = time.time()
    with open(log / "stdout.txt", "wb") as o, open(log / "stderr.txt", "wb") as e:
        p = subprocess.run(cmd, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=o, stderr=e,
                           check=False)
    (log / "exit.txt").write_text(f"{p.returncode}\n")
    return p.returncode, time.time() - started


def summarise(log):
    try:
        doc = json.loads((log / "stdout.txt").read_text())
    except ValueError as e:
        return {"error": f"no structured result: {e}"}
    out = {"exit_code": doc.get("exit_code"), "currentness": doc.get("currentness"),
           "error": doc.get("error"), "targets": []}
    for t in doc.get("targets") or []:
        rep = t.get("report") or {}
        if not t.get("requested"):
            continue
        fns = []
        for c in rep.get("crates") or []:
            for f in c.get("functions") or []:
                if f.get("verdict") == "verified":
                    continue
                fns.append({
                    "function": f.get("function"), "fn_mode": f.get("fn_mode"),
                    "verdict": f.get("verdict"), "establishment": f.get("establishment"),
                    "evidence_tier": f.get("evidence_tier"), "unstable": f.get("unstable"),
                    "verus_success": f.get("verus_success"), "provenance": f.get("provenance"),
                    "timing": f.get("timing"),
                    "obligations": [{
                        "id": o.get("id"), "kind": o.get("kind"), "advisory": o.get("advisory"),
                        "outcome": o.get("outcome"), "unstable": o.get("unstable"),
                        "span": (o.get("span") or {}).get("text"),
                        "checks": o.get("checks"),
                        "has_counterexample": o.get("counterexample") is not None,
                    } for o in f.get("obligations") or []],
                })
        out["targets"].append({
            "target": t.get("target"), "complete": t.get("complete"), "reused": t.get("reused"),
            "report_exit_code": rep.get("exit_code"), "attribution": rep.get("attribution"),
            "verus": [(c.get("name"), c.get("verus")) for c in rep.get("crates") or []],
            "summary": rep.get("summary"),
            "diagnostics": [(d.get("level"), d.get("message"), d.get("function"))
                            if isinstance(d, dict) else d for d in rep.get("diagnostics") or []],
            "not_verified": fns,
        })
    return out


def main():
    ap = argparse.ArgumentParser()
    for a in ("sail", "engine", "workspace", "catalogue", "out"):
        ap.add_argument(f"--{a}", required=True, type=Path)
    ap.add_argument("--ids", default="")
    a = ap.parse_args()
    stock = json.loads((a.workspace / "stock-command.json").read_text())
    args = stock["args"]
    assert args[:2] == ["verus", "verify"], args
    cmd = [str(a.sail.resolve()), "verify", "--output", "json",
           *[x for x in args[2:] if x not in FLAGS]]
    if cmd[-1] == "--":
        cmd = cmd[:-1]
        if cmd[-2:] == ["--fwd-verus-args-to", "roots"]:
            cmd = cmd[:-2]
    out = a.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("SAIL_", "VERUS_")) and k not in ("RUSTC_WRAPPER", "CARGO_TARGET_DIR")}
    for k, v in stock["env"].items():
        if v is None:
            env.pop(k, None)
        elif not k.startswith(("VERUS_", "SAIL_")):
            env[k] = v
    env.update(SAIL_CACHE_DIR=str(out / "cache"), SAIL_VERUS_BUILD=str(a.engine.resolve()),
               SAIL_DAEMON_IDLE_TIMEOUT="0", CARGO_TARGET_DIR=str(Path(stock["root"]) / "target-sail"))
    cwd, src = Path(stock["cwd"]), Path(stock["src"])
    (out / "command.json").write_text(json.dumps({"cmd": cmd, "cwd": str(cwd)}))
    summary = {}
    code, wall = step(cmd, cwd, env, out / "logs" / "original")
    summary["original"] = {"exit": code, "wall_s": round(wall, 1), **summarise(out / "logs/original")}
    print("original", code, round(wall), flush=True)
    edits = tomllib.loads(a.catalogue.read_text())["edit"]
    wanted = set(filter(None, a.ids.split(",")))
    for e in edits:
        if wanted and e["id"] not in wanted:
            continue
        file = src / e["file"]
        original = file.read_text()
        if original.count(e["find"]) != 1:
            summary[e["id"]] = {"error": "the context does not occur exactly once"}
            continue
        file.write_text(original.replace(e["find"], e["replace"]))
        try:
            code, wall = step(cmd, cwd, env, out / "logs" / e["id"])
        finally:
            time.sleep(0.05)
            file.write_text(original)
        summary[e["id"]] = {"exit": code, "wall_s": round(wall, 1), **summarise(out / "logs" / e["id"])}
        print(e["id"], code, round(wall), flush=True)
        (out / "summary.json").write_text(json.dumps(summary, indent=1, sort_keys=True) + "\n")
    code, wall = step(cmd, cwd, env, out / "logs" / "reverted")
    summary["reverted"] = {"exit": code, "wall_s": round(wall, 1), **summarise(out / "logs/reverted")}
    (out / "summary.json").write_text(json.dumps(summary, indent=1, sort_keys=True) + "\n")
    status = subprocess.run([cmd[0], "status", "--json"], env=env, capture_output=True, text=True,
                            check=False).stdout
    try:
        pid = json.loads(status).get("info", {}).get("pid")
    except ValueError:
        pid = None
    if pid:
        subprocess.run(["kill", str(pid)], check=False)


if __name__ == "__main__":
    main()
