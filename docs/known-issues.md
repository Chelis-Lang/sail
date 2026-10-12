# Known issues and acceptance limits

This page lists the defects and validation failures that are open on the M7
development branch, with one pointer each. Each gate's status, best evidence
and indexed failed attempts are in [M7 validation](validation/m7.md); the
[roadmap](ROADMAP.md) tracks the remaining work. Fixed and superseded issues
are in the `archive/m7-contracts-f7103bf` tag.

## Open M7 defects and failing gates

- **Engine diagnostics name `vstd/seq.rs` where stock names `seq.rs`.** The
  engine conformance arm found this on `basic_failure.rs`. It is a diagnostic
  parity defect in engine mode; stock mode is unaffected.
  [L4 report](reports/rescue/l4-engine-correctness.md).
- **The byte oracle trips on stock trigger nondeterminism.** On
  `guide/traits.rs` (7 of 10 runs) and `chapter-6-1.rs`, stock Verus itself
  chooses triggers nondeterministically
  ([VI-0015](verus-issues/VI-0015-nondeterministic-trigger-choice.md));
  verdicts match. The fresh-versus-fresh classification in ADR 0038 is
  pending, and the oracle cannot start on `vest-generated`.
- **The 512 MiB preparation budget is not an established bound.** It is a
  configured limit only; nothing measures process memory against it.
- **M7 is not faster than M6 yet.** Natively, the HumanEval repair batch is
  0.94× of M6, because preparation encoding costs more than the single callback
  saves. On Vest, M7 refuses every function (`SAIL3122`) and loses Currentness
  (`SAIL3114`).
  [L1 profile](reports/rescue/l1-phase-profile.md), [Gate 7](validation/m7.md#gate-7-frozen-repair-performance).
- **Managed focus can fail with `SAIL3107` after a repeated edit.** `cargo verus
  focus` builds under `<target-dir>/verus-partial` with checksum freshness, but
  the missing-receipt retry cleans `<target-dir>`. Byte-identical content then
  produces no receipt. This affects released M6 behaviour.
  [L1 profile](reports/rescue/l1-phase-profile.md).
- **Repair selectors lose compiler-declared input coverage.** A warmup can
  return Historical full evidence after exit-zero verification. See `b89546c`
  in [Gate 7](validation/m7.md#gate-7-frozen-repair-performance) and
  [#77](https://github.com/Chelis-Lang/sail/issues/77).
- **Intermittent shared HTTP deadline failures.** The 250 ms
  lookup/coalescing fixtures have failed in many complete gates and passed in
  others. Cause unknown; the deadlines are unchanged.
  [Gate 3](validation/m7.md#gate-3-workspace-gate-solver-suite-and-native-ci), [#76](https://github.com/Chelis-Lang/sail/issues/76).
- **Verifier wait under amd64 emulation.** Verification sometimes stalls under
  amd64 emulation. [#68](https://github.com/Chelis-Lang/sail/issues/68),
  [reproduction](../corpus/repros/linux-emulation-verus-stall/).
- **Managed metadata ignores Cargo `-F` feature aliases.**
  [#78](https://github.com/Chelis-Lang/sail/issues/78).
- **Thread-local span formatting in interactive preparation.**
  [#75](https://github.com/Chelis-Lang/sail/issues/75),
  [reproduction](../corpus/repros/engine-span-data-worker/).
- **The engine's memory bound is not physically established.** Diagnostic
  reporters release quota before their nested allocations drop, so the 512 MiB
  preparation bound is accounting only. The corrective overlays were never
  selected. [Engine internals](../engine/verus/README.md#prepared-lowering-checkpoint).
- **Imported metadata ranges are uncovered.** Some imported metadata ranges
  are not covered, so preparation misses and falls back to fresh lowering.
  [VI-0014](verus-issues/VI-0014-exported-foreign-span-metadata-gap.md).
- **Unknown reports lack attribution.** Namespace and sysroot Unknown reports
  name the reason and the first failed barrier, but not the file or event.
  [Freshness](freshness.md).
- **Unsupported ordinary-build coverage regenerates.** Build scripts whose
  outputs cannot be fully observed are rerun, and their proofs stay Unknown.
  [Freshness](freshness.md).

## Product limitations

- Current is an input-coverage claim, separate from a solver verdict. Missing
  coverage, lost leases or uncertain declarations produce Unknown. A saved proof
  never becomes a proof of changed inputs. See [freshness](freshness.md) and
  [trust](trust.md).
- Linux retention needs exact kernel and namespace coverage, including lease
  permission for runtime files. An ordinary user may be unable to lease
  system-owned files. See the [native product setup](native-product-setup.md).
  macOS takes a conservative path and may need fresh verification.
- A successful focused check still needs complete Current full validation.
- There is no managed compiler-target quota or generation GC. See
  [workspace hygiene](workspace-hygiene.md#managed-compiler-targets).

## Verus and Z3

[The Verus tracker](verus-issues/README.md) lists every locally recorded
upstream issue, with its reproduction and workaround. Sail preserves raw
diagnostics and refuses uncertain attribution or coverage. Z3 4.16.0 can vary
nonlinear rlimit consumption on identical input; see the
[reproduction](../corpus/repros/z3-nla-rlimit-nondeterminism/README.md) and the
[determinism policy](decisions/0009-solver-nondeterminism-and-unstable-results.md).
Seed-dependent proof discovery is available only through named opt-in
portfolios, with separate keys.
