#!/usr/bin/env python3
"""Sample a command's process-tree RSS, excluding and retaining Sail daemons.

Run with python3 -I. No environment variables are added to the measured child.
RSS is KiB on both platforms. Shared pages are counted once per process;
50 ms sampling can miss short-lived processes and peaks between samples.
"""
import argparse
import ctypes
import errno
import json
import os
from pathlib import Path
import signal
import select
import struct
import subprocess
import sys
import time

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


def mac_ps_processes():
    result = subprocess.run(['ps', '-A', '-o', 'pid=,ppid=,rss=,lstart=,command='],
                            check=True, capture_output=True, text=True)
    processes = {}
    for line in result.stdout.splitlines():
        fields = line.split(None, 8)
        if len(fields) == 9:
            pid, parent, rss = map(int, fields[:3])
            processes[pid] = (parent, rss, ' '.join(fields[3:8]), fields[8].split())
    return processes


class _MacBsdInfo(ctypes.Structure):
    # Public Darwin sys/proc_info.h, LP64 proc_bsdinfo (136 bytes).
    _fields_ = [(name, ctypes.c_uint32) for name in
                'flags status xstatus pid ppid uid gid ruid rgid svuid svgid reserved'.split()]
    _fields_ += [('comm', ctypes.c_char * 16), ('name', ctypes.c_char * 32)]
    _fields_ += [(name, ctypes.c_uint32) for name in 'nfiles pgid pjobc tdev tpgid'.split()]
    _fields_ += [('nice', ctypes.c_int32), ('seconds', ctypes.c_uint64),
                ('micros', ctypes.c_uint64)]


class _MacTaskInfo(ctypes.Structure):
    _fields_ = [(name, ctypes.c_uint64) for name in
                'virtual resident user system threads_user threads_system'.split()]
    _fields_ += [(name, ctypes.c_int32) for name in
                 'policy faults pageins cow sent received syscalls_mach syscalls_unix '
                 'csw threads running priority'.split()]


class _MacAllInfo(ctypes.Structure):
    _fields_ = [('bsd', _MacBsdInfo), ('task', _MacTaskInfo)]


class ProcessTable(dict):
    """A process snapshot with its actual observation backend."""
    def __init__(self, *args, backend):
        super().__init__(*args)
        self.backend = backend


