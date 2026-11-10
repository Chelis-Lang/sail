#!/usr/bin/env python3
"""Spike diagnostic: classify every write(2) to fd 1 in an strace of one run.

writes.py RUN   (RUN holds `trace` from strace.sh and the run's `stdout`)

Writes to the run's stdout file are Cargo's; writes to a pipe are a
compiler process writing to Cargo. Reports whether each ends on a line
boundary and how many lines it carries.
"""
import json, re, sys
from pathlib import Path

run = Path(sys.argv[1])
CALL = re.compile(r'^(\d+) +write\(1<([^>]*)>, "((?:[^"\\]|\\.)*)"(\.\.\.)?, (\d+)\) += (-?\d+)')
RESUMED = re.compile(r'^(\d+) +<\.\.\. write resumed>')
UNFINISHED = re.compile(r'^(\d+) +write\(1<([^>]*)>, "((?:[^"\\]|\\.)*)"(\.\.\.)?, (\d+) <unfinished')
ESC = re.compile(r'\\(x[0-9a-f]{2}|[0-7]{1,3}|.)')


def unescape(s):
    def one(m):
        e = m.group(1)
        if e[0] == "x":
            return chr(int(e[1:], 16))
        if e[0] in "01234567":
            return chr(int(e, 8))
        return {"n": "\n", "t": "\t", "r": "\r"}.get(e, e)
    return ESC.sub(one, s).encode("latin-1")


file_w = {"writes": 0, "bytes": 0, "end_newline": 0, "one_line": 0, "short": 0, "truncated": 0, "pids": set()}
pipe_w = {"writes": 0, "bytes": 0, "end_newline": 0, "one_line": 0, "short": 0, "truncated": 0, "pids": set(),
          "max_write": 0, "multi_line_writes": 0}
rebuilt = bytearray()
pending = {}
unparsed = 0
for raw in open(run / "trace", errors="surrogateescape"):
    m = CALL.match(raw)
    if not m:
        u = UNFINISHED.match(raw)
        if u:
            pending[u.group(1)] = u
            continue
        r = RESUMED.match(raw)
        if r and r.group(1) in pending:
            u = pending.pop(r.group(1))
            ret = int(raw.rsplit("=", 1)[1].split()[0])
            pid, path, data, trunc, asked = u.group(1), u.group(2), u.group(3), u.group(4), int(u.group(5))
        else:
            if "write(" in raw or "resumed" in raw:
                unparsed += 1
            continue
    else:
        pid, path, data, trunc, asked, ret = m.group(1), m.group(2), m.group(3), m.group(4), int(m.group(5)), int(m.group(6))
    if ret < 0:
        continue
    b = unescape(data)[:ret]
    w = pipe_w if path.startswith("pipe:") else file_w if path.endswith("/stdout") else None
    if w is None:
        continue
    w["writes"] += 1
    w["bytes"] += ret
    w["pids"].add(pid)
    w["short"] += ret != asked
    w["truncated"] += bool(trunc) or len(b) != ret
    w["end_newline"] += b.endswith(b"\n")
    w["one_line"] += b.count(b"\n") == 1 and b.endswith(b"\n")
    if w is pipe_w:
        w["max_write"] = max(w["max_write"], ret)
        w["multi_line_writes"] += b.count(b"\n") > 1
    else:
        rebuilt += b
for w in (file_w, pipe_w):
    w["pids"] = len(w["pids"])
print(json.dumps({"run": str(run), "to_stdout_file": file_w, "to_pipes": pipe_w, "unparsed_write_lines": unparsed,
                  "file_writes_rebuild_stdout": bytes(rebuilt) == (run / "stdout").read_bytes(),
                  "stdout_bytes": (run / "stdout").stat().st_size}))
