#!/usr/bin/env python3
"""Compare cold, portable predicate and authenticated closure checks across public spec edits."""

import argparse
from hashlib import sha256
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


PIN = "6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f"


def change(text, old, new):
    if text.count(old) != 1:
        raise ValueError(f"expected one {old!r}")
    return text.replace(old, new)


def histories(examples):
    store = (examples / "KeyValueStore/KeyValueStore.tla").read_text()
    kv_main = (examples / "KeyValueStore/MCKVS.tla").read_text()
    commit = (examples / "transaction_commit/TCommit.tla").read_text()
    for text, expected in [
        (store, "0306e7659212ef214fdb40012f1f24a78b25669292a80127c815da5eec466b07"),
        (kv_main, "4d66a40abeb29558b473770b736dc1bd16c841d9ac847b4644b36cd47d8c8962"),
        (commit, "98974d8c5ee007f4f62188737c20c7dcdb05af099cc53a59c8b8651331fd925d"),
    ]:
        if sha256(text.encode()).hexdigest() != expected:
            raise ValueError(f"source differs from tlaplus/Examples at {PIN}")
    kv_cfg = """SPECIFICATION Spec
CONSTANTS Key = {k1} Val = {v1,v2} TxId = {t1,t2}
NoVal = NoVal
INVARIANTS TypeInvariant TxLifecycle
"""
    tc_cfg = """SPECIFICATION TCSpec
CONSTANT RM = {r1,r2,r3,r4,r5,r6,r7,r8}
INVARIANTS TCTypeOK TCConsistent
CHECK_DEADLOCK FALSE
"""
    kv_restrict = change(store, "OpenTx(t) ==", "OpenTx(t) == /\\ tx = {}")
    kv_expand = change(store, "    /\\ snapshotStore[t][k] = NoVal\n", "    /\\ snapshotStore[t][k] \\in Val \\cup {NoVal}\n")
    old = "/\\ snapshotStore' = [snapshotStore EXCEPT ![t] = [k \\in Key |-> NoVal]]"
    first, last = store.split("CloseTx(t) ==")
    kv_fault = change(first, old, "/\\ UNCHANGED snapshotStore") + "CloseTx(t) ==" + last
    tc_restrict = change(commit, 'rmState[rm] \\in {"prepared", "committed"}', 'rmState[rm] = "prepared"')
    tc_expand = change(commit, 'rmState[rm] \\in {"prepared", "committed"}',
                       'rmState[rm] # "aborted"')
    tc_fault = change(commit, "/\\ notCommitted\n", "/\\ TRUE\n")

    # Each top-level module imports the edited protocol. Only TC's thin MC
    # wrapper is added; the original definitions and state variables are intact.
    for label, entry, wrapper, imported, original, cfg, layout, prop, restricted, expanded, faulty, new_cfg in [
        ("KeyValueStore", "MCKVS", kv_main, "KeyValueStore", store, kv_cfg, "TypeInvariant", "TxLifecycle",
         kv_restrict, kv_expand, kv_fault, kv_cfg.replace("Val = {v1,v2}", "Val = {v1}")),
        ("TCommit", "MCCommit", "---- MODULE MCCommit ----\nEXTENDS TCommit\n====\n", "TCommit", commit, tc_cfg,
         "TCTypeOK", "TCConsistent", tc_restrict, tc_expand, tc_fault, tc_cfg.replace(",r8}", "}")),
    ]:
        yield label, entry, wrapper, imported, layout, [
            ("establish", original, cfg, prop, 0), ("unchanged", original, cfg, prop, 0),
            ("restrict", restricted, cfg, prop, 0), ("expand", expanded, cfg, prop, 0),
            ("constant-layout", expanded, new_cfg, prop, 0), ("property", expanded, new_cfg, layout, 0),
            ("fault", faulty, cfg, prop, 1), ("undo", original, cfg, prop, 0),
        ]


