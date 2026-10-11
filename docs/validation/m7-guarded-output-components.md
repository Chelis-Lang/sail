# Guarded stdout and parent lookup components

These are unselected library/test components, not a live reader pass. The
[original descriptor reader workflow](m7-descriptor-reader-terminal.md) still
failed full Result p95 at 343.142 ms; its Rust and retained stages were
unattempted. Its 42-row recorder tail motivated this comparison, not a claim
that output sealing alone explains all 200 samples.

The [complete record](m7-guarded-output-components.json) retains every timing,
source/binary seal, original strict-Clippy failure and corrected controls. The
complete raw archive is 20,449,413 bytes, SHA-256
`71b0ad46ba3c856895f90e904f45ca6fbba75e0168b5fd435dedac8c60edae50`.
It includes both test executables and the complete-output profiler executable.

## Output owner

The exact retained worker stdout is 2,158,309 bytes, SHA-256
`718fc30e7df9978a21d130df8bbbb459cf9f433865462db4ca33b615ce531347`.
A new service-owned copy was independently hashed after all output guards were
acquired. It includes the entire report; no schema, dependency or repeated
result graph was removed. Each guarded operation includes output namespace,
helper/interpreter, path/mount and complete kernel checks; contention immediately
performs the original full read/hash. Independent input freshness remains outside
this output-only comparison and is not supplied by the output guard.

| Clients / samples per condition | Original median / p95 | Guarded median / p95 | Guarded hits |
| --- | --- | --- | --- |
| 1 / 16 | 9.500 / 13.433 ms | 0.497 / 9.778 ms | 16/16 |
| 4 / 64 | 11.867 / 25.781 ms | 0.382 / 17.186 ms | 51/64 |

The fixed order was original then guarded, with 16 samples per client. No best-of
or rerun was selected. Construction took 609.065 ms and requested retained
storage was 318,900 bytes; these are not an RSS or whole-construction bound.
Six real Linux controls passed, covering wrong digest, restored content/mtime,
rename/replacement, outside-hardlink/mmap writes, helper loss, cancellation and
oversized output. Release all-target strict Clippy passed after four retained
lint corrections. Original and corrected test/source identities remain distinct.

The measured component benefit supports a bounded integration design, not
activation. [ADR 0035](../decisions/0035-guarded-retained-output-digests.md)
requires reservation before construction and through eviction/in-flight references
in the existing frame quota, explicit constructor/FD/watch/helper bounds, and
unchanged per-RPC input/output barriers and fallback before daemon enablement.
No daemon result path currently selects this primitive.

## Parent grouping

The test-only grouping candidate uses at most 64 ephemeral directory descriptors,
checks every child with the original exact device/inode and proc pathname
predicate, then freshly resolves every parent. Any unsupported lookup or parent
mismatch rechecks the complete original absolute inventory. No descriptor memo
survives a barrier. Four real native-path, alias-switch, parent-replacement and
overflow controls passed. The equal-inode hardlink control demonstrates that the
original path predicate alone permits the rebound while the unchanged outer
namespace Observer rejects its rename event.

On one complete original `ReadLease.clean` owner over 512 generated files, 16
original calls had median/p95 14.786/35.889 ms; 16 grouped calls had
12.502/145.220 ms. All original interpreter/helper/native/mount/kernel barriers
and a real writer invalidation were included. This wall-only, fixed-order
comparison does not identify the tail's cause and does not demonstrate a tail
improvement. The candidate stays test-only. No additional timing retry was run.

No proof, endpoint read, live verifier census, Current report admission or
performance acceptance was performed. Every actual control returned normally;
helper ownership uses the original kill/wait cleanup. This record does not claim
an independent exhaustive orphan census.
