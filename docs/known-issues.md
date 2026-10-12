# Known issues and acceptance limits

This page lists the defects and validation failures that are open on the M7
development branch, with one pointer each. Each gate's status, best evidence
and indexed failed attempts are in [M7 validation](validation/m7.md); the
[roadmap](ROADMAP.md) tracks the remaining work. Fixed and superseded issues
are in the `archive/m7-contracts-f7103bf` tag.

## Open M7 defects and failing gates

- **M7 is not faster than M6 yet.** In a one-round native smoke run,
  HumanEval was 1.01× of M6 and Vest 0.77×, and every Vest M7 sample is
  invalid (`SAIL7102`, see below). The speed work (preparation removal and
  managed overhead) has not landed yet.
  [L7b report](reports/rescue/l7b-vest-and-baseline.md), [L1 profile](reports/rescue/l1-phase-profile.md),
  [Gate 7](validation/m7.md#gate-7-frozen-repair-performance).
- **An agent's first managed verify re-verifies vstd.** The first `sail mcp`
  verify of a project takes about 49 s even when every solver check is a
  cache hit; `sail verify` from the CLI takes 0.5 s. A managed job builds in
  its own Cargo target, so its first run verifies vstd again.
  [L10 report](reports/rescue/l10-ci-diet.md).
- **Vest results read back through the daemon are always Unknown.** The
  read-time freshness check needs leases on every file in the workspace,
  beyond its limits. A simpler check, scoped to the files Cargo used for the
  target, is in progress. [L7b report](reports/rescue/l7b-vest-and-baseline.md).
- **Engine fixes await a rebuilt engine runtime.** The pinned runtime
  predates the engine changes in source: prepared-lowering reuse and the
  same-input oracle are removed, engine diagnostics in vstd match stock, and
  repeated friendly names (Vest trait impls) no longer refuse focused repairs
  with `SAIL3122`. Until the runtime is rebuilt from the Verus fork
  (ADR 0039) and re-pinned, the CI image cannot be built from this branch.
  [L7a report](reports/rescue/l7a-preparation.md), [L4b report](reports/rescue/l4b-engine-parity.md).
- **Repair selectors lose compiler-declared input coverage.** A warmup can
  return Historical full evidence after exit-zero verification. See `b89546c`
  in [Gate 7](validation/m7.md#gate-7-frozen-repair-performance) and
  [#77](https://github.com/Chelis-Lang/sail/issues/77).
- **Intermittent shared HTTP deadline failures.** The 250 ms
  lookup/coalescing fixtures have failed in many complete gates and passed in
  others. Cause unknown; the deadlines are unchanged.
  [Gate 3](validation/m7.md#gate-3-workspace-gate-solver-suite-and-native-ci), [#76](https://github.com/Chelis-Lang/sail/issues/76).
- **Flaky tests on a loaded Mac.** `sail-api`'s
  `cancellable::blocked_read_and_write_cancel…` and `sail-verus`'s
  compiler-family and shared-target lock tests fail intermittently ("would
  block") under heavy load on macOS; they pass on native Linux. Under
  investigation.
- **Verifier wait under amd64 emulation.** Verification sometimes stalls under
  amd64 emulation, which is no longer used for performance or Linux
  validation (ADR 0038). [#68](https://github.com/Chelis-Lang/sail/issues/68),
  [reproduction](../corpus/repros/linux-emulation-verus-stall/).
- **Managed metadata ignores Cargo `-F` feature aliases.**
  [#78](https://github.com/Chelis-Lang/sail/issues/78).
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
