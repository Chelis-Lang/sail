# VI-0040: Mac abstract-domain proof differs between stock and the fork arm

- **Found:** 2026-10-31
- **Runtime:** stock `7d829946`, fork `47fb3be6`, Rust 1.99.1, pinned Z3;
  native arm64 macOS 27, mac-m4.metal / us-east-1c
- **Status:** solver-history trigger isolated; bounded compiler correction under qualification
- **Tracks:** [#634](https://github.com/Chelis-Lang/sail/issues/634)

The [pinned-package reproduction](../../corpus/repros/abstract-domain-mac-verdict/README.md)
compares semi-persistent's `abstract-domains` crate without modifying its
proofs. Stock passes all three captured runs. The fork with the managed
verifier flags and shim fails
`semi_persistent_abstract_domains::tnum::Tnum::add_carry_bitwise_has_eq`:
a postcondition error at `abstract-domains/src/tnum.rs:729:17`, exit 101,
1,171 verified / 1 error versus stock's exit zero and 1,172 / 0.
The comparator covers 909 function records in this unit and remains failed.
This is not the accepted VI-0003 timing-entry exception.

The complete Mac candidate comparison reports 22 same / 1 different,
5,357 function records, no unstable/infrastructure/skipped result, with
resource drift in 13 units. Linux passes these pins. This verifier-level
comparison does not execute managed artifact-digest reuse. Completed controls pass with stock/bare-shim and stock/managed-flags, but
fail with fork/bare-shim and fork/managed-flags. The fork also fails with
the pinned stock Z3 executable used directly, without the Sail shim or
managed flags. This localizes the observed behavior outside the cache
prototype and shim; literal replay below isolates the preceding solver history.

The original output and every stock confirmation remain external, archive
SHA-256 `388e2a007aa3dea05168d33876134451c4465c13322cea9137f1b7575ee212d8`.
Independent stock/shim and fork/bare controls are diagnostics, not alternate
acceptance profiles. No resource limit increase, proof port, output filter,
query normalization or Different-to-Same conversion is selected. Default
platform performance/release acceptance remains unqualified at this gate.

Complete final native Mac controls/reductions archive SHA-256:
`f71416129dfb411543e700962b76374bade14793830a58467b95b9cdd049bb30`.
The original failed profile remains the acceptance result; passing stock
diagnostics are not substituted for it.

## Isolated trigger and bounded correction

The first failing proof query itself is byte-identical across stock/fork and
Linux/Mac, SHA-256
`f4950172df4f0ddb03b9f00b477023c79af08da6505235ec9c79a42c424041bb`.
The complete stock SMT input also matches between platforms. The Mac pinned
Z3 replays original stock input as `unsat` and original fork input as `unknown`
with `incomplete quantifiers`. Restoring stock's formal names in **both**
earlier `Tnum::has` and `Tnum::add_carry` termination scopes recovers `unsat`;
restoring either alone does not. Removing just those two completed push/pop
scopes also recovers `unsat`, retaining the other thirteen earlier queries,
every definition axiom and the exact failing proof and resource limit.
These are offline diagnostic controls, never a Sail input-normalization path.
They establish history sensitivity, not an internal Z3 root cause.

ADR 0094 prototypes a lazy companion context for specification termination.
It receives the complete original context, then each later context batch once;
ordinary proof checks retain their own solver history. At most two copies of
a bucket's context are loaded. Options, limits, commands and failure handling
remain intact; timing/resource totals and profile graphs include both streams.
This is a general execution-strategy correction, not another program-specific
formal spelling adjustment.

Prototype `3dc89c8`, Sail `137af383`, builds on both native hosts and verifies
vstd (2,059 / 0). The unchanged full package is 909 Same on both; each complete
broader comparison is 23 Same / 5,357 records with zero unstable, Different,
infrastructure or skipped results (13 resource-drift units). One of those
23 units is the existing matching incompatible RLSF compile, not a passing
allocator. Source-test fixture errors were corrected before qualification.

The complete `c6649ed8` Linux source run then exposed one genuine integration
regression: the unchanged CVC5-SMT-LIB-on-Z3 fixture expected one solver log
and saw two (4,905 Same / one Different; 4,842 nonignored passing tests / one
failure). Candidate `4cd6bd5f` restricts the companion to both the Z3 solver
and Z3 encoding; the original CVC5 fixture is unchanged. All 40 targeted
regressions and 146 AIR tests pass on both native platforms. Its actual
engine builds and release factories pass on both; the complete Linux suite,
remaining corpus/cost controls and final runtime adoption remain in progress.
Neither candidate is selected or shipped. The original failures stay retained.

Original full logged input/transcripts are retained externally: Linux archive
SHA-256 `e0fe9977a540b86b1782505de40441c82cb776d1bb9bed76f2aab9b08b836434`,
Mac `0d854567f9c226ec4eb1b60d779b26cd0811c38a1af894e83a4bbf570f93de12`.
