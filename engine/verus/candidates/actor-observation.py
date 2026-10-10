#!/usr/bin/env python3
"""Bounded, read-only Linux process-group observations for source gate actors.

These samples explain deadlines; they never authorize a proof or change actor
priority, flags, lifetime, stdin/stdout, or the existing timeout. Other process
groups and process environments are not inspected.
"""
import hashlib
import json
import os
import pathlib
import threading
import time


MAX_ROWS = 256
MAX_THREADS = 4096
MAX_ARGV_BYTES = 65536
MAX_SOLVER_IDENTITIES = 64


class Monitor:
    def __init__(self, process, command, output, prefix):
        self.group = process.pid
        self.stop_event = threading.Event()
        self.output = output / (prefix + '.actor-observations.jsonl')
        self.written = 0
        self.truncated = False
        self.solver_hashes = {}
        self.started = time.monotonic_ns()
        self.file = self.output.open('wb')
        arguments = []
        available = MAX_ARGV_BYTES
        for argument in command:
            encoded = str(argument).encode()
            arguments.append(encoded[:available].decode(errors='replace'))
            available = max(0, available - len(encoded))
            if available == 0:
                break
        self.write({'event': 'actor-started', 'unix_ns': time.time_ns(), 'monotonic_ns': self.started,
                    'pid': process.pid, 'process_group': self.group,
                    'argv': arguments, 'argv_truncated': available == 0,
                    'clock_ticks_per_second': os.sysconf('SC_CLK_TCK') if pathlib.Path('/proc').is_dir() else None,
                    'linux_proc_available': pathlib.Path('/proc').is_dir()})
        self.thread = threading.Thread(target=self.loop, daemon=True)
        self.thread.start()

    def write(self, item):
        if self.truncated:
            return
        encoded = json.dumps(item, separators=(',', ':')).encode() + b'\n'
        if self.written + len(encoded) > 8 * 1024 * 1024 - 64:
            encoded = b'{"event":"observation-truncated","limit_bytes":8388608}\n'
            self.truncated = True
        self.file.write(encoded)
        self.file.flush()
        self.written += len(encoded)

    @staticmethod
    def stat(path):
        raw = path.read_text()
        parts = raw[raw.rfind(')') + 2:].split()
        return {'state': parts[0], 'ppid': int(parts[1]), 'group': int(parts[2]),
                'utime_ticks': int(parts[11]), 'stime_ticks': int(parts[12]),
                'starttime_ticks': int(parts[19]), 'rss_pages': int(parts[21])}

    def sample(self):
        rows = []
        thread_count = 0
        for directory in pathlib.Path('/proc').iterdir():
            if not directory.name.isdigit():
                continue
            try:
                row = self.stat(directory / 'stat')
                if row['group'] != self.group:
                    continue
                row['pid'] = int(directory.name)
                row['exe'] = os.readlink(directory / 'exe')
                with (directory / 'cmdline').open('rb') as cmdline:
                    arguments = cmdline.read(MAX_ARGV_BYTES + 1)
                row['argv_truncated'] = len(arguments) > MAX_ARGV_BYTES
                row['argv'] = arguments[:MAX_ARGV_BYTES].decode(errors='replace').rstrip('\0').split('\0')
                row['wchan'] = (directory / 'wchan').read_text().strip()
                row['io'] = {key: int(value) for key, value in
                             (line.split(':', 1) for line in (directory / 'io').read_text().splitlines())}
                row['threads'] = []
                for task in (directory / 'task').iterdir():
                    if thread_count == MAX_THREADS:
                        row['threads_truncated'] = True
                        break
                    try:
                        thread = self.stat(task / 'stat')
                        thread.update(tid=int(task.name), wchan=(task / 'wchan').read_text().strip())
                        row['threads'].append(thread)
                        thread_count += 1
                    except (OSError, ValueError, IndexError):
                        pass  # A thread exited between these read-only observations.
                if pathlib.Path(row['exe']).name == 'z3':
                    metadata = (directory / 'exe').stat()
                    key = (row['exe'], metadata.st_dev, metadata.st_ino, metadata.st_size,
                           metadata.st_mtime_ns, metadata.st_ctime_ns)
                    if key not in self.solver_hashes and len(self.solver_hashes) < MAX_SOLVER_IDENTITIES:
                        digest = hashlib.sha256()
                        with (directory / 'exe').open('rb') as binary:
                            while data := binary.read(1024 * 1024):
                                digest.update(data)
                        self.solver_hashes[key] = digest.hexdigest()
                    row['solver_sha256'] = self.solver_hashes.get(key)
                    row['solver_identity_unavailable'] = key not in self.solver_hashes
                rows.append(row)
                if len(rows) == MAX_ROWS:
                    self.write({'event': 'process-inventory-truncated', 'limit': MAX_ROWS})
                    break
            except (OSError, ValueError, IndexError):
                pass  # A process exited between observations; no inference of proof completion.
        self.write({'event': 'sample', 'unix_ns': time.time_ns(), 'elapsed_ns': time.monotonic_ns() - self.started,
                    'processes': rows})

    def loop(self):
        if not pathlib.Path('/proc').is_dir():
            return
        while not self.stop_event.is_set():
            try:
                self.sample()
            except (OSError, ValueError, IndexError) as error:
                self.write({'event': 'observation-unavailable', 'reason': str(error)})
            self.stop_event.wait(5)

    def close(self, exit_code):
        self.stop_event.set()
        self.thread.join(timeout=2)
        if self.thread.is_alive():
            # A blocked read-only diagnostic must not keep an owned actor alive
            # or extend its deadline. The daemon thread exits with this script.
            return
        self.write({'event': 'actor-ended', 'unix_ns': time.time_ns(), 'elapsed_ns': time.monotonic_ns() - self.started,
                    'exit_code': exit_code})
        self.file.close()
