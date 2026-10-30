# 0091: Settled source and owned-target digest observations

- **Status:** accepted by the owner on 2026-10-29; implementation qualified; Linux target default selected; source and Mac target remain off by default; release qualification open
- **Date:** 2026-10-29
- **Amends:** [ADR 0082](0082-unchanged-call-replay.md), [freshness](../freshness.md)
- **Tracks:** [#560](https://github.com/Chelis-Lang/sail/issues/560)

## Context

The selected within-call cuts preserve fresh reads. Ten-call diagnostic
medians reduce full-path Anvil from 1.818 to 1.536 s on Linux and 1.554 to
1.378 s on macOS. Those calls remain substantially slower than stock; the
measurements are diagnostic, not paired release acceptance.

A read-only Mac component spike on mac-m4.metal/us-east-1b/macOS 27.0 uses
Anvil's resolved package trees (18,872 files, 287,999,767 bytes). Ten rotated
pairs compare complete fresh SHA-256 reads bracketed by full file stamps
against reading a 4,919,620-byte hypothetical ledger and matching every full
stamp. Medians are 544.236 ms fresh and 42.727 ms ledger/stat. Both agree on
the complete selected file list and stamps; seeded files were already settled.

This Python spike supplies nothing to Sail verification. It excludes the walk
from both timed arms, omits target artifacts, and is not Sail's exact snapshot
implementation or a product speedup. It supports a bounded native prototype;
it does not promise the roughly 0.50 s component difference in the product.
The earlier Linux digest spike in ADR 0082 found no measurable gain, so both
platforms must still justify adoption independently.

Raw spike log SHA-256:
`4f90060ac56c9fae0d34a15d54cb04bdb98395928f84b646750530cef366e886`.
Driver SHA-256:
`e19239229f6bf95d9f5f065a1b6f40bea21827ff7967dd464eb64732b5123e51`.

## Decision

Extend ADR 0082's existing full-stat trust to two separately qualified local
optimizations. Keep all logical proof keys, exact solver input bytes, selected
runtime checks, receipt admission and output rules unchanged.

1. A source file's content digest may come from a previous **Current** run's
   complete original observation only when the absolute path, resolved binding,
   type and complete device/inode/size/mode/mtime/ctime stamp still match. The
   original content read must have been bracketed by equal full stamps, and
   that stamp's ctime must have been at least two seconds older than the
   original run's start. A later call never promotes an originally unsettled
   observation merely because time has passed.
2. Artifact/dependency digests and pure receipt revisions may come from a
   previous Current run only after freshly matching the complete owned target's
   entry set, types, bindings and settled full stamps, the complete source
   observation, metadata/selection, compiler family, context and receipt bytes.
   A changed/unknown entry, changed receipt or changed source keeps the ordinary
   cleanup and complete read path. A dirty package is still cleaned. This
   second optimization needs its own native before/after result before selection.

Namespace discovery stays fresh: directories, absent configuration names,
module/target discovery, dep-info, build-script declarations and symlink
ancestors are still checked. New files, a retargeted link, a same-size edit or
a restored mtime invalidate the relevant observation. Nothing may substitute
old bytes beneath a new stamp or carry an observation across a retry.

Only successful complete Current observations seed these private scheduling
ledgers. Historical/Unknown observations, missing fields, inconsistent path
sets, failed reads, damaged records, unsupported platforms and a missing ledger
all take existing fresh paths. Non-Unix never trusts a memo. Each record binds
its schema, namespace, complete entries and checksum. Impose finite entry/byte
ceilings and bounded context retention; overflow is a miss. Ledgers remain
local to the owned Sail cache and are never accepted from shared/team stores.

The two-second rule and its existing limitations (privileged clock rollback or
a filesystem that does not advance ctime) are unchanged from ADR 0082. This
is an explicit extension of that trust, so freshness documentation must change
with the implementation. No transcript synthesis, verifying-run replay record,
background recorder or reduced recording gate is authorized here.

## Qualification and consequences

Before selecting either optimization: native macOS and Linux before/after
measurements with independent on/off controls; same-size restored-mtime edits;
changes less than two seconds before original recording; symlink/ancestor
retargeting; missing/new/deleted source and target entries; malformed ledgers;
changed receipt/selection/config; retry and non-Unix refusal. Preserve current
Currentness, dirty-clean, fault-injection, output and replay controls.

Run ordinary/solver gates, full E2E, fast and standard conformance for the
implementation. Deep, deep-rvt with/without cache, determinism and the complete
six-round four-project timing/memory matrix gate release acceptance. If the
bounded native prototype does not save material wall time, retain fresh reads
on that platform and do not select it merely because a component spike is fast.
