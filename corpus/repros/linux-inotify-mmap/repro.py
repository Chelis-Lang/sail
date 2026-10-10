"""Demonstrate why native event observation is not a content oracle.

Run on Linux: python3 corpus/repros/linux-inotify-mmap/repro.py
No third-party packages. This standalone reproduction uses Python ctypes;
production Sail remains forbid(unsafe_code) and uses a separate fcntl helper.
"""
import ctypes
import fcntl
import mmap
import os
import tempfile

with tempfile.TemporaryDirectory(prefix="sail-inotify-mmap-") as root:
    path = os.path.join(root, "runtime")
    with open(path, "wb") as stream:
        stream.write(b"good")
    writer = os.open(path, os.O_RDWR)
    mapping = mmap.mmap(writer, 0)
    os.close(writer)  # A writable mapping survives its originating descriptor.
    native = ctypes.CDLL(None, use_errno=True)
    fd = native.inotify_init1(os.O_NONBLOCK | os.O_CLOEXEC)
    assert fd >= 0
    watch = native.inotify_add_watch(fd, os.fsencode(path), 2 | 4)  # MODIFY|ATTRIB
    assert watch >= 0
    mapping[:] = b"fail"
    mapping.flush()
    try:
        events = os.read(fd, 65536)
    except BlockingIOError:
        events = b""
    assert not events, events
    with open(path, "rb") as stream:
        assert stream.read() == b"fail"
    reader = os.open(path, os.O_RDONLY)
    try:
        fcntl.fcntl(reader, fcntl.F_SETLEASE, fcntl.F_RDLCK)
    except BlockingIOError:
        rejected = True
    else:
        rejected = False
        fcntl.fcntl(reader, fcntl.F_SETLEASE, fcntl.F_UNLCK)
    assert rejected, "existing writable mapping must prevent read lease"
    mapping.close()
    fcntl.fcntl(reader, fcntl.F_SETLEASE, fcntl.F_RDLCK)
    assert fcntl.fcntl(reader, fcntl.F_GETLEASE) == fcntl.F_RDLCK
    fcntl.fcntl(reader, fcntl.F_SETLEASE, fcntl.F_UNLCK)
    os.close(reader)
    os.close(fd)
    print("changed bytes; zero MODIFY/ATTRIB events; existing mmap rejects lease; unmap permits lease")
