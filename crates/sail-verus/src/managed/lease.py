"""Private Linux read-lease owner. Protocol is bounded by the Rust parent.

A read lease cannot coexist with a writable file description or writable shared
mapping. SIGIO releases every descriptor immediately; this process never waits
for the kernel's lease-break timeout, so an editor can continue its write.
"""
import fcntl
import json
import os
import signal
import resource
import stat
import sys

fds = []

def release(_signal=None, _frame=None):
    for fd in fds:
        try:
            os.close(fd)
        except OSError:
            pass
    os._exit(0)

signal.signal(signal.SIGIO, release)
try:
    line = sys.stdin.buffer.readline(1_048_577)
    if len(line) > 1_048_576 or not line.endswith(b"\n"):
        release()
    request = json.loads(line)
    identities = request["files"]
    if request["runtime"] != support_paths():
        release()
    if not isinstance(identities, list) or len(identities) > 4096:
        release()
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    wanted = min(len(identities) + 16, hard)
    if soft < wanted:
        resource.setrlimit(resource.RLIMIT_NOFILE, (wanted, hard))
    for path, device, inode in identities:
        fd = os.open(bytes(path), os.O_RDONLY | os.O_CLOEXEC | os.O_NONBLOCK)
        fds.append(fd)
        metadata = os.fstat(fd)
        if not stat.S_ISREG(metadata.st_mode) or (metadata.st_dev, metadata.st_ino) != (device, inode):
            release()
        fcntl.fcntl(fd, fcntl.F_SETLEASE, fcntl.F_RDLCK)
    if request["runtime"] != support_paths():
        release()
    # This inventory is final: no lease acquisition or renewal is allowed after
    # readiness. Rust inspects kernel lease state directly, independently of
    # this process being scheduled to acknowledge another request.
    sys.stdout.buffer.write(json.dumps(fds).encode("ascii") + b"\n")
    sys.stdout.buffer.flush()
    sys.stdin.buffer.read(1)
    release()
except (OSError, ValueError, TypeError, AttributeError, KeyError):
    release()
