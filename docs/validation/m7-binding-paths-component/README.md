# Exact path-binding syscall component

This offline component compares 327 healthy native paths under four readers,
32 alternating waves, with exact pathname/device/inode checks. The C fixture
models kernel operations only: its buffers differ from Rust allocations. It
exercises no live proof, freshness owner, full report or client transport.

The [record](record.json) binds all 128 paired samples, strict compiler command,
source and raw hashes. The first attempt and its compiler formatting warnings
remain recorded; the corrected strict source also validates procfs identity.
This component cannot replace the real 200-sample latency target. Actual Rust
regression controls and the full gate are separate.
