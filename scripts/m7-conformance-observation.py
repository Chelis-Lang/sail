#!/usr/bin/env python3
"""Attach a bounded metadata-only Linux observer to one frozen conformance actor.

This script never starts, signals, waits for, or reads a stream from that actor.
It does not read argv, environment, source, configuration, or descriptor contents.
Its own duration/output limits have no effect on the harness's unit deadlines.
"""
import argparse
import json
import os
from pathlib import Path
import time

MAX_PROCESSES = 256
MAX_SCAN = 16384
MAX_THREADS = 512
MAX_FDS = 256
MAX_LINK_BYTES = 8192
MAX_RECORD_BYTES = 256 * 1024
MAX_OUTPUT_BYTES = 24 * 1024 * 1024


def stat(raw):
    """Parse comm with arbitrary spaces/parentheses using the closing delimiter."""
    fields = raw.rsplit(b') ', 1)[1].split()
    return {'state': fields[0].decode('ascii'), 'ppid': int(fields[1]),
            'pgid': int(fields[2]), 'utime_ticks': int(fields[11]),
            'stime_ticks': int(fields[12]), 'starttime_ticks': int(fields[19]),
            'rss_pages': int(fields[21])}


def read_at(fd, name, limit):
    child = os.open(name, os.O_RDONLY | os.O_NONBLOCK, dir_fd=fd)
    try:
        data = os.read(child, limit + 1)
        if len(data) > limit:
            raise ValueError('proc metadata exceeds its byte limit')
        return data
    finally:
        os.close(child)


def link_at(fd, name):
    text = os.readlink(name, dir_fd=fd)
    encoded = os.fsencode(text)
    if len(encoded) > MAX_LINK_BYTES:
        return {'text': encoded[:MAX_LINK_BYTES].decode(errors='replace'),
                'truncated': True}
    return {'text': text, 'truncated': False}


def descendants(rows, leader, start):
    """Only snapshot parent chains rooted in the exact pinned leader are owned."""
    if rows.get(leader, {}).get('starttime_ticks') != start:
        return []
    owned = {leader}
    while True:
        added = {pid for pid, row in rows.items() if pid not in owned and row['ppid'] in owned}
        if not added:
            break
        owned.update(added)
    return sorted(owned)


class Output:
    def __init__(self, path):
        self.file = path.open('xb')
        self.bytes = 0
        self.stopped = False

    def write(self, value):
        if self.stopped:
            return False
        encoded = json.dumps(value, ensure_ascii=True, separators=(',', ':')).encode() + b'\n'
        if len(encoded) > MAX_RECORD_BYTES or self.bytes + len(encoded) > MAX_OUTPUT_BYTES - 512:
            encoded = b'{"event":"observation-truncated","acceptance":false}\n'
            self.stopped = True
        self.file.write(encoded)
        self.file.flush()
        self.bytes += len(encoded)
        return not self.stopped

    def close(self):
        self.file.close()


