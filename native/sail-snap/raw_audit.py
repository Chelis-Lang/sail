#!/usr/bin/env python3
"""Bounded raw oracle for a snapshot before any output-producing command.

Records exact input, ordered stdout/stderr bytes and exit status for every
fresh, marked and resumed run. Nothing is masked or normalized. This scoped
probe does not qualify a corpus, cross-stream stdout/stderr interleaving,
or snapshots after an earlier reply. Such points are refused explicitly.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time


def digest(data):
    return hashlib.sha256(data).hexdigest()


def frame(data):
    return data.replace(b"\0", b"\0\0")


def difference(first, second):
    at = next((i for i, (a, b) in enumerate(zip(first, second)) if a != b), None)
    if at is None and len(first) != len(second):
        at = min(len(first), len(second))
    return dict(equal=first == second, first_difference=at,
                first_bytes=len(first), second_bytes=len(second))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--z3", required=True)
    parser.add_argument("--lib", required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--offset", type=int, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--resumes", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=5)
    args = parser.parse_args()
    if not 1 <= args.resumes <= 16 or not 0 < args.timeout <= 30:
        parser.error("requires 1..16 resumes and timeout in (0,30] seconds")
    data = args.input.read_bytes()
    if len(data) > 2 * 1024 * 1024 or not 0 <= args.offset <= len(data):
        parser.error("input exceeds 2MiB or offset is out of range")
    args.out.mkdir(parents=True, exist_ok=False)
    z3, library = str(Path(args.z3).resolve()), str(Path(args.lib).resolve())
    base_env = {k: v for k, v in os.environ.items() if not k.startswith("SAIL_SNAP")}
    preload = "DYLD_INSERT_LIBRARIES" if os.uname().sysname == "Darwin" else "LD_PRELOAD"
    base_env.pop(preload, None)
    work = Path(tempfile.mkdtemp(prefix="ss-raw-", dir="/tmp"))
    rows = []
    (args.out / "input.smt2").write_bytes(data)
    manifest = dict(runtime_sha256=digest(Path(z3).read_bytes()),
                    library_sha256=digest(Path(library).read_bytes()),
                    input_sha256=digest(data), prefix_sha256=digest(data[:args.offset]),
                    argv=[z3, "-smt2", "-in"], offset=args.offset,
                    platform=dict(zip(("system", "node", "release", "version", "machine"), os.uname())), rows=rows)

    def record(name, out, err, status, **extra):
        (args.out / (name + ".stdout")).write_bytes(out)
        (args.out / (name + ".stderr")).write_bytes(err)
        row = dict(name=name, stdout_sha256=digest(out), stderr_sha256=digest(err),
                   stdout_bytes=len(out), stderr_bytes=len(err), exit=status, **extra)
        rows.append(row)
        return out, err, status

    def run(name, input_bytes, env):
        # communicate drains both streams; timeouts kill/wait only this owned child.
        process = subprocess.Popen([z3, "-smt2", "-in"], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
        try:
            out, err = process.communicate(input_bytes, timeout=args.timeout)
            return record(name, out, err, process.returncode)
        except subprocess.TimeoutExpired:
            process.kill()
            out, err = process.communicate(timeout=2)
            record(name, out, err, process.returncode, error="timeout")
            raise

    def line(stream):
        out = bytearray()
        while len(out) <= 128:
            byte = stream.recv(1)
            if not byte:
                raise EOFError("status closed before newline")
            out.extend(byte)
            if byte == b"\n":
                return bytes(out)
        raise ValueError("status line exceeds bound")

    def drain(stream):
        out = bytearray()
        while chunk := stream.recv(65536):
            out.extend(chunk)
            if len(out) > 16 * 1024 * 1024:
                raise ValueError("output exceeds 16MiB bound")
        return bytes(out)

    def resume(index):
        streams = []
        child_pid = None
        exited = False
        try:
            nonce = f"{index:016x}"
            for role in "ies":
                stream = socket.socket(socket.AF_UNIX)
                stream.settimeout(args.timeout)
                streams.append(stream)
                stream.connect(str(work / "raw"))
                stream.sendall(f"SAILSNAP2 {role} {nonce}\n".encode())
            incoming, errors, status = streams
            ack = line(status)
            if not ack.startswith(b"ok "):
                raise ValueError(repr(ack))
            child_pid = int(ack[3:].strip())
            if child_pid <= 1:
                raise ValueError("invalid owned resumed child pid")
            # Drain stdout/stderr concurrently while writing; none is dropped.
            with ThreadPoolExecutor(max_workers=2) as pool:
                output = pool.submit(drain, incoming)
                error = pool.submit(drain, errors)
                incoming.sendall(frame(data[args.offset:]))
                incoming.shutdown(socket.SHUT_WR)
                out, err = output.result(), error.result()
                exit_line = line(status)
            if not exit_line.startswith(b"exit "):
                raise ValueError(repr(exit_line))
            code = int(exit_line[5:].strip())
            exited = True
            return record(f"resume-{index}", out, err, code,
                          status_ack=ack.decode(), status_exit=exit_line.decode())
        finally:
            if child_pid is not None and child_pid > 1 and not exited:
                # The pid came from this private server's authenticated group.
                # Bound interrupted queries too; never signal by process name.
                try:
                    os.kill(child_pid, 9)
                except ProcessLookupError:
                    pass
            for stream in streams:
                stream.close()

    try:
        prefix = run("prefix-eligibility", data[:args.offset], base_env)
        if prefix != (b"", b"", 0):
            raise ValueError("point is outside this probe's empty-output-prefix scope")
        first = run("fresh-1", data, base_env)
        second = run("fresh-2", data, base_env)
        env = base_env | {preload: library, "SAIL_SNAP_DIR_V2": str(work),
                          "SAIL_SNAP_FRAMING": "2", "SAIL_SNAP_READY": str(work / "ready"),
                          "SAIL_SNAP_TTL_S": "3"}
        marked = run("marked", frame(data[:args.offset]) + b"\0raw\0" + frame(data[args.offset:]), env)
        if (work / "ready").read_bytes() != b"sail-snap/3\n":
            raise ValueError("preload framing not established")
        manifest["snapshot_published"] = (work / "raw").is_socket()
        resumed = [resume(i) for i in range(args.resumes)]
        manifest["comparisons"] = [
            dict(name=name, stdout=difference(first[0], item[0]),
                 stderr=difference(first[1], item[1]), exit_equal=first[2] == item[2])
            for name, item in [("fresh-2", second), ("marked", marked)] +
                              [(f"resume-{i}", item) for i, item in enumerate(resumed)]
        ]
        manifest["all_raw_equal"] = all(
            c["stdout"]["equal"] and c["stderr"]["equal"] and c["exit_equal"]
            for c in manifest["comparisons"])
    except (OSError, ValueError, EOFError, subprocess.TimeoutExpired) as error:
        manifest["error"] = str(error)
        manifest["all_raw_equal"] = False
    finally:
        # Evict only this invocation's private snapshots; their 3s TTL is also
        # bounded if interrupted. No process-name signal or external cleanup.
        shutil.rmtree(work)
        (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({k: v for k, v in manifest.items() if k not in {"rows", "argv"}}))
    return 0 if manifest["all_raw_equal"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
