#!/usr/bin/env python3
"""Sample a command's process-tree RSS, excluding and retaining Sail daemons.

Run with python3 -I. No environment variables are added to the measured child.
RSS is KiB on both platforms. Shared pages are counted once per process;
50 ms sampling can miss short-lived processes and peaks between samples.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading

INTERVAL = 0.05


def linux_processes(root=Path('/proc')):
    """Map pid to (parent, RSS KiB, start identity, argv). Vanished PIDs skip."""
    processes = {}
    page_kb = os.sysconf('SC_PAGE_SIZE') // 1024
    for path in root.iterdir():
        if not path.name.isdecimal():
            continue
        try:
            fields = (path / 'stat').read_text().rsplit(')', 1)[1].split()
            rss = int((path / 'statm').read_text().split()[1]) * page_kb
            argv = (path / 'cmdline').read_bytes().decode(errors='replace').split('\0')
            processes[int(path.name)] = (int(fields[1]), rss, fields[19], argv)
        except (OSError, ValueError, IndexError):
            # A process can disappear between any two reads. Identity prevents
            # a reused PID from inheriting a previous process's ownership.
            continue
    return processes


def mac_processes():
    result = subprocess.run(['ps', '-A', '-o', 'pid=,ppid=,rss=,lstart=,command='],
                            check=True, capture_output=True, text=True)
    processes = {}
    for line in result.stdout.splitlines():
        fields = line.split(None, 8)
        if len(fields) == 9:
            pid, parent, rss = map(int, fields[:3])
            processes[pid] = (parent, rss, ' '.join(fields[3:8]), fields[8].split())
    return processes


def processes():
    return linux_processes() if sys.platform.startswith('linux') else mac_processes()


def daemon(argv):
    return (len(argv) > 1 and Path(argv[0]).name == 'sail'
            and argv[1] in ('serve', '_serve-jobs'))


def load(path, default):
    try:
        return json.loads(path.read_text()) if path else default
    except (OSError, ValueError):
        return default


class Tracker:
    """Remember observed descendants even after they detach and reparent."""
    def __init__(self, known):
        self.owned = {}
        self.daemons = {int(pid): start for pid, start in known.items()}
        self.tree_peak = 0
        self.daemon_peak = 0
        self.samples = 0

    def sample(self, table, root):
        self.owned = {pid: start for pid, start in self.owned.items()
                      if pid in table and table[pid][2] == start}
        self.daemons = {pid: start for pid, start in self.daemons.items()
                        if pid in table and table[pid][2] == start}
        if root in table:
            self.owned.setdefault(root, table[root][2])
        # The table is not necessarily ordered parent before child.
        while True:
            added = {pid: entry[2] for pid, entry in table.items()
                     if entry[0] in self.owned and pid not in self.owned}
            if not added:
                break
            self.owned.update(added)
        for pid in self.owned:
            if daemon(table[pid][3]):
                self.daemons[pid] = table[pid][2]
        # Daemon workers belong to the daemon side, including processes whose
        # command names are not sail. Do not count them in both columns.
        daemon_tree = set(self.daemons)
        while True:
            added = {pid for pid, entry in table.items()
                     if entry[0] in daemon_tree and pid not in daemon_tree}
            if not added:
                break
            daemon_tree.update(added)
        self.tree_peak = max(self.tree_peak, sum(table[pid][1] for pid in self.owned
                                               if pid not in daemon_tree))
        self.daemon_peak = max(self.daemon_peak, sum(table[pid][1] for pid in daemon_tree))
        self.samples += 1


def largest_kb(value, platform):
    return int(value / 1024) if platform == 'darwin' else int(value)


def daemon_hwm(table, pids):
    """Per-process Linux lifetime HWM, separate from sampled summed RSS."""
    values = {}
    if sys.platform.startswith('linux'):
        for pid in pids:
            if pid not in table:
                continue
            try:
                for line in Path(f'/proc/{pid}/status').read_text().splitlines():
                    if line.startswith('VmHWM:'):
                        values[str(pid)] = int(line.split()[1])
            except OSError:
                pass
    return values


def run(args):
    known = load(args.state, {})
    if not isinstance(known, dict):
        known = {}
    errors = []
    try:
        table = processes()
    except (OSError, subprocess.SubprocessError) as error:
        errors.append(str(error))
        table = {}
    status = load(args.status, {})
    pid = (status.get('info') or {}).get('pid')
    if pid in table and daemon(table[pid][3]):
        known[str(pid)] = table[pid][2]
    tracker = Tracker(known)
    if args.snapshot:
        tracker.sample(table, None)
        result = {'daemon_peak_kb': tracker.daemon_peak,
                  'daemon_hwm_kb': daemon_hwm(table, tracker.daemons)}
    else:
        try:
            child = subprocess.Popen(args.command)
        except OSError as error:
            print(f'perf-mem: {error}', file=sys.stderr)
            return 127 if isinstance(error, FileNotFoundError) else 126
        # Forward termination so an interrupted wrapper never intentionally
        # leaves its measured command running. Preserve shell signal status.
        previous = {}
        def forward(signum, _frame):
            try:
                os.kill(child.pid, signum)
            except ProcessLookupError:
                pass
        for signum in (signal.SIGTERM, signal.SIGINT):
            previous[signum] = signal.signal(signum, forward)
        stopped = threading.Event()
        def sample():
            try:
                while True:
                    tracker.sample(processes(), child.pid)
                    if stopped.wait(INTERVAL):
                        break
            except (OSError, subprocess.SubprocessError) as error:
                errors.append(str(error))
        sampler = threading.Thread(target=sample)
        sampler.start()
        try:
            _, wait_status, usage = os.wait4(child.pid, 0)
            code = os.waitstatus_to_exitcode(wait_status)
            child.returncode = code
        finally:
            stopped.set()
            sampler.join()
            for signum, handler in previous.items():
                signal.signal(signum, handler)
        result = {'tree_peak_kb': tracker.tree_peak,
                  'largest_kb': largest_kb(usage.ru_maxrss, sys.platform),
                  'daemon_peak_kb': tracker.daemon_peak,
                  'interval_ms': 50, 'samples': tracker.samples, 'exit': code}
        if errors:
            result.update(tree_peak_kb=None, daemon_peak_kb=None, errors=errors)
    try:
        args.output.write_text(json.dumps(result) + '\n')
        if args.state:
            args.state.write_text(json.dumps(tracker.daemons) + '\n')
    except OSError as error:
        print(f'perf-mem: cannot retain memory observation: {error}', file=sys.stderr)
    return 0 if args.snapshot else code if code >= 0 else 128 - code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--state', type=Path)
    parser.add_argument('--status', type=Path)
    parser.add_argument('--snapshot', action='store_true')
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command[:1] == ['--']:
        args.command.pop(0)
    if not args.command and not args.snapshot:
        parser.error('a command is required after --')
    return run(args)


if __name__ == '__main__':
    sys.exit(main())
