# Runtime identity sidecar component controls

This is an unselected, test-only freshness candidate. It retains one eligibility
boolean on the original runtime validation owner and avoids the repeated hot
identity parse only if that lease binds both exact raw sidecar paths. Initial
cold discovery, independent final identity reads and every per-read freshness
barrier remain unchanged. Canonical aliases or absent membership fall back to
the original parse. No parsed tree, path inventory or shared owner is added.

Six real Linux integration controls passed. They cover both the context `.link`
and identity `.json`, restored bytes and mtime, identical delete/recreate,
symlink alias replacement, outside-hardlink writes, mmap, ancestor redirection,
same-inode namespace rebinding, helper death and an evicted but still-held native
owner. The latter preserves the existing lifetime rule: eviction drops an index
reference; a reader's original owner remains valid only while every fresh barrier
passes. Dirty owners are cleared and cannot be promoted by the boolean.

The initial control run passed in 11.434 s, then strict Clippy failed on the new
test's assertion style. The scope stopped before profiling and retains its
source and binary. After the equivalent assertion correction, the six controls
passed in 9.759 s and release all-target strict Clippy passed in 8.459 s.

One fixed-order complete-inspect comparison used 403 generated identity pairs
referencing **one** actual runtime file, with a 50,779-byte identity JSON. This is
not a 403-file production inventory. The original and candidate each made 16
complete inspections, including all existing namespace, helper, interpreter,
mount and kernel barriers. Revisions and content paths matched after every call.
Hot parse count changed from 16 to zero.

| Complete inspect, milliseconds | Original | Guarded candidate |
| --- | ---: | ---: |
| Median | 0.6055 | 0.3305 |
| Nearest-rank p95 | 1.142 | 0.343 |
| Maximum | 1.142 | 0.343 |

This small reduction supports reviewing the redundant parse. It does not explain
or eliminate the separate 51.489 ms freshness tail from 42 retained Result rows,
or establish the original full four-reader latency target. The complete
[descriptor workflow](m7-descriptor-reader-terminal.md) remains failed at
343.142 ms Result p95; its Rust and retained stages were unattempted.

The [machine-readable record](m7-runtime-sidecar-components.json) retains all
samples, exact source and binary hashes, original failure, corrected checks and
raw archive inventory. No proof, live reader census or acceptance run was added.
At this initial component revision, production remained on its original parse.
The [later production selection](m7-runtime-sidecar-production.md) retains the
same eligibility and barriers; it does not repeat this timing comparison.
