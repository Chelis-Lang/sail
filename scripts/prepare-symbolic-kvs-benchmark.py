#!/usr/bin/env python3
"""Prepare the two recorded TLA+ safety edits from an existing pinned Examples checkout."""
import argparse
from pathlib import Path
import shutil

CLEAN = r"""NoWrites == \A t \in tx : written[t] = {}
TxIsClean == \A t \in TxId \ tx : missed[t] = {}
"""
CONFLICT = r"""AtMostOneConflict == \A a \in tx : \A b \in tx :
    a /= b => ((missed[a] \cap written[a]) = {} \/ (missed[b] \cap written[b]) = {})
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("examples", type=Path)
    parser.add_argument("output", type=Path, help="new output directory")
    args = parser.parse_args()
    kvs = args.examples / "specifications/KeyValueStore"
    module = (kvs / "MCKVS.tla").read_text()
    medium = (kvs / "MCKVSSafetyMedium.cfg").read_text()
    keys = "{k1, k2, k3}"
    if module.count("====") != 1 or medium.count(keys) != 1 or medium.count("{t1, t2}") != 1:
        parser.error("unexpected input, use the Examples revision in the validation guide")
    args.output.mkdir(parents=True)
    for name, addition in (("clean", CLEAN), ("conflict", CLEAN + CONFLICT)):
        edit = args.output / name
        edit.mkdir()
        shutil.copyfile(kvs / "KeyValueStore.tla", edit / "KeyValueStore.tla")
        (edit / "MCKVS.tla").write_text(module.replace("====", addition + "===="))
        for count, base in ((3, medium), (5, medium.replace(keys, "{k1, k2, k3, k4, k5}"))):
            prop = "TxIsClean" if name == "clean" else "AtMostOneConflict"
            (edit / f"{count}-base.cfg").write_text(base)
            (edit / f"{count}-edit.cfg").write_text(base + f"INVARIANT {prop}\n")
            (edit / f"{count}-false.cfg").write_text(base + "INVARIANT NoWrites\n")
        if name == "conflict":
            third = medium.replace("{t1, t2}", "{t1, t2, t3}")
            (edit / "3-transactions.cfg").write_text(third + "INVARIANT AtMostOneConflict\n")


if __name__ == "__main__":
    main()