class MacCollector:
    """Observe the watched forest directly; never enumerate unrelated RSS.

    proc_listchildpids computes the same descendant closure as Tracker's full
    table scan. Previously observed/reparented processes are additional roots.
    A permission/ABI failure falls back to the complete original ps snapshot;
    unknown memory is never substituted with zero. Both routes retain ps's
    start-time identity representation so retained daemon state stays valid.
    """
    def __init__(self):
        if (ctypes.sizeof(_MacBsdInfo), ctypes.sizeof(_MacTaskInfo),
                ctypes.sizeof(_MacAllInfo)) != (136, 96, 232):
            raise OSError('unsupported Darwin process-info ABI')
        self.lib = ctypes.CDLL('/usr/lib/libproc.dylib', use_errno=True)
        self.lib.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64,
                                         ctypes.c_void_p, ctypes.c_int]
        self.lib.proc_pidinfo.restype = ctypes.c_int
        self.lib.proc_listchildpids.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
        self.lib.proc_listchildpids.restype = ctypes.c_int
        self.system = ctypes.CDLL(None, use_errno=True)
        self.system.sysctl.argtypes = [ctypes.POINTER(ctypes.c_int), ctypes.c_uint,
                                      ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t),
                                      ctypes.c_void_p, ctypes.c_size_t]
        self.system.sysctl.restype = ctypes.c_int

    def children(self, pid):
        capacity = 16
        for _ in range(5):
            entries = (ctypes.c_int * capacity)()
            ctypes.set_errno(0)
            count = self.lib.proc_listchildpids(pid, entries, ctypes.sizeof(entries))
            error = ctypes.get_errno()
            if count < 0 or (count == 0 and error not in (0, errno.ESRCH, errno.ENOENT)):
                raise OSError(error, 'cannot enumerate process children')
            if count < capacity:
                return [child for child in entries[:count] if child > 0]
            capacity *= 4
        raise OSError('process child inventory exceeded its bound')

    @staticmethod
    def decode_argv(data):
        if len(data) < 4:
            raise ValueError('truncated Darwin argument header')
        count, = struct.unpack_from('=i', data)
        if not 0 < count <= len(data) - 4:
            raise ValueError('invalid Darwin argument count')
        offset = data.index(b'\0', 4) + 1  # kernel's executable pathname
        while offset < len(data) and data[offset] == 0:
            offset += 1
        argv = []
        for _ in range(count):
            end = data.index(b'\0', offset)
            argv.append(os.fsdecode(data[offset:end]))
            offset = end + 1
        return argv

    def argv(self, pid):
        # CTL_KERN/KERN_PROCARGS2: the counted argv is distinct from environ.
        mib = (ctypes.c_int * 3)(1, 49, pid)
        size = ctypes.c_size_t()
        if self.system.sysctl(mib, 3, None, ctypes.byref(size), None, 0):
            raise OSError(ctypes.get_errno(), 'cannot size process arguments')
        if not 4 < size.value <= 16 * 1024 * 1024:
            raise OSError('invalid process-argument buffer bound')
        data = ctypes.create_string_buffer(size.value)
        if self.system.sysctl(mib, 3, data, ctypes.byref(size), None, 0):
            raise OSError(ctypes.get_errno(), 'cannot read process arguments')
        return self.decode_argv(data.raw[:size.value])

    def collect(self, roots):
        table = ProcessTable(backend='darwin-libproc')
        pending, seen = list(roots), set()
        while pending:
            pid = pending.pop()
            if pid <= 0 or pid in seen:
                continue
            seen.add(pid)
            info = _MacAllInfo()
            ctypes.set_errno(0)
            size = self.lib.proc_pidinfo(pid, 2, 0, ctypes.byref(info), ctypes.sizeof(info))
            if not size and ctypes.get_errno() in (errno.ESRCH, errno.ENOENT):
                continue
            if size != ctypes.sizeof(info) or info.bsd.pid != pid:
                raise OSError(ctypes.get_errno(), 'incomplete process information')
            name = os.fsdecode(info.bsd.name or info.bsd.comm)
            argv = self.argv(pid) if name == 'sail' else [name]
            # Keep the existing lstart identity and PID-reuse comparisons.
            started = ' '.join(time.ctime(info.bsd.seconds).split())
            table[pid] = (info.bsd.ppid, info.task.resident // 1024, started, argv)
            pending.extend(self.children(pid))
        return table


_MAC_COLLECTOR = None


def mac_processes(roots=None):
    global _MAC_COLLECTOR
    try:
        if _MAC_COLLECTOR is None:
            _MAC_COLLECTOR = MacCollector()
        return _MAC_COLLECTOR.collect(roots if roots is not None else {os.getpid()})
    except (OSError, ValueError, AttributeError):
        # Protected/setuid descendants and unsupported OS interfaces retain the
        # complete original observation instead of silently losing their RSS.
        return ProcessTable(mac_ps_processes(), backend='darwin-ps')


def processes(roots=None):
    return linux_processes() if sys.platform.startswith('linux') else mac_processes(roots)


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
        self.backends = set()

    def sample(self, table, root):
        self.backends.add(getattr(table, 'backend', 'linux-procfs' if sys.platform.startswith('linux') else 'unspecified'))
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


def prime(args):
    """Observe the initial inventory before either arm releases its command."""
    known = load(args.state, {})
    if not isinstance(known, dict):
        known = {}
    errors = []
    try:
        # Prime the same inventory before readiness in both arms. Otherwise
        # the control changes pre-command work as well as recurring sampling.
        status = load(args.status, {})
        pid = (status.get('info') or {}).get('pid')
        roots = {int(value) for value in known} | {os.getpid()}
        if isinstance(pid, int) and pid > 0:
            roots.add(pid)
        table = processes(roots)
    except (OSError, subprocess.SubprocessError) as error:
        errors.append(str(error))
        table = {}
    status = load(args.status, {})
    pid = (status.get('info') or {}).get('pid')
    # The cache's own status identifies its server. Darwin can rewrite argv[0]
    # (and ps cannot delimit executable paths with spaces), so command spelling
    # must not reject that authoritative PID. Retain the observed start identity.
    if pid in table:
        known[str(pid)] = table[pid][2]
    tracker = Tracker(known)
    tracker.backends.add(getattr(table, 'backend', 'linux-procfs' if sys.platform.startswith('linux') else 'unspecified'))
    return tracker, table, errors


def observe(args):
    """Separate interpreter: inventories cannot hold the command waiter's GIL."""
    tracker, _, errors = prime(args)
    # Only this observer and its future ps children receive low priority.
    # The command and wait4 reporter keep their caller's scheduling priority.
    if sys.platform == 'darwin':
        try:
            os.setpriority(os.PRIO_PROCESS, 0, 19)
        except OSError as error:
            errors.append(f'cannot lower observer priority: {error}')
    sys.stdout.buffer.write(b'R')
    sys.stdout.buffer.flush()
    message = b''
    while len(message) < 8:
        chunk = os.read(0, 8-len(message))
        if not chunk:
            return 2
        message += chunk
    root, = struct.unpack('!q', message)
    try:
        if args.no_sample:
            os.read(0, 1)
        else:
            while True:
                tracker.sample(processes(set(tracker.owned) | set(tracker.daemons) | {root}), root)
                if select.select([0], [], [], INTERVAL)[0]:
                    os.read(0, 1)
                    break
    except (OSError, subprocess.SubprocessError) as error:
        errors.append(str(error))
    result = dict(tree_peak_kb=tracker.tree_peak, daemon_peak_kb=tracker.daemon_peak,
                  samples=tracker.samples, daemons=tracker.daemons, errors=errors,
                  collection_backends=sorted(tracker.backends))
    sys.stdout.write(json.dumps(result)+'\n')
    sys.stdout.flush()
    return 0


def run(args):
    if args.snapshot:
        tracker, table, errors = prime(args)
        tracker.sample(table, None)
        # A server may have detached between samples before it was ever seen
        # as a descendant. The harness supplies fresh status after the call;
        # retain that completion observation and seed the following phase.
        result = load(args.output, {})
        result['collection_backends'] = sorted(set(result.get('collection_backends', [])) | tracker.backends)
        result.update(daemon_peak_kb=max(result.get('daemon_peak_kb') or 0,
                                         tracker.daemon_peak),
                      daemon_hwm_kb=daemon_hwm(table, tracker.daemons))
    else:
        command = [sys.executable, '-I', str(Path(__file__).resolve()), '--observer',
                   '--output', str(args.output)]
        for name in ('state', 'status'):
            if value := getattr(args, name):
                command += ['--'+name, str(value)]
        if args.no_sample:
            command.append('--no-sample')
        observer = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        if observer.stdout.read(1) != b'R':
            observer.communicate()
            print('perf-mem: observer failed before readiness', file=sys.stderr)
            return 2
        # Preparing Python and the initial process inventory is harness setup,
        # before run_one starts its clock. The FIFO releases the actual command.
        if args.ready:
            args.ready.write_text('ready\n')
            with args.go.open('rb', buffering=0) as gate:
                if not gate.read(1):
                    observer.communicate()
                    return 2
        command_started = time.monotonic()
        try:
            child = subprocess.Popen(args.command)
        except OSError as error:
            observer.communicate()
            print(f'perf-mem: {error}', file=sys.stderr)
            return 127 if isinstance(error, FileNotFoundError) else 126
        observer.stdin.write(struct.pack('!q', child.pid))
        observer.stdin.flush()
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
        try:
            _, wait_status, usage = os.wait4(child.pid, 0)
            wall_seconds = time.monotonic() - command_started
            code = os.waitstatus_to_exitcode(wait_status)
            child.returncode = code
        finally:
            observed, _ = observer.communicate(b'x')
            for signum, handler in previous.items():
                signal.signal(signum, handler)
        try:
            observation = json.loads(observed)
            if observer.returncode != 0:
                raise ValueError(f'observer exit {observer.returncode}')
            errors = observation['errors']
            tracker = Tracker(observation['daemons'])
        except (ValueError, KeyError, TypeError) as error:
            observation = dict(tree_peak_kb=None, daemon_peak_kb=None, samples=0)
            errors = [f'invalid observer result: {error}']
            tracker = Tracker({})
        result = {'tree_peak_kb': observation['tree_peak_kb'],
                  'largest_kb': largest_kb(usage.ru_maxrss, sys.platform),
                  'daemon_peak_kb': observation['daemon_peak_kb'],
                  'interval_ms': 50, 'samples': observation['samples'], 'exit': code,
                  'collection_backends': observation.get('collection_backends', [])}
        result['wall_seconds'] = wall_seconds
        if errors:
            result.update(tree_peak_kb=None, daemon_peak_kb=None, errors=errors)
        if args.no_sample:
            result.update(tree_peak_kb=None, largest_kb=None, daemon_peak_kb=None)
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
    parser.add_argument('--no-sample', action='store_true')
    parser.add_argument('--observer', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--ready', type=Path)
    parser.add_argument('--go', type=Path)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command[:1] == ['--']:
        args.command.pop(0)
    if not args.command and not args.snapshot and not args.observer:
        parser.error('a command is required after --')
    if bool(args.ready) != bool(args.go):
        parser.error('--ready and --go must be supplied together')
    return observe(args) if args.observer else run(args)


if __name__ == '__main__':
    sys.exit(main())
