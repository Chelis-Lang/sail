#!/usr/bin/env python3
"""Spike A3/A4 list helpers.

  order.py strat LIST.json OUT.json [--seed S] [--count N]
      Every mutant (or the first N) in stratified random order: a round-robin
      over the classes, each class in seeded hash order. Any prefix is then a
      class-stratified random sample, so a naive loop that stops early still
      leaves a sample to extrapolate from.
  order.py pick LIST.json OUT.json ID... [--fill N] [--seed S]
      The named mutants, then a stratified fill from the rest up to N.
"""
import hashlib
import json
import sys


def strat(mutants, seed):
    classes = {}
    for m in mutants:
        classes.setdefault(m["class"], []).append(m)
    for c in classes.values():
        c.sort(key=lambda m: hashlib.sha256(f"{seed}|{m['id']}".encode()).hexdigest())
    order = sorted(classes, key=lambda c: hashlib.sha256(f"{seed}|{c}".encode()).hexdigest())
    out = []
    while any(classes.values()):
        for c in order:
            if classes[c]:
                out.append(classes[c].pop(0))
    return out


def opt(args, name, default):
    if name in args:
        i = args.index(name)
        v = args[i + 1]
        del args[i:i + 2]
        return v
    return default


def main():
    args = sys.argv[1:]
    seed = opt(args, "--seed", "20261110")
    count = int(opt(args, "--count", "0"))
    fill = int(opt(args, "--fill", "0"))
    cmd, src, dst, *ids = args
    lst = json.load(open(src))
    ms = lst["mutants"]
    if cmd == "strat":
        out = strat(ms, seed)
        if count:
            out = out[:count]
    else:
        by = {m["id"]: m for m in ms}
        missing = [i for i in ids if i not in by]
        if missing:
            raise SystemExit(f"not found: {missing}")
        out = [by[i] for i in ids]
        rest = [m for m in strat(ms, seed) if m["id"] not in set(ids)]
        out += rest[:max(0, fill - len(out))]
    json.dump({"catalogue": lst["catalogue"], "mutants": out}, open(dst, "w"), indent=1)
    print(len(out), "mutants")


if __name__ == "__main__":
    main()
