# Linux event observation is insufficient for cached freshness

An existing writable shared mapping can change file bytes without an inotify
MODIFY or ATTRIB event. Closing the originating writable descriptor does not
remove the mapping. A directory watch also misses writes through a preexisting
hardlink outside the watched directory; direct inode watches cover that case,
but still cannot cover mapped writes.

Run `python3 corpus/repros/linux-inotify-mmap/repro.py` on Linux. The reproduction
asserts changed bytes and zero file events, rejection of a kernel read lease
while the mapping exists, and successful lease acquisition after it is unmapped.
The fixture uses external Python ctypes solely to call inotify for the repro.
Sail's production code contains no unsafe Rust.

The [inotify manual](https://man7.org/linux/man-pages/man7/inotify.7.html) documents
its mmap limitation. The [lease manual](https://man7.org/linux/man-pages/man2/F_SETLEASE.2const.html)
documents read leases, writer conflicts, immediate break visibility through
F_GETLEASE, and SIGIO notification before a conflicting open or truncate.

Freshness readers must hash mutable source/config/build inputs on every read.
Expensive runtime digests may be retained only while kernel read leases remain
held, with path/inode bindings checked and synchronous native barriers. Unsupported
leases, helper loss, existing writable mappings, queue overflow and lost watches
must prevent cached Current claims. Read-only bind mounts and ordinary mode bits
are not sufficient: an external writable alias or existing mapping can remain.
