#!/usr/bin/env python3
"""Correctness-only preload regression checks, using the pinned Z3.

python3 native/sail-snap/test.py --lib LIB --z3 Z3
Runs no performance measurement. Each test owns its socket directory; the
finally block evicts its snapshots, whose short TTL also bounds interruption.
"""

import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time


def frame(data):
    return data.replace(b"\0", b"\0\0")


def marker(name):
    return b"\0" + name.encode() + b"\0"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lib", required=True)
    parser.add_argument("--z3", required=True)
    parser.add_argument("--sail", help="optional Sail daemon executable for shim fallback checks")
    parser.add_argument("--shim", help="optional rebuilt sail-z3 (requires --sail)")
    args = parser.parse_args()
    library = str(Path(args.lib).resolve())
    z3 = str(Path(args.z3).resolve())
    root = Path(tempfile.mkdtemp(prefix="ss2-", dir="/tmp"))
    rows = []
    preload = "DYLD_INSERT_LIBRARIES" if os.uname().sysname == "Darwin" else "LD_PRELOAD"

    def environment(directory, count="2", memory=str(1024 * 1024 * 1024)):
        directory.mkdir(mode=0o700, exist_ok=True)
        # Legacy DIR is supplied only in this negative-control test, so the
        # pre-fix library also arms and demonstrates the original defects.
        return dict(os.environ, **{preload: library}, SAIL_SNAP_DIR=str(directory),
                    SAIL_SNAP_DIR_V2=str(directory), SAIL_SNAP_FRAMING="2",
                    SAIL_SNAP_READY=str(directory / "ready"), SAIL_SNAP_TTL_S="2",
                    SAIL_SNAP_MAX_PARKED=count, SAIL_SNAP_MAX_BYTES=memory)

    def run(data, env=None, extra=()):
        return subprocess.run([z3, "-smt2", "-in", *extra], input=data, env=env,
                              capture_output=True, timeout=10)

    def sockets(directory):
        return [p for p in directory.iterdir() if p.name.startswith("s") and p.is_socket()]

    def check(name, condition, **details):
        rows.append(dict(test=name, passed=bool(condition), **details))

    try:
        probe = root / "state-probe"
        subprocess.run([os.environ.get("CC", "cc"), "-O2", "-Wall", "-Wextra", "-Werror",
                        "-pthread", str(Path(__file__).with_name("state_probe.c")), "-o", str(probe)],
                       check=True, timeout=20)
        cases = ["thread", "timer", "cpu", "descriptor"]
        if os.uname().sysname == "Linux":
            cases.append("posix-timer")
        for case in cases:
            directory = root / ("kernel-" + case)
            data = b"unchanged bytes\n"
            plain = subprocess.run([probe, case], input=data, capture_output=True, timeout=5)
            done = subprocess.run([probe, case], input=marker("sstate") + frame(data),
                                  env=environment(directory), capture_output=True, timeout=5)
            check("process-state-" + case, plain.returncode == 0 and
                  (plain.returncode, plain.stdout, plain.stderr) ==
                  (done.returncode, done.stdout, done.stderr) and not sockets(directory),
                  parked=len(sockets(directory)), plain_exit=plain.returncode,
                  preload_exit=done.returncode, stdout_equal=plain.stdout == done.stdout,
                  stderr_equal=plain.stderr == done.stderr)

        for name, data in [
            ("nul-string", b'(echo "x\0y")\n(exit)\n'),
            ("nul-comment", b'; x\0y\n(echo "after")\n(exit)\n'),
            ("adjacent-nuls", b'; x\0\0\0y\n(echo "after")\n(exit)\n'),
            ("all-bytes", b'; ' + bytes(range(256)) + b'\n(exit)\n'),
        ]:
            plain = run(data)
            done = run(frame(data), environment(root / name))
            check(name, (plain.returncode, plain.stdout, plain.stderr) ==
                  (done.returncode, done.stdout, done.stderr),
                  plain_exit=plain.returncode, preload_exit=done.returncode,
                  stdout_equal=plain.stdout == done.stdout, stderr_equal=plain.stderr == done.stderr)

        def input_for(prefix):
            return b"".join(marker(f"s{prefix}{i}") + b"(push)\n(pop)\n" for i in range(8)) + b'(echo "done")\n(exit)\n'

        for name, prefix, extra in [
            ("idle-timeout-thread", (Path(__file__).resolve().parents[2] /
                                     "corpus/repros/z3-snapshot-timer-state/idle-timer.smt2").read_bytes(), ()),
            ("active-global-timeout-thread", b'', ('-T:30',)),
        ]:
            directory = root / name
            suffix = b'(echo "done")\n(exit)\n'
            plain = run(prefix + suffix, extra=extra)
            done = run(frame(prefix) + marker("sstate") + frame(suffix),
                       environment(directory), extra=extra)
            check(name, (plain.returncode, plain.stdout, plain.stderr) ==
                  (done.returncode, done.stdout, done.stderr) and not sockets(directory),
                  stdout_equal=plain.stdout == done.stdout,
                  stderr_equal=plain.stderr == done.stderr,
                  plain_exit=plain.returncode, preload_exit=done.returncode,
                  parked=len(sockets(directory)))

        directory = root / "output-descriptor"
        output_file = root / "solver-output.txt"
        prefix = f'(set-option :regular-output-channel "{output_file}")\n'.encode()
        suffix = b'(echo "still-open")\n(exit)\n'
        plain = run(prefix + suffix)
        plain_file = output_file.read_bytes()
        output_file.unlink()  # Both runs start with the same absent output path.
        done = run(frame(prefix) + marker("sdescriptor") + frame(suffix), environment(directory))
        check("solver-output-descriptor", plain_file == output_file.read_bytes() and
              (plain.returncode, plain.stdout, plain.stderr) ==
              (done.returncode, done.stdout, done.stderr) and not sockets(directory),
              parked=len(sockets(directory)), side_effect_equal=plain_file == output_file.read_bytes())

        directory = root / "count"
        done = run(input_for("one"), environment(directory))
        parked = len(sockets(directory))
        check("single-solver-count", done.returncode == 0 and done.stdout == b"done\n" and parked == 2,
              parked=parked, limit=2)

        streams = []
        def resume_check():
            for role in "ies":
                stream = socket.socket(socket.AF_UNIX)
                stream.settimeout(10)
                stream.connect(str(sockets(directory)[0]))
                stream.sendall(f"SAILSNAP2 {role} 0123456789abcdef\n".encode())
                streams.append(stream)
            incoming, errors, status = streams
            def line(stream):
                result = b""
                while not result.endswith(b"\n"):
                    byte = stream.recv(1)
                    if not byte:
                        raise EOFError("snapshot closed before its status line")
                    result += byte
                return result
            ack = line(status)
            rest = b'; genuine\0NUL\n(echo "resume")\n(exit)\n'
            incoming.sendall(frame(rest))
            incoming.shutdown(socket.SHUT_WR)
            out = b""
            while chunk := incoming.recv(65536):
                out += chunk
            err = b""
            while chunk := errors.recv(65536):
                err += chunk
            exit_line = line(status)
            for stream in streams:
                stream.close()
            plain = run(rest)
            check("resumed-nul", ack.startswith(b"ok ") and exit_line == b"exit 0\n" and
                  out == plain.stdout and err == plain.stderr, stdout_equal=out == plain.stdout,
                  stderr_equal=err == plain.stderr)
        try:
            resume_check()
        except (OSError, EOFError) as error:
            check("resumed-nul", False, error=str(error))
        finally:
            for stream in streams:
                stream.close()

        directory = root / "concurrent"
        env = environment(directory)
        children = [subprocess.Popen([z3, "-smt2", "-in"], stdin=subprocess.PIPE,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
                    for _ in range(4)]
        for i, child in enumerate(children):
            child.stdin.write(input_for(f"concurrent{i}"))
            child.stdin.close()
        results = []
        for child in children:
            out, err = child.stdout.read(), child.stderr.read()
            results.append(child.wait(timeout=10) == 0 and out == b"done\n" and not err)
        parked = len(sockets(directory))
        check("concurrent-global-count", all(results) and 0 < parked <= 2, parked=parked, limit=2)

        directory = root / "memory"
        done = run(input_for("memory"), environment(directory, memory="1"))
        parked = len(sockets(directory))
        check("resident-byte-cap", done.returncode == 0 and done.stdout == b"done\n" and parked == 0,
              parked=parked, limit_bytes=1)

        directory = root / "zero"
        done = run(input_for("zero"), environment(directory, count="0"))
        parked = len(sockets(directory))
        check("zero-count", done.returncode == 0 and done.stdout == b"done\n" and parked == 0,
              parked=parked, limit=0)

        if args.shim or args.sail:
            if not args.shim or not args.sail:
                raise ValueError("--shim and --sail must be supplied together")
            sock = root / "daemon.sock"
            daemon = subprocess.Popen([str(Path(args.sail).resolve()), "serve", "--socket", str(sock),
                                       "--cache", str(root / "cache")],
                                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                for _ in range(100):
                    if sock.exists():
                        break
                    time.sleep(0.05)
                for label, selected in [("armed", library), ("incompatible", str(root / "empty-library"))]:
                    if label == "incompatible":
                        Path(selected).write_bytes(b"not a preload library")
                    env = {key: value for key, value in os.environ.items() if not key.startswith("SAIL_")}
                    directory = root / ("shim-" + label)
                    (directory / "provenance").mkdir(parents=True)
                    env.update(SAIL_Z3_PATH=z3, SAIL_DAEMON_SOCKET=str(sock), SAIL_SNAPSHOT_LIB=selected,
                               SAIL_SNAPSHOT_DIR=str(directory), SAIL_PROVENANCE_DIR=str(directory / "provenance"),
                               SAIL_SNAPSHOT_TTL_S="2")
                    data = (f'(set-info :source |{label}|)\n(push)\n'.encode() +
                            b'(echo "x\0y")\n(exit)\n')
                    plain = run(data)
                    done = subprocess.run([str(Path(args.shim).resolve()), "-smt2", "-in"], input=data,
                                          env=env, capture_output=True, timeout=10)
                    observations = [json.loads(p.read_text()) for p in (directory / "provenance").glob("*.json")]
                    started = any(row.get("solver_started") for row in observations)
                    check("shim-" + label, started and (plain.returncode, plain.stdout, plain.stderr) ==
                          (done.returncode, done.stdout, done.stderr), solver_started=started,
                          stdout_equal=plain.stdout == done.stdout, stderr_equal=plain.stderr == done.stderr)
            finally:
                daemon.terminate()
                daemon.wait(timeout=10)
    finally:
        for directory in root.iterdir():
            if directory.is_dir():
                for path in sockets(directory):
                    path.unlink(missing_ok=True)
        # Bound test-owned parked processes even when a failed assertion or
        # malformed library interrupted publication. Do not signal other PIDs.
        time.sleep(0.5)
        shutil.rmtree(root)
    print(json.dumps(rows, indent=2))
    return 0 if all(row["passed"] for row in rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