def check(args, module, cfg, folder, layout, prop, expected, key):
    env = dict(os.environ)
    env.pop("DIVE_SYMBOLIC_PROOF_KEY", None)
    if key:
        env["DIVE_SYMBOLIC_PROOF_KEY"] = str(key)
    start = time.monotonic_ns()
    proc = subprocess.run([args.sail, "runtime", "symbolic-check", "--dive", args.dive, "--module", module,
                           "--config", cfg, "--session", folder, "--type-invariant", layout, "--property-proof",
                           prop, "--time-limit", str(args.seconds)], env=env, capture_output=True)
    elapsed = round((time.monotonic_ns() - start) / 1e6, 3)
    if proc.returncode != expected:
        raise RuntimeError(f"{module.stem} {folder.name}: {proc.returncode}: {proc.stdout[-1000:]!r} {proc.stderr[-700:]!r}")
    row = json.loads(proc.stdout)
    if expected == 1:
        if row.get("trace_validated") is not True or row["ending"]["name"] != prop:
            raise ValueError("missing independently validated named violation")
        return dict(ms=elapsed, violated=prop, depth=row["depth"])
    if row.get("proof") != "inductive-safety" or row["property"] != prop:
        raise ValueError("missing named proof")
    return dict(ms=elapsed, worker_s=row["seconds"], total=row["transition_obligations"], reused=row["reused_obligations"],
                sweeps=row["closure_sweeps"])



def measure_kvs(args, root, key):
    label, entry, wrapper, imported, layout, history = next(histories(args.specifications))
    module = root / f"{entry}.tla"
    module.write_text(wrapper)
    cfg = module.with_suffix(".cfg")
    txids = ",".join(f"t{i + 1}" for i in range(args.kvs_transactions))
    for index in (0, 1, 2, 3, 7):
        name, source, config, prop, expected = history[index]
        (root / f"{imported}.tla").write_text(source)
        cfg.write_text(config.replace("Key = {k1}", "Key = {k1,k2}")
                       .replace("TxId = {t1,t2}", "TxId = {" + txids + "}"))
        for trial in range(1 if index == 0 else args.trials):
            modes = [True] if index == 0 else ([True, False] if trial % 2 else [False, True])
            for signed in modes:
                sample = check(args, module, cfg, root / "session", layout, prop, expected, key if signed else None)
                if index != 0 and sample["sweeps"] != 0:
                    raise ValueError("edited candidate required a fresh search")
                print(json.dumps(dict(model=label, transactions=args.kvs_transactions, trial=trial,
                                      edit=name, signed=signed, **sample)), flush=True)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("sail", "dive", "specifications"):
        parser.add_argument("--" + arg, required=True, type=Path)
    parser.add_argument("--seconds", type=int, default=60)
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--kvs-transactions", type=int,
                        help="grow the two-key KVS edit panel and establish its baseline only once")
    args = parser.parse_args()
    if min(args.seconds, args.trials, args.kvs_transactions if args.kvs_transactions is not None else 1) < 1:
        parser.error("positive trials and seconds required")
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        key = root / "key"
        key.touch(mode=0o600)
        key.write_bytes(os.urandom(32))
        if args.kvs_transactions is not None:
            return measure_kvs(args, root, key)
        for label, entry, wrapper, imported, layout, history in histories(args.specifications):
            for trial in range(args.trials):
                case = root / f"{label}-{trial}"
                case.mkdir()
                module = case / f"{entry}.tla"
                module.write_text(wrapper)
                cfg = module.with_suffix(".cfg")
                source = case / f"{imported}.tla"
                session = case / "session"
                for index, (name, text, config, prop, expected) in enumerate(history):
                    source.write_text(text)
                    cfg.write_text(config)
                    portable = case / f"portable-{index}"
                    if session.exists():
                        shutil.copytree(session, portable)
                    runs = {}
                    modes = [("authenticated", session, key), ("portable", portable, None),
                             ("cold", case / f"cold-{index}", None)]
                    if trial % 2:
                        modes.reverse()
                    for mode, folder, secret in modes:
                        runs[mode] = check(args, module, cfg, folder, layout, prop, expected, secret)
                    if expected:
                        if len({run["depth"] for run in runs.values()}) != 1:
                            raise ValueError("different control depths")
                    else:
                        if len({run["total"] for run in runs.values()}) != 1:
                            raise ValueError("different closure domains")
                        if runs["cold"]["reused"] or runs["portable"]["reused"]:
                            raise ValueError("unauthenticated obligation reuse")
                    print(json.dumps(dict(model=label, trial=trial, edit=name, property=prop, **runs)), flush=True)


if __name__ == "__main__":
    main()