class Observer:
    def __init__(self, pid, start, output):
        self.pid = pid
        self.start = start
        self.output = output
        self.root_fd = os.open(Path('/proc') / str(pid), os.O_RDONLY | os.O_DIRECTORY)
        self.initial = time.monotonic()
        try:
            self.assert_leader()
        except BaseException:
            os.close(self.root_fd)
            raise

    def row_at(self, fd):
        return stat(read_at(fd, 'stat', 4096))

    def assert_leader(self):
        row = self.row_at(self.root_fd)
        # A proc directory descriptor stays attached to the original task and
        # cannot redirect payload reads through numerical PID reuse.
        if row['starttime_ticks'] != self.start or row['state'] in ('Z', 'X'):
            raise ValueError('owned leader exited or identity is unavailable')
        return row

    def scan(self):
        rows = {self.pid: self.assert_leader()}
        scanned = 0
        with os.scandir('/proc') as entries:
            for entry in entries:
                if not entry.name.isdecimal():
                    continue
                scanned += 1
                if scanned > MAX_SCAN:
                    raise ValueError('process snapshot exceeds scan bound')
                fd = None
                try:
                    fd = os.open(entry.path, os.O_RDONLY | os.O_DIRECTORY)
                    rows[int(entry.name)] = self.row_at(fd)
                except (OSError, ValueError, IndexError):
                    pass  # A task can disappear while the snapshot is read.
                finally:
                    if fd is not None:
                        os.close(fd)
        return rows

    def sample(self):
        rows = self.scan()
        owned = descendants(rows, self.pid, self.start)
        if len(owned) > MAX_PROCESSES:
            raise ValueError('owned process inventory exceeds bound')
        pinned = {self.pid: self.root_fd}
        try:
            for pid in owned:
                if pid != self.pid:
                    fd = None
                    try:
                        fd = os.open(Path('/proc') / str(pid), os.O_RDONLY | os.O_DIRECTORY)
                        current = self.row_at(fd)
                        if (current['ppid'], current['starttime_ticks']) != (rows[pid]['ppid'], rows[pid]['starttime_ticks']):
                            os.close(fd)
                            fd = None
                            continue
                        pinned[pid] = fd
                        fd = None
                    except (OSError, ValueError, IndexError):
                        if fd is not None:
                            os.close(fd)
                        continue
            thread_budget = MAX_THREADS
            fd_budget = MAX_FDS
            processes = []
            for pid in owned:
                fd = pinned.get(pid)
                if fd is None:
                    continue
                # Check the entire pinned parent chain immediately before any
                # payload read. A vanished/reparented ancestor yields no row.
                chain = pid
                valid = True
                while chain != self.pid:
                    parent_fd = pinned.get(chain)
                    if parent_fd is None:
                        valid = False
                        break
                    current = self.row_at(parent_fd)
                    if (current['ppid'], current['starttime_ticks']) != (rows[chain]['ppid'], rows[chain]['starttime_ticks']):
                        valid = False
                        break
                    chain = current['ppid']
                if not valid:
                    continue
                self.assert_leader()
                row = self.row_at(fd)
                row['pid'] = pid
                try:
                    row['exe'] = link_at(fd, 'exe')
                    row['cwd'] = link_at(fd, 'cwd')
                    row['wchan'] = read_at(fd, 'wchan', 256).decode(errors='replace').strip()
                    row['io'] = {key: int(value) for key, value in
                                 (line.split(':', 1) for line in read_at(fd, 'io', 4096).decode('ascii').splitlines())}
                    row['threads'] = []
                    task_fd = os.open('task', os.O_RDONLY | os.O_DIRECTORY, dir_fd=fd)
                    try:
                        with os.scandir(task_fd) as tasks:
                            for task in tasks:
                                if thread_budget == 0:
                                    row['threads_truncated'] = True
                                    break
                                thread_fd = None
                                try:
                                    thread_fd = os.open(task.name, os.O_RDONLY | os.O_DIRECTORY, dir_fd=task_fd)
                                    thread = self.row_at(thread_fd)
                                    thread['tid'] = int(task.name)
                                    thread['wchan'] = read_at(thread_fd, 'wchan', 256).decode(errors='replace').strip()
                                    row['threads'].append(thread)
                                    thread_budget -= 1
                                except (OSError, ValueError, IndexError):
                                    pass
                                finally:
                                    if thread_fd is not None:
                                        os.close(thread_fd)
                    finally:
                        os.close(task_fd)
                    row['fds'] = []
                    descriptors = os.open('fd', os.O_RDONLY | os.O_DIRECTORY, dir_fd=fd)
                    try:
                        with os.scandir(descriptors) as entries:
                            for entry in entries:
                                if fd_budget == 0:
                                    row['fds_truncated'] = True
                                    break
                                try:
                                    row['fds'].append({'fd': int(entry.name), 'target': link_at(descriptors, entry.name)})
                                    fd_budget -= 1
                                except (OSError, ValueError):
                                    pass
                    finally:
                        os.close(descriptors)
                except (OSError, ValueError, IndexError) as error:
                    row['payload_unavailable'] = str(error)[:256]
                processes.append(row)
            self.assert_leader()
            return self.output.write({'event': 'sample', 'unix_ns': time.time_ns(),
                                      'elapsed_s': time.monotonic() - self.initial,
                                      'leader': self.pid, 'leader_starttime_ticks': self.start,
                                      'processes': processes, 'acceptance': False})
        finally:
            for pid, fd in pinned.items():
                if pid != self.pid:
                    os.close(fd)

    def close(self):
        os.close(self.root_fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pid', type=int, required=True)
    parser.add_argument('--start-time', type=int, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.pid <= 0 or args.start_time <= 0 or not Path('/proc').is_dir():
        parser.error('a pinned Linux process identity is required')
    output = Output(args.output)
    observer = None
    try:
        observer = Observer(args.pid, args.start_time, output)
        output.write({'event': 'observation-started', 'unix_ns': time.time_ns(),
                      'leader': args.pid, 'leader_starttime_ticks': args.start_time,
                      'clock_ticks_per_second': os.sysconf('SC_CLK_TCK'),
                      'page_size': os.sysconf('SC_PAGE_SIZE'), 'acceptance': False})
        # This only bounds this observer. The independently running harness owns
        # its original3600s per-side deadline and process-group kill/drain path.
        deadline = observer.initial + 7300
        while time.monotonic() < deadline and observer.sample():
            time.sleep(5)
        output.write({'event': 'observation-ended', 'reason': 'observer own duration/output bound', 'acceptance': False})
    except (OSError, ValueError, IndexError) as error:
        output.write({'event': 'observation-ended', 'reason': str(error)[:256], 'acceptance': False})
    finally:
        if observer is not None:
            observer.close()
        output.close()


if __name__ == '__main__':
    main()
