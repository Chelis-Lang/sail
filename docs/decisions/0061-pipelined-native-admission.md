# 0061: Certify native units after their compile

- **Status:** accepted by the owner 2026-10-20; implemented by `perf/pipelined-native` (supersedes the #265 prototype)
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

## Decision

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
fixed 2 s. A target whose dependencies were built by an earlier run without
certificates (for example a forced first run) therefore refuses at once
instead of waiting 2 s on every later run (#285).

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

## Prototype measurement (#265)

These are native c7i.8xlarge runs in us-east-1 with runtime `9c0ca7e7`, three
interleaved rounds each, comparing the prototype (`7e0dde85`) with the #256
branch (`8c318739`) and stock. Values are medians in seconds, with the range.

| Scenario | Stock | #256 | Prototype |
|---|---:|---:|---:|
| Demo, first run | 31.88 (31.60–31.97) | 34.36 (34.35–34.50), +7.8% | 33.86 (33.68–33.91), +6.2% |
| Demo, unchanged rerun | — | 0.215 | 0.217 |
| Vest A, cold | 72.54 (72.35–73.15) | 75.91 (75.70–76.44), +4.6% | 75.27 (74.91–75.54), +3.8% |
| Vest B, edit | 40.58 | 25.83 | 25.86 |
| Vest A, returned | 40.51 | 1.643 | 1.656 |
| Vest A, unchanged | 0.222 | 0.528 | 0.531 |
| Vest, comment no-op | 40.44 | 24.74 | 24.69 |

Vest uses the public profile with `-- --rlimit 100`. The default rlimit fails
`parse_cbor_with_child` in stock and Sail alike
(VI-0032, added by #257). Every call
exits 0, and every Sail run's stdout and stderr equal stock's after
normalization. A fresh demo run certifies the same 29 producer outputs and
retains the project target in both arms. On the demo, vstd starts about
0.55 s earlier. In the prototype's first version the extern-name check still
ran before the compile, and certification stopped after the first pipelined
dependent. That check now runs after the producers settle.

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
  `dependents_wait_only_for_producers_running_in_this_run`,
  `pipelined_capture_leaves_out_scratch_directories_but_refuses_other_kinds`,
  `a_dependent_of_a_failed_producer_is_never_certified` (a producer that
  rewrites its metadata after a dependent copied it, then certifies the new
  bytes) and
  `running_marks_cover_every_dependent_visible_output_and_end_with_the_driver`.
- Selection gates (owner review, the full gate, solver, every e2e phase,
  fast and standard conformance in every arm, the instant-revert Vest cycle,
  and paired cold timings on Linux and macOS) are recorded in the
  [cold first-run record](../validation/cold-first-run.md#pipelined-native-admission-adr-0061).

Raw evidence: native evidence bucket,
`experiments/2026-10-20/pipelined-admission/pipe-evidence.tgz`, SHA-256
`9d043354596540526003ce545e3bb075c7618e154773b7dbb82a4eb143745d77`.

## Shipped-proof producer path (2026-10-21)

The same native-producer ordering applies when observing native units for a
shipped vstd lookup. That observer previously retained the precompile waits.
It now captures the present regular artifacts with `capture_pipelined` and
compiles against the private copies. After successful compilation, publication
still waits for the copied inputs' producer records and requires the existing
complete graph, source, environment, compiler and output checks. The vstd
consumer's certificate and consumed-byte requirements are unchanged. A missing
or failed producer therefore still prevents reuse. Native performance and
both-platform qualification of this implementation are pending.
