#!/usr/bin/env python3
"""Spike analysis (diagnostic only; not a conformance rule).

analyze.py OUT PROJ...
  plain runs: JSON framing, per-line pretty-JSON grammar, and whether the
              digit-masked stream is a line interleaving of the digit-masked
              captures of tee round 1 of the same arm and project.
  tee runs:   sail-conformance-invocations check (exact bytes).
  arms:       compare stock/n with fork/n, and stock/n with stock/n+1.
"""
import json, re, subprocess, sys, hashlib
from pathlib import Path

TOOL = str(Path.home() / "sail/target/release/sail-conformance-invocations")
LINE = re.compile(rb'^ *(?:"(?:[^"\\]|\\.)*": )?(?:\{\}|\[\]|\{|\[|"(?:[^"\\]|\\.)*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null),?$|^ *[\]}],?$')
MASK = re.compile(rb"\d+")


def docs(data):
    text = data.decode()
    dec = json.JSONDecoder()
    off = n = 0
    while True:
        while off < len(text) and text[off].isspace():
            off += 1
        if off == len(text):
            return n, True
        try:
            _, off = dec.raw_decode(text, off)
        except ValueError:
            return n, False
        n += 1


def split(data):
    assert data == b"" or data.endswith(b"\n"), "unterminated"
    return data.split(b"\n")[:-1]


def interleaving(combined, parts, limit=2000):
    ids = {}
    seqs = [[ids.setdefault(l, len(ids)) for l in p] for p in parts if p]
    if sum(map(len, seqs)) != len(combined):
        return f"line count {len(combined)} vs {sum(map(len, seqs))}"
    frontier = {tuple([0] * len(seqs))}
    for number, line in enumerate(combined, 1):
        i = ids.get(line)
        nxt = set()
        if i is not None:
            for st in frontier:
                for k, s in enumerate(seqs):
                    if st[k] < len(s) and s[st[k]] == i:
                        nxt.add(st[:k] + (st[k] + 1,) + st[k + 1:])
        if not nxt:
            return f"line {number} unexplained: {line[:100]!r}"
        if len(nxt) > limit:
            return f"undecided at line {number}"
        frontier = nxt
    return None


def masked_parts(run):
    parts = []
    for d in sorted((run / "captures").iterdir()):
        meta = json.loads((d / "meta.json").read_text())
        if meta["package_name"] is None:
            continue
        parts.append([MASK.sub(b"N", l) for l in split((d / "stdout").read_bytes())])
    return parts


