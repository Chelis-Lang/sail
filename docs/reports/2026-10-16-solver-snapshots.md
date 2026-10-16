# Solver snapshots: prototype findings

Status: experimental and disabled by default. No default-mode acceptance or
release selection. [ADR 0051](../decisions/0051-exact-solver-snapshots.md) describes
the candidate; the [validation record](../validation/solver-snapshots.md) lists
the completed checks and missing gates.

The prototype parks copies of the pinned Z3 process at exact input-prefix
boundaries. On an edit miss it resumes the latest available prefix instead of
re-solving that prefix. Ordinary exact cache keys and fresh-solver fallback
remain necessary. A raw-output equivalence proof is still missing.

## Exploratory native measurements

The recovered `m112b-results.tgz` archive has SHA-256
`6234d60a9ad4ef0358e09c63d2f002303145a974f5ee53fc9bdea3c619c4b021`.
Its `m112/t-anvil-shim/results.tsv` contains two rounds of the fork's Cargo-Verus
focused invocation through Sail's shim, with and without snapshots. Native
EC2 c7i.4xlarge, us-east-1; prototype source lineage through `88076b62`.
These are candidate timings, not default CLI acceptance or measurements of the
new admission bounds.

| Anvil edit | Round 1, shim | Round 1, snapshots | Round 2, shim | Round 2, snapshots |
|---|---:|---:|---:|---:|
| Body, early | 87.970 s | 35.834 s | 98.609 s | 46.906 s |
| Body, late | 87.106 s | 39.417 s | 104.083 s | 55.948 s |
| New lemma | 86.612 s | 35.996 s | 97.776 s | 39.824 s |
| Widely used spec | 114.373 s | 112.605 s | 110.403 s | 119.803 s |
| Fix after failure | 55.605 s | 55.555 s | 55.335 s | 61.785 s |

The body and lemma rows justify further investigation. Wide changes and the
failure/fix cycle have no demonstrated gain. The archive also contains earlier
Anvil `sail`/`snap` invocations that exited 1 before verification because they
used partial selectors with `verify` rather than `focus`. Those failed samples
remain in scope and cannot establish a performance comparison.

The native exactness experiments compare replies both raw and after masking
statistics. The exploratory successes after masking include `:num-allocs`
differences. They therefore do not establish raw-byte equality. The updated
report predicate counts masked replies as failures of byte-exactness. Earlier
format-comparison failures remain failures; they are not silently discarded.

## Review findings and bounded next step

The original preload's marker syntax consumed genuine NUL input. A tiny real
pinned-Z3 example, `(echo "x<NUL>y")`, exited 0 plainly and 1 with the preload;
a NUL in a comment could silently remove subsequent output. The prototype also
checked its snapshot count only when starting a solver, so a single solver
could create eight parked snapshots with a configured limit of two; four
concurrent solvers created 32.

The local hardening separates NUL data from markers, checks the preload framing
version before any input, and admits each parked process under a shared count
and conservative resident-memory budget. Corrupt or unavailable setup falls
back before forwarding input. Admission refusal only skips the snapshot.
The count/memory caps apply to parked-process admission, not to all future
memory use of ordinary active solver workers.

Next, qualify the revised prototype natively, measure its costs with the new
bounds, and preserve both raw and masked reply counts. To avoid redundant
measurement work, a future harness may memoize a fresh oracle by solver binary
digest, exact argv and input digest, explicitly recording reference reuse.
Timing an edit can select only changed transcripts after establishing the full
base cache; this is a changed-transcript experiment and cannot be labelled a
whole-project benchmark. Neither optimization permits dropping failed samples.
