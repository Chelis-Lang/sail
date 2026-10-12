#!/usr/bin/env python3
"""L11 warm-session analysis (experimental, rescue lane L11).

Usage:
  l11.py measure ROOT --z3 Z3 [--jobs N]   offline replays (A setup, B emulation)
  l11.py report ROOT [--history-edit F] [--history-fast F]   tables (JSON + text)

ROOT is a `sail-bench warm-capture` output directory:
  ROOT/manifest.json
  ROOT/r<round>/<sequence>/<cycle>/{sail,fresh}/...
sail/:  <id>.timing.json + <id>.in.smt2   (cached sessions, shim L11 timing)
fresh/: <id>.in.smt2, <id>.out, <id>.jsonl (pass-through tap)
"""

import argparse
import collections
import hashlib
import json
import os
import re
import statistics
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor

SENTINEL = b'(echo "L11-SENTINEL")\n'
OUTPUT_HEADS = (b"check-sat", b"get-", b"echo", b"eval", b"simplify")


# ---------------------------------------------------------------- parsing

def split_commands(data):
    """Top-level commands as (start, end) byte offsets."""
    out = []
    i, n = 0, len(data)
    while i < n:
        c = data[i]
        if c in b" \t\r\n":
            i += 1
            continue
        if c == 0x3B:  # ;
            j = data.find(b"\n", i)
            i = n if j < 0 else j + 1
            continue
        if c != 0x28:  # not '(' : skip a stray token
            j = i
            while j < n and data[j] not in b" \t\r\n(":
                j += 1
            i = j
            continue
        depth, j = 0, i
        while j < n:
            d = data[j]
            if d == 0x22:  # string
                j += 1
                while j < n:
                    if data[j] == 0x22:
                        if j + 1 < n and data[j + 1] == 0x22:
                            j += 2
                            continue
                        break
                    j += 1
            elif d == 0x7C:  # |symbol|
                j = data.find(b"|", j + 1)
                if j < 0:
                    j = n
            elif d == 0x3B:
                k = data.find(b"\n", j)
                j = n - 1 if k < 0 else k
            elif d == 0x28:
                depth += 1
            elif d == 0x29:
                depth -= 1
                if depth == 0:
                    break
            j += 1
        out.append((i, min(j + 1, n)))
        i = j + 1
    return out


def head(data, cmd):
    s, e = cmd
    m = re.match(rb"\(\s*([^\s()]+)", data[s:min(e, s + 64)])
    return m.group(1) if m else b""


def produces_output(data, cmd):
    h = head(data, cmd)
    return any(h.startswith(p) for p in OUTPUT_HEADS)


def split_responses(out):
    """Responses: balanced s-expressions or single lines."""
    res = []
    i, n = 0, len(out)
    while i < n:
        c = out[i]
        if c in b" \t\r\n":
            i += 1
            continue
        if c == 0x28:
            depth, j = 0, i
            while j < n:
                d = out[j]
                if d == 0x22:
                    j += 1
                    while j < n:
                        if out[j] == 0x22:
                            if j + 1 < n and out[j + 1] == 0x22:
                                j += 2
                                continue
                            break
                        j += 1
                elif d == 0x7C:
                    k = out.find(b"|", j + 1)
                    j = n if k < 0 else k
                elif d == 0x28:
                    depth += 1
                elif d == 0x29:
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            res.append(out[i:j + 1])
            i = j + 1
        else:
            j = out.find(b"\n", i)
            j = n if j < 0 else j
            res.append(out[i:j])
            i = j + 1
    return res


class Stream:
    """A session input with its command structure."""

    def __init__(self, data):
        self.data = data
        self.cmds = split_commands(data)
        self.heads = [head(data, c) for c in self.cmds]
        self.level = []  # level before each command
        lvl = 0
        for h in self.heads:
            self.level.append(lvl)
            if h == b"push":
                lvl += 1
            elif h == b"pop":
                lvl = max(0, lvl - 1)
        self.checks = [i for i, h in enumerate(self.heads) if h == b"check-sat"]
        # blocks: (first cmd index, last cmd index) of level-0 push..pop
        self.blocks = []
        start = None
        for i, h in enumerate(self.heads):
            if h == b"push" and self.level[i] == 0:
                start = i
            elif h == b"pop" and self.level[i] == 1 and start is not None:
                self.blocks.append((start, i))
                start = None
        self.block_of = {}
        for b, (s, e) in enumerate(self.blocks):
            for i in range(s, e + 1):
                self.block_of[i] = b
        self.first_push = next((self.cmds[i][0] for i, h in enumerate(self.heads)
                                if h == b"push" and self.level[i] == 0), len(data))

    def signatures(self):
        """Per check: sha256 of its assertion stack (every state command in
        scope, in order, including options), like key v1."""
        levels = [hashlib.sha256()]
        sigs = []
        for i, h in enumerate(self.heads):
            if h == b"push":
                levels.append(levels[-1].copy())
                levels[-1].update(b"(push)")
                continue
            if h == b"pop":
                if len(levels) > 1:
                    levels.pop()
                continue
            if h == b"check-sat":
                sigs.append(levels[-1].hexdigest())
                continue
            if produces_output(self.data, self.cmds[i]):
                continue
            a, b = self.cmds[i]
            levels[-1].update(self.data[a:b])
        return sigs

    def boundary_at_or_before(self, offset):
        """Largest command start <= offset."""
        best = 0
        for s, _ in self.cmds:
            if s <= offset:
                best = s
            else:
                break
        return best

    def block_start_at_or_before(self, offset):
        best = 0
        for s, _e in self.blocks:
            o = self.cmds[s][0]
            if o <= offset:
                best = o
            else:
                break
        return best


