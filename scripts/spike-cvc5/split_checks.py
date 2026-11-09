#!/usr/bin/env python3
"""SPIKE: flatten incremental SMT logs into one single-check script per check.

  split_checks.py OUT_DIR MAX LOG.smt2...

Each output holds every command active at a `(check-sat)` (push/pop scopes
resolved textually, original command bytes kept) followed by that check.
Commands that only produce output (get-model, get-info, echo, ...) are
dropped. Checks are taken round-robin across logs until MAX are written.
"""
import sys
from pathlib import Path


def commands(text):
    out, depth, start, i, n = [], 0, None, 0, len(text)
    while i < n:
        c = text[i]
        if c == ";":
            while i < n and text[i] != "\n":
                i += 1
            continue
        if c == '"':
            i += 1
            while i < n:
                if text[i] == '"':
                    if i + 1 < n and text[i + 1] == '"':
                        i += 2
                        continue
                    break
                i += 1
        elif c == "|":
            i += 1
            while i < n and text[i] != "|":
                i += 1
        elif c == "(":
            if depth == 0:
                start = i
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                out.append(text[start:i + 1])
        i += 1
    return out


def head(command):
    return command[1:].split(None, 1)[0].rstrip(")")


OUTPUT_ONLY = {"get-model", "get-info", "get-value", "get-unsat-core", "echo", "exit",
               "get-proof", "get-option", "get-assertions"}


def checks(path):
    stack = [[]]
    for command in commands(Path(path).read_text(errors="replace")):
        name = head(command)
        if name == "push":
            parts = command[1:-1].split()
            for _ in range(int(parts[1]) if len(parts) > 1 else 1):
                stack.append([])
        elif name == "pop":
            parts = command[1:-1].split()
            for _ in range(int(parts[1]) if len(parts) > 1 else 1):
                if len(stack) > 1:
                    stack.pop()
        elif name in ("check-sat", "check-sat-assuming"):
            yield [c for frame in stack for c in frame] + [command]
        elif name not in OUTPUT_ONLY:
            stack[-1].append(command)


def main():
    out, limit, logs = Path(sys.argv[1]), int(sys.argv[2]), sorted(sys.argv[3:])
    out.mkdir(parents=True, exist_ok=True)
    streams = [(Path(log).stem, checks(log)) for log in logs]
    written = 0
    while streams and written < limit:
        for item in list(streams):
            name, stream = item
            script = next(stream, None)
            if script is None:
                streams.remove(item)
                continue
            safe = "".join(ch if ch.isalnum() or ch in "-_." else "_" for ch in name)[:80]
            (out / f"c{written:03d}-{safe}.smt2").write_text("\n".join(script) + "\n")
            written += 1
            if written >= limit:
                break
    print(f"wrote {written} single-check scripts from {len(logs)} logs")


if __name__ == "__main__":
    main()
