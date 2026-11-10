#!/usr/bin/env python3
"""Turn `cargo xtask exec-mutants --sites` output into campaign.py mutant lists.

  exec_list.py --sites SITES.tsv --src SRC_ROOT --catalogue CAT.toml --name ironkve
               --out-all ALL.json --out-pick PICK.json [--count 30] [--near FILE:FUNCTION,...]

Every site becomes a find/replace edit over whole source lines (extended until
the find text is unique in its file). ALL.json holds every site that converts;
PICK.json holds COUNT of them: up to three sites of each `--near` function
(the functions closest to their resource limit in the baseline), then a
round-robin over the classes in seeded hash order, one function at most once
until every function with a site of that class has been used.
"""
import argparse
import hashlib
import json
import os

CLASSES = ["arith", "compare", "constant", "logic", "statement", "rare-region", "loop-bound", "off-by-one"]


def collapse(text):
    return " ".join(text.split())


def convert(src_root, loc, original, replacement):
    file, line, col = loc.rsplit(":", 2)
    line, col = int(line), int(col)
    with open(os.path.join(src_root, file), encoding="utf-8") as f:
        text = f.read()
    starts = [0]
    for i, ch in enumerate(text):
        if ch == "\n":
            starts.append(i + 1)
    start = starts[line - 1] + col - 1
    end = start
    if original:
        want = collapse(original)
        # The shortest span from `start` whose collapsed text is the original.
        while end <= len(text) and collapse(text[start:end]) != want:
            end += 1
        if end > len(text):
            return None
    # The site's own text never has spaces around it in the report.
    insert = replacement
    if not original and replacement:
        insert = " " + replacement
    first, last = line - 1, text.count("\n", 0, end)
    while True:
        a = starts[first]
        b = starts[last + 1] if last + 1 < len(starts) else len(text)
        find = text[a:b]
        if text.count(find) == 1:
            break
        if first > 0:
            first -= 1
        elif last + 1 < len(starts):
            last += 1
        else:
            return None
        if last + 1 < len(starts):
            last += 1
    replace = text[a:start] + insert + text[end:b]
    if replace == find:
        return None
    return file, line, find, replace


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--sites", required=True)
    p.add_argument("--src", required=True)
    p.add_argument("--catalogue", required=True)
    p.add_argument("--name", required=True)
    p.add_argument("--out-all", required=True)
    p.add_argument("--out-pick", required=True)
    p.add_argument("--count", type=int, default=30)
    p.add_argument("--near", default="")
    p.add_argument("--seed", default="20261109")
    a = p.parse_args()

    mutants, skipped, n = [], 0, {}
    for row in open(a.sites, encoding="utf-8"):
        cols = row.rstrip("\n").split("\t")
        loc, cls, function, original, replacement = cols[:5]
        got = convert(a.src, loc, original, replacement)
        if got is None:
            skipped += 1
            continue
        file, line, find, replace = got
        n[cls] = n.get(cls, 0) + 1
        mutants.append({
            "id": f"{a.name}-{cls}-{n[cls]:03d}", "file": file, "find": find, "replace": replace,
            "description": f"{cls}: `{original}` to `{replacement}` in `{function}` at {loc}",
            "class": cls, "function": function, "line": line, "mode": "exec", "site": loc,
            "catalogue_expected": None, "catalogue": a.catalogue,
            "order": hashlib.sha256(f"{a.seed}|{loc}|{cls}|{replacement}".encode()).hexdigest(),
        })
    json.dump({"catalogue": a.catalogue, "mutants": mutants}, open(a.out_all, "w"), indent=1)

    picked, used = [], set()
    for spec in filter(None, a.near.split(",")):
        file, function = spec.split(":")
        have = sorted((m for m in mutants if m["file"].endswith(file) and m["function"] == function),
                      key=lambda m: m["order"])
        classes = set()
        for m in have:
            if m["class"] not in classes and len(classes) < 3:
                classes.add(m["class"])
                m["near_rlimit"] = True
                picked.append(m)
                used.add((m["file"], m["function"]))
    queues = {c: sorted((m for m in mutants if m["class"] == c and m not in picked), key=lambda m: m["order"])
              for c in CLASSES}
    while len(picked) < a.count and any(queues.values()):
        for c in CLASSES:
            q = queues[c]
            if len(picked) >= a.count or not q:
                continue
            fresh = next((m for m in q if (m["file"], m["function"]) not in used), q[0])
            q.remove(fresh)
            used.add((fresh["file"], fresh["function"]))
            picked.append(fresh)
    json.dump({"catalogue": a.catalogue, "mutants": picked}, open(a.out_pick, "w"), indent=1)
    count = {}
    for m in picked:
        count[m["class"]] = count.get(m["class"], 0) + 1
    print(f"{len(mutants)} sites converted, {skipped} skipped; picked {len(picked)}: {count}; "
          f"{len({(m['file'], m['function']) for m in picked})} functions")


if __name__ == "__main__":
    main()