def lcp(a, b):
    n = min(len(a), len(b))
    lo, hi = 0, n
    ma, mb = memoryview(a), memoryview(b)
    if ma[:n] == mb[:n]:
        return n
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if ma[:mid] == mb[:mid]:
            lo = mid
        else:
            hi = mid - 1
    return lo


# ---------------------------------------------------------------- z3 runs

def run_z3(z3, payload, timeout=600):
    """Feeds payload, returns [(response, seconds since spawn)] and total."""
    t0 = time.perf_counter()
    p = subprocess.Popen([z3, "-smt2", "-in"], stdin=subprocess.PIPE,
                         stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)

    def feed():
        try:
            p.stdin.write(payload)
            p.stdin.close()
        except Exception:
            pass

    th = threading.Thread(target=feed, daemon=True)
    th.start()
    out = b""
    stamps = []  # (len(out) after read, time)
    fd = p.stdout.fileno()
    while True:
        chunk = os.read(fd, 1 << 16)
        if not chunk:
            break
        out += chunk
        stamps.append((len(out), time.perf_counter() - t0))
    p.wait()
    total = time.perf_counter() - t0
    # time each response by the read that completed it
    res = split_responses(out)
    timed = []
    pos = 0
    si = 0
    for r in res:
        k = out.find(r, pos)
        end = k + len(r)
        pos = end
        while si < len(stamps) - 1 and stamps[si][0] < end:
            si += 1
        timed.append((r, stamps[si][1] if stamps else total))
    return timed, total


def time_prefix(z3, data, q):
    """Seconds from spawn until a fresh Z3 has consumed data[:q] (median of 3)."""
    vals = []
    for _ in range(3):
        timed, _total = run_z3(z3, data[:q] + SENTINEL + b"(exit)\n")
        for r, t in reversed(timed):
            if r.strip() == b"L11-SENTINEL":
                vals.append(t)
                break
    return statistics.median(vals) if vals else None


LABEL_RE = re.compile(rb"\(define-fun\s+(%%(?:global_)?location_label%%\d+)\s+\(\)\s+Bool\s+true\)")


def labels(reply):
    return sorted(set(m.decode() for m in LABEL_RE.findall(reply)))


def rlimit_count(reply):
    m = re.search(rb":rlimit-count\s+(\d+)", reply)
    return int(m.group(1)) if m else None


def check_facts(stream, replies_by_cmd):
    """Per check: answer, rlimit delta (stats around it), follow-up labels."""
    facts = []
    stats_before = None
    pending = None
    for i, h in enumerate(stream.heads):
        r = replies_by_cmd.get(i)
        if r is None:
            continue
        if h == b"get-info" and b":all-statistics" in stream.data[stream.cmds[i][0]:stream.cmds[i][1]]:
            c = rlimit_count(r)
            if pending is not None and pending["after"] is None:
                pending["after"] = c
            stats_before = c
        elif h == b"check-sat":
            pending = {"cmd": i, "answer": r.strip().decode(errors="replace"),
                       "before": stats_before, "after": None, "followups": [], "labels": []}
            facts.append(pending)
        elif pending is not None and pending["after"] is None and h != b"echo":
            pending["followups"].append(r)
            pending["labels"] = sorted(set(pending["labels"]) | set(labels(r)))
    for f in facts:
        f["rlimit"] = (f["after"] - f["before"]) if (f["after"] is not None and f["before"] is not None) else None
        f["followups"] = hashlib.sha256(b"\x00".join(f["followups"])).hexdigest()[:16]
    return facts


def replies_for(stream, out):
    res = split_responses(out)
    idx = [i for i in range(len(stream.cmds)) if produces_output(stream.data, stream.cmds[i])]
    return {i: r for i, r in zip(idx, res)}


# ---------------------------------------------------------------- loading

def load_root(root):
    path = os.path.join(root, "manifest.json")
    if os.path.exists(path):
        man = json.load(open(path))
        return man, man["cycles"]
    cycles = []
    for rd in sorted(os.listdir(root)):
        if not re.fullmatch(r"r\d+", rd):
            continue
        for seq in sorted(os.listdir(os.path.join(root, rd))):
            for cyc in sorted(os.listdir(os.path.join(root, rd, seq))):
                cycles.append({"round": int(rd[1:]), "sequence": seq, "cycle": cyc})
    return {}, cycles


def session_files(d, suffix):
    if not os.path.isdir(d):
        return []
    return sorted(f[: -len(suffix)] for f in os.listdir(d) if f.endswith(suffix))


