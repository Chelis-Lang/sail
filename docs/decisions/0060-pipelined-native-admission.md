# 0060: Certify native units after their compile

- **Status:** proposed (prototype on `perf/pipelined-admission`; not selected)
- **Date:** 2026-10-20
- **Amends:** [ADR 0050](0050-retained-proofs-instant-revert.md)
- **Relates to:** [#232](https://github.com/Chelis-Lang/sail/issues/232),
  [cold first run](../validation/cold-first-run.md)

## Context

Instant revert certifies every native unit in a retained profile, so that a
later verified target's dependency graph can be bound to successful,
admitted compiles. Before compiling a native unit, the driver waits in two
ways. First, until every artifact in its lookup directories has a producer
certificate (`producers::ready_for`, up to 2 s). Second, until no compiler
scratch directory remains there (`capture_after_directory_work`). Only then
does it copy the directories, check the copies against the certificates, and
compile against the copies.

Cargo starts a library's dependents as soon as its metadata exists. The first
wait holds such a dependent until its producer has fully finished and
certified. The second wait holds it until every other unit compiling into the
same directory has finished. On a fresh demo project these waits put about
0.5–0.9 s on the critical path: `verus_prettyplease` starts only after
`verus_syn` has finished, where stock starts it from `verus_syn`'s metadata.
`synstructure` waits 0.8 s.

The driver already repeats the whole certification after the compile.
`Native::finish` checks that the copies are intact, that every original still
holds the copied bytes, and that each of those bytes is a certified producer
output (`established`). It also checks the compiler-read document. The checks
before the compile only decide whether to observe at all.

## Decision (proposed)

A native unit does not wait before its compile. It copies the regular
artifacts present in its lookup directories at once, leaving other units'
scratch directories out of the copy. It compiles against the copies, as
today. After the compile it waits until each copied input's producer has
settled, then runs the existing certification unchanged.

A producer has settled when its certificate exists or when no driver of this
run is still producing it. Each driver in a retained run writes a mark, bound
to the run, for its possible outputs before its compiler starts, and removes
it when it ends, after any certificate it writes. A mark from an earlier run
counts as no producer. A mark whose driver died is waited for at most 120 s
and then counts as no certificate.

Verified targets keep their existing waits before compiling. Only the
producer wait now ends when the producer's mark is gone instead of after a
fixed 2 s.

## Soundness argument

**What a unit can observe before its certificate exists.** The compiler of a
native unit U reads its lookup directories only through U's private copies.
Those copies are fixed when U starts, mode 0400, and not written afterwards.
They hold exactly the regular artifacts that were present: complete files,
since rustc renames finished outputs into place. A dependency P's metadata
may be copied while P is still building its library. Crate lookup reads
files in the lookup directory and never descends into a subdirectory, so
leaving scratch directories out cannot change which crate is found. It can
only turn a lookup that would fail on an incomplete file into a lookup that
finds nothing. Either way U's compile fails or succeeds on the copies alone,
and those copies are what is certified. Admitted sources are bound
separately by the compiler-read document. U's dependents see U's outputs
before U is certified, exactly as Cargo exposes them without Sail. Nothing
in the retained arena treats an uncertified output as certified.

**What a certificate asserts, and why it is unchanged.** A native certificate
for U is written only by `Native::finish`. That happens after U's compiler
succeeded and every copied input's producer has settled, and only when all of
the following hold:
- the copies still hold their recorded bytes;
- each original input still holds those bytes;
- each input has a certificate of this context, compiler and policy whose
  recorded body is exactly those bytes, for that output path (`certifies`);
- the compiler-read document matches the admitted closure, the copies and
  the tracked environment.

These are the same predicates over the same bytes as today's checks after
the compile. The checks the prototype drops before the compile cover a
subset of them, over bytes those later checks read again in full.
Certificates are acyclic: U waits only on its inputs' producers, never on its
dependents. So no certificate can exist unless every input of every unit in
its closure was certified when that unit finished.

**Why every failure is a miss, never a wrong admission.** A producer that
fails, is refused admission, or certifies other bytes than the ones U copied
leaves U without a certificate. Rereading the original or the copy shows a
changed byte, and `certifies` refuses. A mark that cannot be written means no
wait; the certificate is then required at once, and U misses if it is absent.
A mark that cannot be removed holds dependents for at most 120 s, after
which they miss. A verified target whose graph contains an uncertified input
fails `established` and is neither retained nor restored. It then compiles
and verifies normally. Every refusal therefore costs only reuse.

## Consequences

- Cargo's metadata pipelining is kept for native units. The wait moves to
  the end of a unit's driver, where it overlaps with dependents that Cargo
  has already started.
- A unit's driver can end later than its compiler while it waits for a
  slower producer. Units that need its full completion (proc macros, linked
  outputs) wait as they would for a slow compile.
- Native graphs may now omit an input's library when only its metadata
  existed. That does not affect reuse, because a native certificate is never
  restored; native units always compile.
- The tests show that each dropped check's failure is still caught:
  `certification_after_the_compile_rejects_bytes_the_early_check_rejected`,
  `dependents_wait_only_for_producers_running_in_this_run` and
  `pipelined_capture_leaves_out_scratch_directories_but_refuses_other_kinds`.
- **Before selection:** owner review of this argument, the full gate,
  solver, e2e `managed` and `managed-copies` phases, fast and standard
  conformance in every arm, the instant-revert Vest cycle, and paired
  cold/warm timings on Linux and macOS.
