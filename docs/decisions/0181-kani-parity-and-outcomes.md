# 0181: Kani parity with direct Kani, and the outcome taxonomy

- **Status:** accepted for implementation (Kani phase checkpoint, owner approval 2026-11-28)
- **Date:** 2026-11-28
- **Extends:** [ADR 0156](0156-fresh-uncached-kani-cli.md), [ADR 0180](0180-kani-phase-scope.md)

## Context

The Kani phase design (section 5) sets the correctness contract for the Kani
backend: Sail must give the same per-harness results as direct Kani at the
same pins. ADR 0156 named eleven export fields that differed between one
Sail run and one direct run of the same harness. A comparison that allows a
field to differ because Sail differed on it proves nothing; the allowance has
to come from the oracle itself, the way ADR 0009 treats plain Verus.

On the fast corpus (2026-11-28, `docs/validation/kani-conformance.md`) two
direct runs of the same harness differ only in timing values, the run's own
paths, and the order of Cargo's `Compiling` lines, which follows Cargo's
parallel job scheduling. Spike S3 found one more field that differs between
direct rebuilds: rustc table indices printed inside the vtable-size sanity
check's description.

## Decision

**Oracle.** Direct stock Kani at the same pin, with the same argv,
environment and project bytes, run at least twice per harness, without Sail.

**Parity.** For each harness, everything below is exact between Sail and a
direct run: driver exit status and signal; every check's property id (from
`Check N:`), class (`category`), function, description, location and status;
the check count and the failed-check list; the backend configuration (solver,
object bits) and the rest of the export, including property counts and CBMC
statistics other than timings; every stdout and stderr line, with its order,
count and line ending; and the driver's argv and environment with only the
run's own directory differing.

**Variance.** A field may differ only if it is named individually and two
direct runs of the same project were seen to differ on it in the same report
(the witness rule). Each name says how it may differ: a finite nonnegative
number, a timestamp, the same path below each run's own directory, a
multiset. The named set is `VARIANCE` in
`crates/sail-conformance/src/kani/compare.rs`: ADR 0156's eleven export
fields; the six CBMC `Runtime` lines, `Verification Time`, Cargo's
`Finished` duration and `Reading GOTO program` path in the streams; the
order of Cargo's `Compiling` lines (compared as a multiset); and the
vtable-size check description with its `id: N` indices masked. Adding a name
needs a measured direct-versus-direct difference, recorded in the validation
record. A named field that differs only against Sail makes the harness
`different`.

**Verdicts per harness.** `same`: Sail's claim and every compared field match
the first direct run, and the two direct runs agree outside the named fields.
`unstable`: the direct runs differ outside the named fields and Sail matches
one of them; reported, not counted as a Sail difference. `different`: any
other difference. `no-claim`: Sail made no claim where direct Kani made one;
a downgrade, counted on its own, never `same`; when the driver ran to
completion under Sail its raw output is still compared, and a difference
there makes the harness `different`. `infra-error`: a direct run could not
complete. A project the shared argv cannot build is reported as blocked:
with the same argv both arms fail alike, so there is nothing to compare.

**Outcome taxonomy.** `bounded_success` and `counterexample` carry tier
Bounded. `bound_insufficient` completes with no tier. No claim, never cached
and never reported as success: `timeout`, `out-of-memory`, `crash`,
`cancelled`, `unsupported` (a harness kind or tool setting not admitted
yet), `build-failure`, `integrity` (tool identity or output integrity did
not hold) and `refused` (another stated reason). "Verified" never appears.
Direct Kani's claim is read from its own exit status, verdict line, export
result and failed-check categories, never through Sail's decoder.

## Consequences

- The fast tier is the gate for every Kani behaviour change: every admitted
  harness `same` over three rounds, with no `different` or `infra-error`.
- A batch executor or reuse layer changes only the Sail side; the comparison
  and its variance set stay as they are.
- Integrity supervision (ADR 0182) will add no-claim outcomes where direct
  Kani prints success. Those are `no-claim` here, the registered
  downgrade-only divergence, never `different`.
- The fault arm shows that the decoder alone admits two of S6's false-success
  paths; only tool authentication keeps them out of Sail today. ADR 0182 has
  to close them at the CBMC boundary.