def load_sail(d):
    out = []
    for sid in session_files(d, ".timing.json"):
        t = json.load(open(os.path.join(d, sid + ".timing.json")))
        for k in list(t):
            if k.endswith("_ns") and t[k] is not None:
                t[k] = int(t[k])
        t["id"] = sid
        t["input"] = os.path.join(d, sid + ".in.smt2")
        out.append(t)
    return out


def load_fresh(d):
    out = []
    for sid in session_files(d, ".jsonl"):
        checks, stats = [], []
        start = None
        for line in open(os.path.join(d, sid + ".jsonl")):
            r = json.loads(line)
            if r["type"] == "check":
                checks.append(r)
            elif r["type"] == "stats":
                stats.append(r)
        start_ns = int(sid.split("-")[1])
        out.append({"id": sid, "start_ns": start_ns, "checks": checks, "stats": stats,
                    "input": os.path.join(d, sid + ".in.smt2"),
                    "out": os.path.join(d, sid + ".out")})
    return out


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


# ---------------------------------------------------------------- measure

def measure(args):
    root = args.root
    man, cycles = load_root(root)
    z3 = args.z3
    tasks = []
    by_seq = {}
    for c in cycles:
        by_seq.setdefault((c["round"], c["sequence"]), []).append(c)
    results = {"A": {}, "B": {}}

    # --- A: exact-prefix setup per missed cached session
    a_jobs = []
    for (rnd, seq), cs in sorted(by_seq.items()):
        if seq.startswith("cold-"):
            continue
        prev_inputs = []
        for c in cs:
            d = os.path.join(root, f"r{rnd}", seq, c["cycle"], "sail")
            sess = load_sail(d)
            inputs = {s["id"]: open(s["input"], "rb").read() for s in sess}
            if c["cycle"] != "c00-initial":
                for s in sess:
                    if s.get("miss_ns") is None:
                        continue
                    data = inputs[s["id"]]
                    div = max((lcp(data, p) for p in prev_inputs), default=0)
                    a_jobs.append((rnd, seq, c["cycle"], s["id"], s["input"], div))
            prev_inputs.extend(inputs.values())
    print(f"A: {len(a_jobs)} missed sessions", file=sys.stderr)

    def do_a(job):
        rnd, seq, cyc, sid, path, div = job
        data = open(path, "rb").read()
        st = Stream(data)
        t = json.load(open(path[: -len(".in.smt2")] + ".timing.json"))
        miss_start = t.get("miss_start_offset") or len(data)
        q_div = st.boundary_at_or_before(min(div, miss_start))
        q_f = st.block_start_at_or_before(q_div)
        q_m = st.first_push if st.first_push <= q_div else 0
        r = {"round": rnd, "sequence": seq, "cycle": cyc, "id": sid, "divergence": div,
             "miss_start": miss_start, "q_div": q_div, "q_f": q_f, "q_m": q_m,
             "first_push": st.first_push, "bytes": len(data)}
        r["t_spawn"] = time_prefix(z3, data, 0)
        r["t_div"] = time_prefix(z3, data, q_div)
        r["t_miss"] = time_prefix(z3, data, miss_start) if miss_start != q_div else r["t_div"]
        r["t_f"] = time_prefix(z3, data, q_f) if q_f not in (q_div, 0) else (r["t_div"] if q_f == q_div else r["t_spawn"])
        r["t_m"] = time_prefix(z3, data, q_m) if q_m not in (0, q_f, q_div) else (
            r["t_spawn"] if q_m == 0 else (r["t_f"] if q_m == q_f else r["t_div"]))
        return r

    with ThreadPoolExecutor(args.jobs) as ex:
        for r in ex.map(do_a, a_jobs):
            results["A"][f'{r["round"]}/{r["sequence"]}/{r["cycle"]}/{r["id"]}'] = r

    # --- B: history-tolerant emulation on the traced round-0 fresh arm
    b_jobs = []
    for (rnd, seq), cs in sorted(by_seq.items()):
        if rnd != 0 or seq.startswith("cold-"):
            continue
        seen_v1, seen_v2 = set(), set()
        prev_inputs = []
        for c in cs:
            d = os.path.join(root, "r0", seq, c["cycle"], "fresh")
            sess = load_fresh(d)
            if c["cycle"] != "c00-initial":
                for s in sess:
                    keys2 = [k.get("prefix_key") for k in s["checks"]]
                    miss_at = next((i for i, k in enumerate(keys2) if k not in seen_v2), None)
                    if miss_at is None:
                        continue
                    new = [i for i, k in enumerate(s["checks"]) if k.get("key") not in seen_v1]
                    data = open(s["input"], "rb").read()
                    best = max(range(len(prev_inputs)), key=lambda j: lcp(data, prev_inputs[j]), default=None)
                    div = lcp(data, prev_inputs[best]) if best is not None else 0
                    b_jobs.append((seq, c["cycle"], s, miss_at, new, div,
                                   prev_inputs[best] if best is not None else None))
            for s in sess:
                for k in s["checks"]:
                    seen_v1.add(k.get("key"))
                    seen_v2.add(k.get("prefix_key"))
                prev_inputs.append(open(s["input"], "rb").read())
    print(f"B: {len(b_jobs)} missed sessions", file=sys.stderr)

    def do_b(job):
        seq, cyc, s, miss_at, new, div, prev = job
        data = open(s["input"], "rb").read()
        st = Stream(data)
        out = open(s["out"], "rb").read()
        ref = check_facts(st, replies_for(st, out))
        new_set = set(new)
        # check index -> block
        keep_blocks = set()
        for ci, cmd in enumerate(st.checks):
            if ci in new_set and cmd in st.block_of:
                keep_blocks.add(st.block_of[cmd])
        level0_new = [ci for ci in new if st.checks[ci] not in st.block_of] if new else []
        # build the B stream: every command outside blocks, and kept blocks
        parts, kept_cmds = [], []
        marker_at_m = None
        for i, (a, b) in enumerate(st.cmds):
            blk = st.block_of.get(i)
            if marker_at_m is None and st.heads[i] == b"push" and st.level[i] == 0:
                parts.append(SENTINEL)
                marker_at_m = True
            if blk is not None and blk not in keep_blocks:
                continue
            parts.append(data[a:b] + b"\n")
            kept_cmds.append(i)
        payload = b"".join(parts)
        if marker_at_m is None:
            payload += SENTINEL
        runs = [run_z3(z3, payload) for _ in range(3)]
        timed, _ = runs[0]
        total = statistics.median(t for _, t in runs)
        # map responses back to kept output commands (sentinel is first extra)
        resp = [r for r, _ in timed]
        times = [t for _, t in timed]
        try:
            k = next(i for i, r in enumerate(resp) if r.strip() == b"L11-SENTINEL")
        except StopIteration:
            k = None
        t_ms = []
        for tm, _ in runs:
            for r, t in tm:
                if r.strip() == b"L11-SENTINEL":
                    t_ms.append(t)
                    break
        t_m = statistics.median(t_ms) if t_ms else None
        resp_wo = [r for i, r in enumerate(resp) if i != k]
        times_wo = [t for i, t in enumerate(times) if i != k]
        out_cmds = [i for i in kept_cmds if produces_output(data, st.cmds[i])]
        rep = dict(zip(out_cmds, resp_wo))
        tim = dict(zip(out_cmds, times_wo))
        bf = {f["cmd"]: f for f in check_facts(st, rep)}
        rf = {f["cmd"]: f for f in ref}
        checks = []
        for ci in new:
            cmd = st.checks[ci]
            b, r = bf.get(cmd), rf.get(cmd)
            lim = s["checks"][ci].get("rlimit")
            checks.append({
                "check": ci,
                "fresh_answer": r and r["answer"], "b_answer": b and b["answer"],
                "fresh_rlimit": r and r["rlimit"], "b_rlimit": b and b["rlimit"],
                "limit": int(lim) if lim and lim.isdigit() else None,
                "fresh_labels": r and r["labels"], "b_labels": b and b["labels"],
                "followups_equal": (r and b and r["followups"] == b["followups"]),
                "b_time": tim.get(cmd),
                "fresh_solve_us": (s["checks"][ci]["answered_us"] - s["checks"][ci]["sent_us"])
                if s["checks"][ci].get("answered_us") is not None else None,
            })
        # B-long: a long-lived process that also ran the previous version of
        # each re-solved block (same index) just before it; only where the
        # previous version's stack before that block is the same bytes.
        long_facts = {}
        if prev is not None:
            old = Stream(prev)
            items = []
            for i, (a, b) in enumerate(st.cmds):
                blk = st.block_of.get(i)
                if blk is not None and blk not in keep_blocks:
                    continue
                if blk is not None and st.blocks[blk][0] == i and blk < len(old.blocks):
                    os_, oe = old.blocks[blk]
                    start_new = st.cmds[i][0]
                    start_old = old.cmds[os_][0]
                    if start_new == start_old and prev[:start_old] == data[:start_new]:
                        for j in range(os_, oe + 1):
                            items.append(("old", j))
                items.append(("new", i))
            pay = b"".join((data if src == "new" else prev)[st.cmds[j][0]:st.cmds[j][1]] + b"\n"
                           if src == "new" else prev[old.cmds[j][0]:old.cmds[j][1]] + b"\n"
                           for src, j in items)
            tm, _ = run_z3(z3, pay)
            outs = [(src, j) for src, j in items
                    if produces_output(data if src == "new" else prev,
                                       st.cmds[j] if src == "new" else old.cmds[j])]
            rep2 = {j: r for (src, j), (r, _) in zip(outs, tm) if src == "new"}
            long_facts = {f["cmd"]: f for f in check_facts(st, rep2)}
        for x in checks:
            lf = long_facts.get(st.checks[x["check"]])
            x["long_answer"] = lf and lf["answer"]
            x["long_rlimit"] = lf and lf["rlimit"]
            x["long_labels"] = lf and lf["labels"]
        return {"sequence": seq, "cycle": cyc, "id": s["id"], "sha256": sha(s["input"]),
                "checks_total": len(s["checks"]), "miss_at": miss_at, "new": new,
                "after_miss": len(s["checks"]) - miss_at,
                "unchanged_after_miss": sum(1 for i in range(miss_at, len(s["checks"])) if i not in new_set),
                "level0_new_checks": level0_new,
                "divergence": div, "first_push": st.first_push,
                "t_m": t_m, "t_total": total, "checks": checks}

    with ThreadPoolExecutor(args.jobs) as ex:
        for r in ex.map(do_b, b_jobs):
            results["B"][f'{r["sequence"]}/{r["cycle"]}/{r["id"]}'] = r

    # --- B over the live cached sessions: unchanged = stack signature seen
    # in an earlier cycle of the same round; B solves only the rest.
    results["B2"] = {}
    b2_jobs = []
    for (rnd, seq), cs in sorted(by_seq.items()):
        if seq.startswith("cold-"):
            continue
        seen = set()
        for c in cs:
            d = os.path.join(root, f"r{rnd}", seq, c["cycle"], "sail")
            sess = load_sail(d)
            sigs_all = {}
            for s in sess:
                st = Stream(open(s["input"], "rb").read())
                sigs_all[s["id"]] = st.signatures()
            if c["cycle"] != "c00-initial":
                for s in sess:
                    if s.get("miss_ns") is None:
                        continue
                    sg = sigs_all[s["id"]]
                    new = [i for i, x in enumerate(sg) if x not in seen]
                    b2_jobs.append((rnd, seq, c["cycle"], s, new))
            for v in sigs_all.values():
                seen.update(v)
    print(f"B2: {len(b2_jobs)} missed sessions", file=sys.stderr)

    def do_b2(job):
        rnd, seq, cyc, s, new = job
        data = open(s["input"], "rb").read()
        st = Stream(data)
        miss = s.get("miss_start_offset") or len(data)
        after = [ci for ci, cmd in enumerate(st.checks) if st.cmds[cmd][1] > miss]
        new_set = set(new)
        keep_blocks = set(st.block_of[st.checks[ci]] for ci in new if st.checks[ci] in st.block_of)
        parts, kept = [], []
        marked = False
        for i, (a, b) in enumerate(st.cmds):
            if not marked and st.heads[i] == b"push" and st.level[i] == 0:
                parts.append(SENTINEL)
                marked = True
            blk = st.block_of.get(i)
            if blk is not None and blk not in keep_blocks:
                continue
            parts.append(data[a:b] + b"\n")
            kept.append(i)
        payload = b"".join(parts) + (b"" if marked else SENTINEL)
        runs = [run_z3(z3, payload) for _ in range(3)]
        total = statistics.median(t for _, t in runs)
        t_ms = []
        for tm, _ in runs:
            for r, t in tm:
                if r.strip() == b"L11-SENTINEL":
                    t_ms.append(t)
                    break
        timed = [x for x in runs[0][0] if x[0].strip() != b"L11-SENTINEL"]
        outs = [i for i in kept if produces_output(data, st.cmds[i])]
        rep = {i: r for i, (r, _) in zip(outs, timed)}
        facts = check_facts(st, rep)
        lim = re.findall(rb":rlimit (\d+)", data)
        limit = max((int(x) for x in lim), default=0)
        return {"round": rnd, "sequence": seq, "cycle": cyc, "id": s["id"],
                "checks": len(st.checks), "after_miss": len(after),
                "new": len(new), "new_after_miss": len([c for c in after if c in new_set]),
                "unchanged_after_miss": len([c for c in after if c not in new_set]),
                "t_m": statistics.median(t_ms) if t_ms else None, "t_total": total,
                "first_push": st.first_push, "miss_start": miss,
                "answers": [f["answer"] for f in facts if f["cmd"] in set(st.checks[c] for c in new)],
                "rlimits": [f["rlimit"] for f in facts if f["cmd"] in set(st.checks[c] for c in new)],
                "limit": limit}

    with ThreadPoolExecutor(args.jobs) as ex:
        for r in ex.map(do_b2, b2_jobs):
            results["B2"][f'{r["round"]}/{r["sequence"]}/{r["cycle"]}/{r["id"]}'] = r

    json.dump(results, open(os.path.join(root, "measure.json"), "w"), indent=1)
    print("wrote measure.json", file=sys.stderr)


