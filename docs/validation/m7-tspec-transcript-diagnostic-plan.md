# One tspec transcript diagnostic preparation

This retained preparation scope preceded execution. The subsequent single run
was [interrupted by filesystem/container failure](m7-tspec-transcript-interrupted.md)
before a unit result; its original settings and failed historical scope remain.

The [retained complete standard failure](m7-default-standard-e0ca-terminal.md)
and [VI-0013](../verus-issues/VI-0013-verismo-tspec-stock-wait.md) remain
unchanged. The selected stock tspec arm exceeded its original 3,600-second
bound. Neither sleeping metadata nor the pinned AIR channel audit establishes
the cause. This preparation launches no verifier, shim, daemon or solver.

The concrete [freeze script](../../scripts/m7-tspec-transcript-freeze.py) and
[execution controller](../../scripts/m7-tspec-transcript-run.py) bind the
existing `0f31c1d` conformance executable and `012707a` shim, exact stock
runtime, pinned corpus and immutable runner source. The conformance executable
has SHA-256 `1f62c6c276158cdf3dc083337115323eff02a97d25df1806fcf7c558709b78bf`;
its run implementation is byte-identical to the retained e0ca implementation,
`59d7ca4686025280daee10cfd8f15f9a982739d49ea0e89e440a7638ae2aaa93`.
This earlier-product diagnostic cannot qualify the current standard tier.

Only the private corpus manifest changes: Verismo receives
`--log smt-transcript --log-dir {out}/smt-transcripts`. Every other parsed
manifest field is identical. The existing runner expands `{out}` to each
arm's distinct output directory, preserving both arms' files even though
Cargo runs them from the same package directory. The single selected unit
keeps jobs 1, its original thread and solver policy, 3,600-second per-arm
bound, classifier, one initial plain run and two conditional confirmation
runs. Sail runs only if the plain arm completes. There is no manual retry,
repair preparation, cache fill or modified acceptance oracle.

The execution controller writes the outer harness stdout/stderr directly to
retained files. It introduces no proof timeout or alternate pipe drain. The
unchanged harness still owns its unit process group, concurrent raw stream
readers, timeout kill and classification. An independent, unchanged metadata
observer has a 24 MiB/7,300-second cap; reaching that cap can omit later
conditional confirmation observations and never certifies their absence.

Bounded ownership scans retain PID birth and group identity only for leaders
observed under the live pinned harness. Cleanup rechecks those identities
before signalling child groups and the primary. PID reuse, a missing parent,
or an unrelated group grants no signal authority. Scans cap at 16,384 process
entries and 256 retained groups. An unobserved or escaped orphan cannot be
certified absent. Controller failure is retained separately from the
harness's original result. Catchable controller SIGTERM/SIGHUP enters that
same cleanup through unwind; SIGKILL cannot run cleanup. The initial
unexecuted controller without those handlers remains retained separately.
Every admitted child also has a kernel start time at least its live parent's
start time along the full rooted chain. A child predating a recycled
intermediate PID or a zombie parent cannot grant cleanup authority.

A supplementary archive copies at most 512 MiB across 4,096 files, with a
120-second copying deadline. It excludes the private environment file and
never deletes or truncates original captures or transcripts. Partial archive
and observer scopes remain explicitly partial. These caps do not bound stock
log growth; initial free space is checked and primary files stay outside
compiler scratch.

Pinned Verus flushes QUERY to the transcript before enqueueing it to the
solver writer, and writes RESPONSE after the reader delivers its completion
delimiter. Logs therefore identify intended batches and completed response
batches, not accepted solver wire. An unmatched QUERY, incomplete transcript
or unavailable filename must remain separately labelled; none alone proves
which actor stalled. Diagnostic success does not explain the old timeout.

The [preparation record](m7-tspec-transcript-diagnostic-plan.json) binds actual
Linux validation, script hashes and eleven pure ownership/storage controls.
The first host setup lacked `tomllib`; the first native prepare used the
unsupported `Path.open(opener=...)` argument. Both setup failures remain
retained, with no actor launched. The corrected prepare and validation-only
controller pass; execution remains pending lead review.