def main():
    out = Path(sys.argv[1])
    result = {}
    for proj in sys.argv[2:]:
        for arm in ("stock", "fork"):
            key = f"{proj}/{arm}"
            r = result[key] = {}
            # plain
            reference = out / "tee" / arm / proj / "1"
            ref = masked_parts(reference) if (reference / "exit").exists() else None
            plain = {"runs": 0, "exit0": 0, "framing_ok": 0, "docs": [], "bad_grammar_lines": 0,
                     "masked_multiset_ok": 0,
                     "masked_interleaving_ok": 0, "masked_failures": []}
            for run in sorted((out / "plain" / arm / proj).glob("*"), key=lambda p: int(p.name)):
                if not (run / "exit").exists():
                    continue
                data = (run / "stdout").read_bytes()
                plain["runs"] += 1
                plain["exit0"] += (run / "exit").read_text().strip() == "0"
                n, ok = docs(data)
                plain["framing_ok"] += ok
                plain["docs"].append(n)
                lines = split(data)
                plain["bad_grammar_lines"] += sum(1 for l in lines if not LINE.match(l))
                if ref is not None:
                    masked = [MASK.sub(b"N", l) for l in lines]
                    # Stock's func-details key order varies between runs, so
                    # only the multiset of masked lines is comparable there.
                    plain["masked_multiset_ok"] += sorted(masked) == sorted(l for part in ref for l in part)
                    # Same-shaped packages make the masked search explode; exact bytes (tee runs) do not.
                    why = interleaving(masked, ref) if proj == "proj2" else "skipped"
                    plain["masked_interleaving_ok"] += why is None
                    if why:
                        plain["masked_failures"].append(f"{run.name}: {why}"[:90])
            r["plain"] = plain
            # tee
            tee = {"runs": 0, "exit0": 0, "framing_ok": 0, "combined_docs": [], "explained": 0, "recovered_docs": [],
                   "incomplete_documents": 0, "contiguous": [], "parts": [], "max_frontier": 0, "failures": [],
                   "invocations": [], "probes": []}
            for run in sorted((out / "tee" / arm / proj).glob("*"), key=lambda p: int(p.name)):
                if not (run / "exit").exists():
                    continue
                tee["runs"] += 1
                tee["exit0"] += (run / "exit").read_text().strip() == "0"
                p = subprocess.run([TOOL, "check", str(run)], capture_output=True, text=True)
                if p.returncode == 2:
                    tee["failures"].append(f"{run.name}: {p.stderr.strip()}")
                    continue
                a = json.loads(p.stdout)["arm"]
                tee["framing_ok"] += a["combined_parses"]
                tee["combined_docs"].append(a["combined_documents"])
                tee["explained"] += a["stdout"]["explained"] and p.returncode == 0
                tee["recovered_docs"].append(a["documents"])
                tee["incomplete_documents"] += a["incomplete_documents"]
                tee["contiguous"].append(a["stdout"]["contiguous_parts"])
                tee["parts"].append(a["stdout"]["parts"])
                tee["invocations"].append(a["invocations"])
                tee["probes"].append(a["probes"])
                tee["max_frontier"] = max(tee["max_frontier"], a["stdout"]["max_frontier"])
                if not a["stdout"]["explained"]:
                    tee["failures"].append(f"{run.name}: {a['stdout']['failure']}")
            r["tee"] = tee
        # arms
        def compare(a, b):
            p = subprocess.run([TOOL, "compare", str(a), str(b)], capture_output=True, text=True)
            if p.returncode == 2:
                return {"error": p.stderr.strip()}
            return json.loads(p.stdout)
        for label, pairs in (
            ("stock-vs-fork", [(out / "tee/stock" / proj / str(n), out / "tee/fork" / proj / str(n)) for n in range(1, 21)]),
            ("stock-vs-stock", [(out / "tee/stock" / proj / str(n), out / "tee/stock" / proj / str(n + 1)) for n in range(1, 20)]),
            ("fork-vs-fork", [(out / "tee/fork" / proj / str(n), out / "tee/fork" / proj / str(n + 1)) for n in range(1, 20)]),
        ):
            agg = {"compared": 0, "same": 0, "pairs": set(), "pairs_with_documents": set(), "functions": set(),
                   "equal_arg_digests": set(), "stdout_byte_equal": set(), "stderr_byte_equal": set(),
                   "rlimit_drift": set(), "differing_keys": {}, "differences": []}
            for a, b in pairs:
                if not ((a / "exit").exists() and (b / "exit").exists()):
                    continue
                c = compare(a, b)
                agg["compared"] += 1
                if "error" in c:
                    agg["differences"].append(c["error"])
                    continue
                agg["same"] += c["same"]
                for k in ("pairs", "pairs_with_documents", "functions", "equal_arg_digests", "stdout_byte_equal",
                          "stderr_byte_equal", "rlimit_drift"):
                    agg[k].add(c["arms"][k])
                for k, v in c["arms"]["differing_keys"].items():
                    agg["differing_keys"][k] = agg["differing_keys"].get(k, 0) + v
                agg["differences"] += c["arms"]["differences"][:3]
            for k, v in agg.items():
                if isinstance(v, set):
                    agg[k] = sorted(v)
            agg["differences"] = agg["differences"][:10]
            result[f"{proj}/{label}"] = agg
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    main()
