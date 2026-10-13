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
- **A project's first managed verify verifies vstd.** As with plain `cargo
  verus verify` on a fresh target, the first managed run of each project
  verifies vstd in its own Cargo target (about 31 s natively). Later runs from
  the CLI, MCP or any agent session reuse it. A shared per-toolchain vstd is a
  deferred follow-up. [L16 report](reports/rescue/l16-first-contact-vstd.md).
- **Engine verification re-verifies vstd on every run.**
  - `sail verify --engine interactive` and `repair-check` report two verifier
    invocations and no reused target on every run.
  - Each run gets a new `input_revision`, although no file changed and the
    context digest is stable.
  - On HumanEval a run takes about 33 s, against 5.7 s for stock managed.
  - Seen on `e20d849` with an engine built from fork `3a91b1d7`. Cause not
    established. [L18 report](reports/rescue/l18-m8-stage-profile.md).
- **The first passing stock run after a failing run re-verifies vstd.**
  - It cost +27.5 s on HumanEval and on Vest in 12 of 12 such native
    samples.
  - It affects the agent's fix-after-failure cycle.
  - Cause not established.
    [L18 report](reports/rescue/l18-m8-stage-profile.md).
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

## Resolved by decision

- **A cache-warm run can print a unit's errors in another module order.**
  `debug_expand.rs` sometimes printed the root module's error before `M4`'s
  with the same messages and verdicts
  ([VI-0007](verus-issues/VI-0007-nondeterministic-error-order.md)).
  Conformance compares messages as a multiset since
  [ADR 0042](decisions/0042-diagnostics-compared-as-a-set.md), so this order
  is no longer a difference.
  [Repro](../corpus/repros/sail-cache-warm-error-order/) (kept).
