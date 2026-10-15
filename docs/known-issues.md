# Known issues and acceptance limits

This page lists the defects and validation failures that are open on the M7
development branch, with one pointer each. Each gate's status, best evidence
and indexed failed attempts are in [M7 validation](validation/m7.md); the
[roadmap](ROADMAP.md) tracks the remaining work. Fixed and superseded issues
are in the `archive/m7-contracts-f7103bf` tag.

## Open M7 defects and failing gates

- **Vest repair is about 1.8× M6, below the 2× figure.** The owner accepted
  M7's speed as measured (ADR 0038 amendment 2026-10-13). On the `13a1176c`
  runtime, three native rounds, every sample valid and Current: HumanEval
  2.14×, Vest 1.82× (each Vest cycle about 28 s against 51 s). L18's cheap
  items cannot close the gap without new mechanism: rustc incremental is
  already on, and Sail's whole overhead is smaller than the gap.
  [Gate 7](validation/m7.md#gate-7-frozen-repair-performance); L22 and L23
  (rescue lane report, tag `archive/rescue-m7-98f6292`).
- **A project's first managed verify verifies vstd.** As with plain `cargo
  verus verify` on a fresh target, the first managed run of each project
  verifies vstd in its own Cargo target (about 31 s natively). Later runs from
  the CLI, MCP or any agent session reuse it. A shared per-toolchain vstd is a
  deferred follow-up. The 31 s first-contact cost was measured by L16
  (rescue lane report, tag `archive/rescue-m7-98f6292`).
- **Default and engine runs each verify vstd once in their own target.** That
  was about 30 s on HumanEval for the first default `sail verify` and again
  for the first engine run (`sail verify --engine interactive` or
  `repair-check`), measured while default mode still ran stock Verus. Both
  now run Sail's Verus (ADR 0043); whether they can share a target is not
  yet measured. Repair functions share
  one target, so a new function no longer verifies vstd again.
  L19 measured the separate stock and engine targets and L23 the shared
  repair target (rescue lane report, tag `archive/rescue-m7-98f6292`).
- **An engine verify after a repair re-verifies the selected crate.** A repair
  rebuilds the selected crate in the shared target, so a later
  `sail verify --engine interactive` with the same arguments and unchanged
  source verifies that crate again (HumanEval 6.2 s instead of 0.6 s, Vest
  27.5 s instead of 1.8 s). Why the rebuilt bytes differ was not
  investigated. L23 (rescue lane report, tag `archive/rescue-m7-98f6292`).
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
- **Release-candidate wording gaps (low).** A failing managed summary leads
  with `0 verified` before `… passed by Verus's totals`; after a Rust compile
  error the managed count line reads `0 verified, 0 failed, 0 unknown`; and
  the README's Install assumes `curl` and apt package lists, which a minimal
  container lacks. Findings 5, 8 and 9 of the
  [release-candidate record](validation/release-candidate.md#rc1-findings).

## Product limitations

- Current means the files a result depended on are unchanged; it is separate
  from the verdict. In doubt a result reads back Unknown ("inputs could not be
  confirmed unchanged; re-run"), never Current. A few rare changes during a
  run are not detected; see [freshness](freshness.md) and [trust](trust.md).
- On macOS, a Cargo configuration file created and deleted again within one
  managed run is not detected (owner decision, ADR 0041 note of 2026-10-14).
- A successful focused check still needs complete Current full validation.
- Managed compiler targets are removed only by `sail cache projects prune`;
  there is no quota or automatic collection. See
  [disk use and cleanup](workspace-hygiene.md#managed-compiler-targets).
- Sail's Verus, like the stock release, is built without Verus's `singular`
  feature, so `by(integer_ring)` proofs fail in Sail as they do in the stock
  release. The repair engine falls back to ordinary verification for such
  targets (`SAIL3124`), which cannot accept a repair. See
  [Verus features the engine lacks](proof-repair.md#verus-features-the-engine-lacks).
- Sail runs only its own Verus, at the release it pins
  ([ADR 0043](decisions/0043-default-mode-runs-the-fork.md)). A project that
  needs another Verus release cannot use Sail yet. `SAIL_CARGO_VERUS` runs
  another `cargo-verus` only as a labelled development override
  (`warning[SAIL0108]`).
- Until the next runtime re-pin, the bundled Verus cannot run
  `cargo verus new`
  ([VI-0018](verus-issues/VI-0018-source-build-cargo-verus-new.md)); the
  README creates new projects with `cargo new` instead.

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
- **The engine rejected `by(integer_ring)` that a Singular build proves.**
  The engine runtime, like the stock release, has no `singular` feature, and
  the Singular deep run's engine arm reported all three Singular units
  Different (engine exit 1, never a false verified). The lead decided against
  a Singular engine variant: the engine manifest records its Verus features,
  a target that needs a missing one falls back to stock verification
  (`SAIL3124`), and the engine arm counts such units as
  `engine-lacks-feature`, never same or different.
  [Reproduction](../corpus/repros/engine-without-singular/) (kept),
  [deep-tier record](validation/deep-tier.md).
