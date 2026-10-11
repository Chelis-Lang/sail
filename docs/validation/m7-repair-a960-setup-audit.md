# Original repair budget and retained setup diagnostic

The original `a9609e9` repair fixture exceeded its unchanged 120-second whole
budget while polling the missing-function job. That job started 27 milliseconds
after submission and had only about three seconds before the deadline. Its empty
worker streams do not establish a missing-function hang, queue contention or a
cold dependency rebuild. The [machine record](m7-repair-a960-setup-audit.json)
separates this original failure from the subsequent diagnostic.

| Completed job | Worker lifecycle | Managed runner | Cargo reported |
| --- | ---: | ---: | ---: |
| Initial success | 93.081 s | 86.911 s | 1m 06s |
| Declaration follow-up | 22.262 s | 17.050 s | 4.71 s |

The follow-up reused ordinary dependencies and freshly checked the requested
crate. Both jobs completed with Current full evidence, and the existing required
declaration follow-up remained intact. About 6.170 and 5.212 seconds respectively
fell outside the reported runner interval. The remaining runner time includes
setup and post-verification work; subtracting Cargo's rounded timings does not
identify a particular operation. No speed or load-causality claim follows.

The original fixture deleted its temporary project and cache after copying only
public worker stdout/stderr. Its automatically generated setup timing sidecars
were not retained. Code inspection confirms that the compiler-artifact family
and optional sysroot selection omit only the driver-local function; proof
contexts and receipts still bind the complete request. A new missing selector
does not by itself establish a fresh dependency target. Running jobs receive
their final revision only after execution, so a null revision is not a phase
identifier.

One separately authorized diagnostic invoked the byte-exact original fixture
and product with the original 120/30/5 limits, assertions and environment. Its
[controller](m7-repair-setup-diagnostic.py) preserved the original scratch paths
in place instead of deleting them, observed the actual daemon's birth, and kept
the fixture's original actor cleanup. This retained the existing bounded setup
sidecars without adding a compiler probe, proof call or environment selector.
The run failed when the container filesystem became read-only. The initial
summary and 36 operations survive, but the primary outcome and final result
could not be written. The shell exited 120 and explicitly failed to write its
exit file with `Read-only file system`; an empty log is not a successful run.

The last completed phases were metadata (1.327 seconds), runtime binaries
(1.764), helper identity (0.261), toolchain probes (1.778), sysroot guard (0.847)
and sysroot handoff (0.010). The next code phase is sysroot hashing, whose timer
had not completed. No Cargo stderr or missing submission was observed. This
interrupted run cannot establish the original hashing cost or a product defect.

Readable project/cache/raw files were streamed to a private host archive without
writing to the read-only container. Its 91 regular members total 28,269 bytes;
sockets were excluded. The original daemon leader was absent after exit, but no
global orphan-absence claim is made. Container start identities were unchanged.
Concurrent solver and reader work is recorded without attributing the failure
to it. No further run or speculative production correction was launched. A
future authorized diagnostic requires writable storage and must retain the same
phase records before selecting a concrete optimization; all original correctness
and timing checks remain mandatory.