# ---------------------------------------------------------------- report

def catalogue(seq):
    if seq.startswith("loop-"):
        return "agent-loops"
    if seq.startswith("edits-"):
        return "edits"
    if seq.startswith("repair-"):
        return "repair " + seq[len("repair-"):]
    return seq


def pct(vals, p):
    vals = sorted(v for v in vals if v is not None)
    if not vals:
        return None
    k = (len(vals) - 1) * p
    lo, hi = int(k), min(int(k) + 1, len(vals) - 1)
    return vals[lo] + (vals[hi] - vals[lo]) * (k - lo)


def med(vals):
    return pct(vals, 0.5)


def critical_saving(sessions, deltas):
    """Cycle saving if each session's end moves earlier by its delta."""
    if not sessions:
        return 0.0
    old = max(s["end_ns"] for s in sessions)
    new = max(s["end_ns"] - deltas.get(s["id"], 0) for s in sessions)
    return max(0.0, (old - new) / 1e6)


def report(args):
    root = args.root
    man, cycles = load_root(root)
    m = json.load(open(os.path.join(root, "measure.json")))
    A, B = m["A"], m["B"]
    b_by_sha = {}
    for v in B.values():
        b_by_sha[(v["sequence"], v["cycle"], v["sha256"])] = v
    near = args.near
    # missed sessions that are a reordering of an earlier cycle's session
    # (same multiset of commands, different order: VI-0001)
    permuted = {}
    seen_sorted = {}
    for c in cycles:
        rnd, seq, cyc = c["round"], c["sequence"], c["cycle"]
        if seq.startswith("cold-"):
            continue
        acc = seen_sorted.setdefault((rnd, seq), set())
        cur = set()
        for s in load_sail(os.path.join(root, f"r{rnd}", seq, cyc, "sail")):
            data = open(s["input"], "rb").read()
            # byte histogram: invariant under VI-0001's reordering and the
            # label renumbering it causes; an edit almost surely changes it
            hist = collections.Counter(data)
            hs = hashlib.sha256(repr(sorted(hist.items())).encode()).hexdigest()
            cur.add(hs)
            if s.get("miss_ns") is not None and hs in acc:
                permuted[(rnd, seq, cyc, s["id"])] = (s["end_ns"] - s["miss_ns"]) / 1e6
        acc.update(cur)
    rows = []
    for c in cycles:
        rnd, seq, cyc = c["round"], c["sequence"], c["cycle"]
        if seq.startswith("cold-") or cyc == "c00-initial":
            continue
        arms = {a["arm"]: a for a in c.get("arms", [])}
        sail_arm = arms.get("sail", {})
        d = os.path.join(root, f"r{rnd}", seq, cyc, "sail")
        sess = load_sail(d)
        t0 = int(sail_arm.get("started_ns") or 0)
        missed = [s for s in sess if s.get("miss_ns") is not None]
        dA_up, dA_f, dA_m, dB = {}, {}, {}, {}
        setups_live, setups_off, solves = [], [], []
        b_matched, b_fallback = 0, 0
        unchanged_resolved_sail, unchanged_resolved_b = 0, 0
        for s in missed:
            a = A.get(f"{rnd}/{seq}/{cyc}/{s['id']}")
            live_setup = None
            if s.get("replay_done_ns"):
                live_setup = (s["replay_done_ns"] - s["miss_ns"]) / 1e6
            elif s.get("first_output_ns"):
                live_setup = (s["first_output_ns"] - s["miss_ns"]) / 1e6
            setups_live.append(live_setup)
            tail = (s["end_ns"] - s["miss_ns"]) / 1e6
            solves.append(tail - (live_setup or 0))
            if a:
                setups_off.append(a["t_miss"] * 1e3)
                dA_up[s["id"]] = a["t_miss"] * 1e9
                dA_f[s["id"]] = (a["t_f"] if a["q_f"] > 0 else a["t_spawn"]) * 1e9
                dA_m[s["id"]] = (a["t_m"] if a["q_m"] > 0 else a["t_spawn"]) * 1e9
            b = m.get("B2", {}).get(f"{rnd}/{seq}/{cyc}/{s['id']}")
            if b is not None:
                b_matched += 1
                lim = b["limit"] or 0
                fail = any(x != "unsat" for x in b["answers"]) or any(
                    lim and r is not None and r > near * lim for r in b["rlimits"]) or (
                    len(b["answers"]) < b["new"])
                unchanged_resolved_sail += b["unchanged_after_miss"]
                if fail:
                    b_fallback += 1
                    unchanged_resolved_b += b["unchanged_after_miss"]
                else:
                    div = a["divergence"] if a else 0
                    cost = (b["t_total"] - (b["t_m"] or 0)) if div >= b["first_push"] else b["t_total"]
                    dB[s["id"]] = max(0.0, tail * 1e6 - cost * 1e9)
        fail_ns = [s["first_failure_ns"] for s in sess if s.get("first_failure_ns")]
        first_fail = (min(fail_ns) - t0) / 1e6 if fail_ns and t0 else None
        first_fail_a = None
        if fail_ns and t0:
            s_f = min((s for s in sess if s.get("first_failure_ns")), key=lambda s: s["first_failure_ns"])
            first_fail_a = first_fail - dA_f.get(s_f["id"], 0) / 1e6
        pt = [v for k, v in permuted.items() if k[:3] == (rnd, seq, cyc)]
        rows.append({
            "permuted_misses": len(pt), "permuted_tail_ms": pt,
            "round": rnd, "catalogue": catalogue(seq), "sequence": seq, "cycle": cyc,
            "expected": c.get("expected"),
            "plain_ms": arms.get("plain", {}).get("wall_ms"),
            "sail_ms": sail_arm.get("wall_ms"),
            "sail_outcome": sail_arm.get("outcome"),
            "plain_outcome": arms.get("plain", {}).get("outcome"),
            "sessions": len(sess), "missed": len(missed),
            "solver_window_ms": ((max(s["end_ns"] for s in sess) - min(s["start_ns"] for s in sess)) / 1e6) if sess else None,
            "setup_live_ms": setups_live, "setup_offline_ms": setups_off, "after_setup_ms": solves,
            "sum_setup_ms": sum(x for x in setups_off if x),
            "save_A_upper_ms": critical_saving(sess, dA_up),
            "save_A_f_ms": critical_saving(sess, dA_f),
            "save_A_m_ms": critical_saving(sess, dA_m),
            "save_B_ms": critical_saving(sess, dB),
            "b_matched": b_matched, "b_fallback": b_fallback,
            "unchanged_resolved_sail": unchanged_resolved_sail,
            "unchanged_resolved_b": unchanged_resolved_b,
            "first_failure_ms_sail": first_fail, "first_failure_ms_A": first_fail_a,
        })
        # overlap of setup intervals among missed sessions
        iv = sorted((s["miss_ns"], s.get("replay_done_ns") or s.get("first_output_ns") or s["miss_ns"]) for s in missed)
        ov = 0
        for i in range(1, len(iv)):
            if iv[i][0] < max(e for _, e in iv[:i]):
                ov += 1
        rows[-1]["overlapping_setups"] = ov

    # plain first failure and unchanged checks re-solved under plain, from fresh r0
    plain_ff, plain_unchanged, checks_total = {}, {}, {}
    by_seq = {}
    for c in cycles:
        if c["round"] == 0 and not c["sequence"].startswith("cold-"):
            by_seq.setdefault(c["sequence"], []).append(c)
    for seq, cs in by_seq.items():
        seen = set()
        for c in cs:
            arms = {a["arm"]: a for a in c.get("arms", [])}
            fr = arms.get("fresh", {})
            sess = load_fresh(os.path.join(root, "r0", seq, c["cycle"], "fresh"))
            t0 = int(fr.get("started_ns") or 0)
            ff = []
            un = 0
            n = 0
            for s in sess:
                for k in s["checks"]:
                    n += 1
                    if k.get("key") in seen:
                        un += 1
                    if k.get("answer") not in (None, "unsat") and k.get("answered_us") is not None:
                        ff.append(s["start_ns"] + k["answered_us"] * 1000)
            plain_ff[(seq, c["cycle"])] = (min(ff) - t0) / 1e6 if ff and t0 else None
            plain_unchanged[(seq, c["cycle"])] = un
            checks_total[(seq, c["cycle"])] = n
            for s in sess:
                for k in s["checks"]:
                    seen.add(k.get("key"))
    for r in rows:
        r["first_failure_ms_plain"] = plain_ff.get((r["sequence"], r["cycle"]))
        r["unchanged_resolved_plain"] = plain_unchanged.get((r["sequence"], r["cycle"]))
        r["checks"] = checks_total.get((r["sequence"], r["cycle"]))

    # B verdict / label / rlimit agreement over new checks
    agree = {"checks": 0, "answer_equal": 0, "nonunsat": 0, "labels_equal": 0, "followups_equal": 0,
             "rlimit_ratio": [], "used_frac": [], "unsat_over_near": 0}
    for v in B.values():
        for x in v["checks"]:
            agree["checks"] += 1
            agree["answer_equal"] += x["fresh_answer"] == x["b_answer"]
            if x["fresh_answer"] != "unsat":
                agree["nonunsat"] += 1
                agree["labels_equal"] += x["fresh_labels"] == x["b_labels"]
                agree["followups_equal"] += bool(x["followups_equal"])
            if x["fresh_rlimit"] and x["b_rlimit"]:
                agree["rlimit_ratio"].append(max(x["fresh_rlimit"], x["b_rlimit"]) / min(x["fresh_rlimit"], x["b_rlimit"]))
            if x.get("long_answer") is not None:
                agree["long_checks"] = agree.get("long_checks", 0) + 1
                agree["long_answer_equal"] = agree.get("long_answer_equal", 0) + (x["long_answer"] == x["fresh_answer"])
                if x["fresh_answer"] != "unsat":
                    agree["long_nonunsat"] = agree.get("long_nonunsat", 0) + 1
                    agree["long_labels_equal"] = agree.get("long_labels_equal", 0) + (x["long_labels"] == x["fresh_labels"])
                if x["fresh_rlimit"] and x["long_rlimit"]:
                    lr = max(x["fresh_rlimit"], x["long_rlimit"]) / min(x["fresh_rlimit"], x["long_rlimit"])
                    agree.setdefault("long_ratio", []).append(lr)
            if x["limit"] and x["fresh_rlimit"] is not None:
                agree["used_frac"].append(x["fresh_rlimit"] / x["limit"])
                if x["b_answer"] == "unsat" and x["b_rlimit"] is not None and x["b_rlimit"] > near * x["limit"]:
                    agree["unsat_over_near"] += 1
    rr = agree.pop("rlimit_ratio")
    uf = agree.pop("used_frac")
    agree["rlimit_ratio_p50_p99_max"] = [pct(rr, 0.5), pct(rr, 0.99), max(rr) if rr else None]
    agree["rlimit_differs"] = sum(1 for x in rr if x != 1.0)
    lr = agree.pop("long_ratio", [])
    agree["long_ratio_p50_p99_max"] = [pct(lr, 0.5), pct(lr, 0.99), max(lr) if lr else None]
    agree["long_rlimit_differs"] = sum(1 for x in lr if x != 1.0)
    agree["used_frac_p50_p99_max"] = [pct(uf, 0.5), pct(uf, 0.99), max(uf) if uf else None]

    # per catalogue summary
    summary = {}
    for cat in sorted(set(r["catalogue"] for r in rows)):
        rs = [r for r in rows if r["catalogue"] == cat]
        cyc_ids = sorted(set((r["sequence"], r["cycle"]) for r in rs))
        setups = [x for r in rs for x in r["setup_offline_ms"]]
        live = [x for r in rs for x in r["setup_live_ms"]]
        sail = [r["sail_ms"] for r in rs]
        summary[cat] = {
            "cycles": len(cyc_ids), "samples": len(rs),
            "missed_buckets_per_cycle_median": med([r["missed"] for r in rs]),
            "missed_buckets_per_cycle_max": max(r["missed"] for r in rs),
            "setup_offline_ms_p50": med(setups), "setup_offline_ms_p90": pct(setups, 0.9),
            "setup_live_ms_p50": med(live), "setup_live_ms_p90": pct(live, 0.9),
            "after_setup_ms_p50": med([x for r in rs for x in r["after_setup_ms"]]),
            "plain_ms_median": med([r["plain_ms"] for r in rs]),
            "sail_ms_median": med(sail),
            "solver_window_ms_median": med([r["solver_window_ms"] for r in rs]),
            "sum_setup_ms_median": med([r["sum_setup_ms"] for r in rs]),
            "save_A_upper_ms_median": med([r["save_A_upper_ms"] for r in rs]),
            "save_A_upper_ms_max": max(r["save_A_upper_ms"] for r in rs),
            "save_A_f_ms_median": med([r["save_A_f_ms"] for r in rs]),
            "save_A_m_ms_median": med([r["save_A_m_ms"] for r in rs]),
            "save_B_ms_median": med([r["save_B_ms"] for r in rs]),
            "save_B_ms_max": max(r["save_B_ms"] for r in rs),
            "save_A_upper_pct_median": med([100 * r["save_A_upper_ms"] / r["sail_ms"] for r in rs if r["sail_ms"]]),
            "save_A_f_pct_median": med([100 * r["save_A_f_ms"] / r["sail_ms"] for r in rs if r["sail_ms"]]),
            "save_B_pct_median": med([100 * r["save_B_ms"] / r["sail_ms"] for r in rs if r["sail_ms"]]),
            "critical_share_A_upper": (sum(r["save_A_upper_ms"] for r in rs) / s) if (s := sum(r["sum_setup_ms"] for r in rs)) else None,
            "overlapping_setups_total": sum(r["overlapping_setups"] for r in rs),
            "b_matched": sum(r["b_matched"] for r in rs), "b_fallback": sum(r["b_fallback"] for r in rs),
            "missed_total": sum(r["missed"] for r in rs),
            "unchanged_resolved_plain_median": med([r["unchanged_resolved_plain"] for r in rs]),
            "unchanged_resolved_sail_median": med([r["unchanged_resolved_sail"] for r in rs]),
            "unchanged_resolved_b_median": med([r["unchanged_resolved_b"] for r in rs]),
            "checks_median": med([r["checks"] for r in rs]),
            "unchanged_resolved_plain_mean": sum(r["unchanged_resolved_plain"] or 0 for r in rs) / len(rs),
            "unchanged_resolved_sail_mean": sum(r["unchanged_resolved_sail"] for r in rs) / len(rs),
            "unchanged_resolved_b_mean": sum(r["unchanged_resolved_b"] for r in rs) / len(rs),
            "save_A_upper_ms_mean": sum(r["save_A_upper_ms"] for r in rs) / len(rs),
            "save_A_f_ms_mean": sum(r["save_A_f_ms"] for r in rs) / len(rs),
            "save_B_ms_mean": sum(r["save_B_ms"] for r in rs) / len(rs),
            "sail_ms_mean": sum(r["sail_ms"] or 0 for r in rs) / len(rs),
            "plain_ms_mean": sum(r["plain_ms"] or 0 for r in rs) / len(rs),
            "first_failure_ms": {k: med([r[k] for r in rs if r.get(k) is not None]) for k in
                                 ("first_failure_ms_plain", "first_failure_ms_sail", "first_failure_ms_A")},
            "failing_samples": sum(1 for r in rs if r.get("first_failure_ms_sail") is not None),
            "permuted_miss_samples": sum(1 for r in rs if r["permuted_misses"]),
            "permuted_miss_tail_ms_median": med([t for r in rs for t in r["permuted_tail_ms"]]),
        }
    out = {"near": near, "rows": rows, "summary": summary, "b_agreement": agree}
    json.dump(out, open(os.path.join(root, "report.json"), "w"), indent=1)
    print(json.dumps({"summary": summary, "b_agreement": agree}, indent=1))


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    m = sub.add_parser("measure")
    m.add_argument("root")
    m.add_argument("--z3", required=True)
    m.add_argument("--jobs", type=int, default=8)
    r = sub.add_parser("report")
    r.add_argument("root")
    r.add_argument("--near", type=float, default=0.01)
    args = ap.parse_args()
    if args.cmd == "measure":
        measure(args)
    else:
        report(args)


if __name__ == "__main__":
    main()
