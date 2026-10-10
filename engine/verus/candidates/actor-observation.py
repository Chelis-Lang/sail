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
    def __init__(self, command, output, prefix):
        # Prepare the thread before an actor exists. All sidecar I/O happens on
        # this daemon; it cannot block communicate or change the actor deadline.
        self.stop_event = threading.Event()
        self.bound = threading.Event()
        self.output = output / (prefix + '.actor-observations.jsonl')
        self.written = 0
        self.truncated = False
        self.solver_hashes = {}
        self.command = command
        self.process = None
        self.leader_start = None
        self.exit_code = None
        self.thread = threading.Thread(target=self.loop, daemon=True)
        self.thread.start()

    def bind(self, process, started):
        self.process = process
        self.group = process.pid
        self.started = started
        self.bound.set()  # No file/proc I/O on the actor-control thread.

    def leader(self):
        if self.process.poll() is not None:
            return False
        try:
            row = self.stat_text((pathlib.Path('/proc') / str(self.group) / 'stat').read_text())
        except (OSError, ValueError, IndexError):
            return False
        if row['ppid'] != os.getpid() or row['group'] != self.group:
            return False
        if self.leader_start is None:
            self.leader_start = row['starttime_ticks']
        if row['starttime_ticks'] != self.leader_start:
            return False
        # A process reaped by communicate can be numerically reused between
        # the first liveness check and /proc read. Never inspect its payload.
        return self.process.poll() is None

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
    def stat_text(raw):
        parts = raw[raw.rfind(')') + 2:].split()
        return {'state': parts[0], 'ppid': int(parts[1]), 'group': int(parts[2]),
                'utime_ticks': int(parts[11]), 'stime_ticks': int(parts[12]),
                'starttime_ticks': int(parts[19]), 'rss_pages': int(parts[21])}

    @staticmethod
    def read_at(directory, name, limit=MAX_ARGV_BYTES):
        with os.fdopen(os.open(name, os.O_RDONLY, dir_fd=directory), 'rb') as file:
            return file.read(limit)

    def process_row(self, directory):
        # Pin this /proc directory inode. Later PID reuse cannot redirect a
        # payload read to a replacement process through a pathname lookup.
        fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
        try:
            row = self.stat_text(self.read_at(fd, 'stat').decode())
            if row['group'] != self.group or not self.leader():
                os.close(fd)
                return None
            row['pid'] = int(directory.name)
            row['exe'] = os.readlink('exe', dir_fd=fd)
            arguments = self.read_at(fd, 'cmdline', MAX_ARGV_BYTES + 1)
            row['argv_truncated'] = len(arguments) > MAX_ARGV_BYTES
            row['argv'] = arguments[:MAX_ARGV_BYTES].decode(errors='replace').rstrip('\0').split('\0')
            row['wchan'] = self.read_at(fd, 'wchan').decode().strip()
            row['io'] = {key: int(value) for key, value in
                         (line.split(':', 1) for line in self.read_at(fd, 'io').decode().splitlines())}
            row['threads'] = []
            return row, fd
        except BaseException:
            os.close(fd)
            raise

    def sample(self):
        if not self.leader():
            return False
        rows = []
        thread_count = 0
        for directory in pathlib.Path('/proc').iterdir():
            if not directory.name.isdigit():
                continue
            try:
                owned = self.process_row(directory)
                if owned is None:
                    continue
                row, process_fd = owned
                # Payloads and task paths remain relative to the pinned original
                # process directory, never a reusable numeric PID pathname.
                task_root = pathlib.Path('/proc/self/fd') / str(process_fd) / 'task'

                for task in task_root.iterdir():
                    if thread_count == MAX_THREADS:
                        row['threads_truncated'] = True
                        break
                    try:
                        thread = self.stat_text((task / 'stat').read_text())
                        thread.update(tid=int(task.name), wchan=(task / 'wchan').read_text().strip())
                        row['threads'].append(thread)
                        thread_count += 1
                    except (OSError, ValueError, IndexError):
                        pass  # A thread exited between these read-only observations.
                if pathlib.Path(row['exe']).name == 'z3':
                    metadata = os.stat('exe', dir_fd=process_fd)
                    key = (row['exe'], metadata.st_dev, metadata.st_ino, metadata.st_size,
                           metadata.st_mtime_ns, metadata.st_ctime_ns)
                    if key not in self.solver_hashes and len(self.solver_hashes) < MAX_SOLVER_IDENTITIES:
                        digest = hashlib.sha256()
                        with os.fdopen(os.open('exe', os.O_RDONLY, dir_fd=process_fd), 'rb') as binary:
                            while data := binary.read(1024 * 1024):
                                digest.update(data)
                        self.solver_hashes[key] = digest.hexdigest()
                    row['solver_sha256'] = self.solver_hashes.get(key)
                    row['solver_identity_unavailable'] = key not in self.solver_hashes
                os.close(process_fd)
                process_fd = None
                rows.append(row)
                if len(rows) == MAX_ROWS:
                    self.write({'event': 'process-inventory-truncated', 'limit': MAX_ROWS})
                    break
            except (OSError, ValueError, IndexError):
                if 'process_fd' in locals() and process_fd is not None:
                    os.close(process_fd)
                    process_fd = None
                pass  # A process exited between observations; no inference of proof completion.
        if not self.leader():
            return False
        self.write({'event': 'sample', 'unix_ns': time.time_ns(), 'elapsed_ns': time.monotonic_ns() - self.started,
                    'leader_starttime_ticks': self.leader_start, 'processes': rows})
        return True

    def loop(self):
        self.bound.wait()
        if self.process is None:
            return
        arguments = []
        available = MAX_ARGV_BYTES
        for argument in self.command:
            encoded = str(argument).encode()
            arguments.append(encoded[:available].decode(errors='replace'))
            available = max(0, available - len(encoded))
            if available == 0:
                break
        try:
            with self.output.open('wb') as self.file:
                self.write({'event': 'actor-started', 'unix_ns': time.time_ns(),
                            'monotonic_ns': self.started, 'pid': self.group,
                            'process_group': self.group, 'argv': arguments,
                            'argv_truncated': available == 0,
                            'clock_ticks_per_second': os.sysconf('SC_CLK_TCK') if pathlib.Path('/proc').is_dir() else None,
                            'linux_proc_available': pathlib.Path('/proc').is_dir()})
                if pathlib.Path('/proc').is_dir():
                    while not self.stop_event.is_set():
                        try:
                            if not self.sample():
                                self.write({'event': 'observation-stopped', 'reason': 'owned leader exited, disappeared or changed identity'})
                                break
                        except (OSError, ValueError, IndexError) as error:
                            self.write({'event': 'observation-unavailable', 'reason': str(error)})
                            break
                        self.stop_event.wait(5)
                self.write({'event': 'actor-ended', 'unix_ns': time.time_ns(),
                            'elapsed_ns': time.monotonic_ns() - self.started,
                            'exit_code': self.exit_code,
                            'leader_starttime_ticks': self.leader_start})
        except (OSError, ValueError):
            pass  # Diagnostic I/O is unavailable; proof outcome is independent.

    def close(self, exit_code):
        self.exit_code = exit_code
        self.stop_event.set()
        self.bound.set()
        self.thread.join(timeout=2)
        # A blocked diagnostic daemon exits with the script. The actor has
        # already completed or been killed/drained at its original deadline.
