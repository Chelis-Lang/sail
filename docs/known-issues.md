# Known issues and acceptance limits

This page lists the defects and validation failures that are open on the M7
development branch, with one pointer each. Each gate's status, best evidence
and indexed failed attempts are in [M7 validation](validation/m7.md); the
[roadmap](ROADMAP.md) tracks the remaining work. Fixed and superseded issues
are in the `archive/m7-contracts-f7103bf` tag.

## Open M7 defects and failing gates

- **Vest repair is 1.80× M6, below the 2× gate.** Three native rounds on the
  pinned fork runtime, every sample valid and Current: HumanEval 2.13× (pass),
  Vest 1.80× (each Vest cycle about 29 s against 53 s).
  [Gate 7](validation/m7.md#gate-7-frozen-repair-performance),
  [L12 report](reports/rescue/l12-fork-fixes.md).
- **A cache-warm run can print a unit's errors in another module order.** In
  one of seven standard-tier cache-warm runs, `debug_expand.rs` printed the
  root module's error before `M4`'s; plain Verus printed `M4`'s first in all
  19 runs. Same errors and verdicts. Suspected cause: stock's report-order
  printing ([VI-0007](verus-issues/VI-0007-nondeterministic-error-order.md))
  under warm-cache timing; not established.
  [Repro](../corpus/repros/sail-cache-warm-error-order/).
- **A project's first managed verify verifies vstd.** As with plain `cargo
  verus verify` on a fresh target, the first managed run of each project
  verifies vstd in its own Cargo target (about 31 s natively). Later runs from
  the CLI, MCP or any agent session reuse it. A shared per-toolchain vstd is a
  deferred follow-up. [L16 report](reports/rescue/l16-first-contact-vstd.md).
- **Repair selectors lose compiler-declared input coverage.** A warmup can
  return Historical full evidence after exit-zero verification. See `b89546c`
  in [Gate 7](validation/m7.md#gate-7-frozen-repair-performance) and
  [#77](https://github.com/Chelis-Lang/sail/issues/77). Not yet rechecked
  after ADR 0041.
- **Intermittent shared HTTP deadline failures.** The 250 ms
  lookup/coalescing fixtures have failed in many complete gates and passed in
  others. Cause unknown; the deadlines are unchanged.
  [Gate 3](validation/m7.md#gate-3-workspace-gate-solver-suite-and-native-ci), [#76](https://github.com/Chelis-Lang/sail/issues/76).
- **Verifier wait under amd64 emulation.** Verification sometimes stalls under
  amd64 emulation, which is no longer used for performance or Linux
  validation (ADR 0038). [#68](https://github.com/Chelis-Lang/sail/issues/68),
  [reproduction](../corpus/repros/linux-emulation-verus-stall/).
- **Managed metadata ignores Cargo `-F` feature aliases.**
  [#78](https://github.com/Chelis-Lang/sail/issues/78).

## Product limitations

- Current means the files a result depended on are unchanged; it is separate
  from the verdict. In doubt a result reads back Unknown ("inputs could not be
  confirmed unchanged; re-run"), never Current. A few rare changes during a
  run are not detected; see [freshness](freshness.md) and [trust](trust.md).
- A successful focused check still needs complete Current full validation.
- There is no managed compiler-target quota or generation GC. See
  [disk use and cleanup](workspace-hygiene.md#managed-compiler-targets).

## Verus and Z3

[The Verus tracker](verus-issues/README.md) lists every locally recorded
upstream issue, with its reproduction and workaround. Sail preserves raw
diagnostics and refuses uncertain attribution or coverage. Z3 4.16.0 can vary
nonlinear rlimit consumption on identical input; see the
[reproduction](../corpus/repros/z3-nla-rlimit-nondeterminism/README.md) and the
[determinism policy](decisions/0009-solver-nondeterminism-and-unstable-results.md).
Seed-dependent proof discovery is available only through named opt-in
portfolios, with separate keys.
