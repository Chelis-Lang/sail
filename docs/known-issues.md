# Known issues and acceptance limits

This page lists the defects and validation failures that are open on the M7
development branch, with one pointer each. Each gate's status, best evidence
and indexed failed attempts are in [M7 validation](validation/m7.md); the
[roadmap](ROADMAP.md) tracks the remaining work. Fixed and superseded issues
are in the `archive/m7-contracts-f7103bf` tag.

## Open M7 defects and failing gates

- **Real-project repair benchmark refuses before any sample.** At `387ba80`,
  the first measured HumanEval GCD is Current and accepted but uses three
  compiler invocations against the one-driver requirement. No samples have
  been measured and Vest has never been reached. Cause unknown.
  [Gate 7](validation/m7.md#gate-7-frozen-repair-performance), [#73](https://github.com/Chelis-Lang/sail/issues/73).
- **Repair selectors lose compiler-declared input coverage.** A warmup can
  return Historical full evidence after exit-zero verification. See `b89546c`
  in [Gate 7](validation/m7.md#gate-7-frozen-repair-performance) and
  [#77](https://github.com/Chelis-Lang/sail/issues/77).
- **Full Result reads miss the 200 ms p95 target.** At `387ba80`, Python Result
  p95 is 436.008 ms, measured under emulation. Cause unknown.
  [Gate 5](validation/m7.md#gate-5-responsive-reads), [#69](https://github.com/Chelis-Lang/sail/issues/69).
- **Standard-tier conformance is incomplete.** Stock Verus waits on the
  Verismo tspec unit until its 3,600 s limit. Cache fill/warm on the standard
  tier has never completed.
  [VI-0013](verus-issues/VI-0013-verismo-tspec-stock-wait.md), [Gate 6](validation/m7.md#gate-6-default-and-cache-conformance).
- **Native gate failure on `rescue/m7`.** `restored_output_write_invalidates_both_overlapping_input_observations`
  (`sail-daemon`) failed on a native x86-64 host and passed under emulation.
  Cause unknown. [Gate 3](validation/m7.md#gate-3-workspace-gate-solver-suite-and-native-ci).
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
