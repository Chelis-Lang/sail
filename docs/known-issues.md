# Known issues and acceptance limits

This page lists the defects and validation failures that are open on the M7
development branch, with one pointer each. Each gate's status, best evidence
and indexed failed attempts are in [M7 validation](validation/m7.md); the
[roadmap](ROADMAP.md) tracks the remaining work. Fixed and superseded issues
are in the `archive/m7-contracts-f7103bf` tag.

## Open M7 defects and failing gates

- **The CI image cannot be built from this branch yet.** The engine source
  (`interactive.patch`, and soon the Verus fork) no longer matches the pinned
  runtime in `engine/verus/artifact-linux.json`, so the image's engine check
  refuses it (`SAIL0944`). Lanes run gates against a locally built engine
  until the rebuilt runtime is pinned (ADR 0039).
- **M7 speed is measured in one round only.** One native round per catalogue
  after the rescue work: HumanEval M7 3.46× M6, Vest 1.90×, with every sample
  valid. The gate needs three rounds at ≥2× on each catalogue.
  [L7c report](reports/rescue/l7c-managed-overhead.md),
  [Gate 7](validation/m7.md#gate-7-frozen-repair-performance).
- **Agent sessions re-verify vstd and MCP re-verifies every call.** The proof
  context includes nearly every environment variable, so each new agent
  session or terminal gets a new Cargo target and verifies vstd again (about
  35–43 s); MCP's blocking `verify` also forces a fresh run on every call
  (9.1 s against 1.7 s from the CLI on HumanEval). A fix is in progress.
  [L10 report](reports/rescue/l10-ci-diet.md).
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
