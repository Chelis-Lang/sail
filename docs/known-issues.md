# Known issues and acceptance limits

This page lists the defects and validation failures that are open on the M7
development branch, with one pointer each. Each gate's status, best evidence
and indexed failed attempts are in [M7 validation](validation/m7.md); the
[roadmap](ROADMAP.md) tracks the remaining work. Fixed and superseded issues
are in the `archive/m7-contracts-f7103bf` tag.

## Open M7 defects and failing gates

- **Changed-check continuation remains unqualified** ([#158](https://github.com/Chelis-Lang/sail/issues/158)).
  Historical fork continuations have raw statistics differences. Three fresh
  full runs per real IronKV/Anvil trace also differ in literal clock bytes,
  so both one-use live samples were skipped; one IronKV sample additionally
  retains a memory-observer exit refusal. The strict gate stays failed and
  the prototype stays archived, with no default integration. See the
  [qualification record](validation/solver-snapshots.md).
- **rc.5 omits verifier stdout after dependency-store seeding** ([#184](https://github.com/Chelis-Lang/sail/issues/184)).
  A second new project reuses vstd but omits its `verification results::`
  line; plain stock on a fresh target prints it. Verdicts, exit codes and
  stderr matched in the reproduction. The source correction is merged and
  passed both-platform checks, full native fast/standard conformance and
  required PR checks. The original draft asset is unchanged;
  [evidence](validation/dependency-store-output.md).
- **Native-linker changes can be missed by dependency-store seeding** ([#187](https://github.com/Chelis-Lang/sail/issues/187)).
  On source `0a7c1d0d` plus stdout fix `2f7102c4` (macOS 26.5, Apple M3,
  pinned fork `73dea021`), replacing private `cc` bytes at unchanged `PATH`
  after a successful donor made fresh stock exit 101 with a linker error,
  while store-seeded Sail exited 0 without that diagnostic. Current main
  still uses v1 native-artifact seeding by source inspection; today's exact
  main was not rerun. Raw archive SHA-256
  `7bea96ebf3bd10a292ec6508bb7322a8a799fbc25ef024a89cf256d90783f51c`.
  The unmerged #187 replacement has a separate APFS mmap guard blocker;
  the #166/#189 metadata correction fixes neither defect.
- **Draft rc.5 can hide source inputs through output-directory exclusions (#189).**
  Adding an alternative module under a configured Cargo target or Sail cache
  can reuse an old proof although fresh compilation rejects it with E0761.
  The source correction uses owned metadata targets and refuses managed
  reuse when Sail cache and package roots physically overlap. Native
  full fast/standard and the separate joined main checks pass; the original
  draft asset is unchanged. Cache/package overlap in either direction remains
  an unsupported setup, taking SAIL3112 fallback without Current proof state.
  See the [validation record](validation/cargo-target-context.md).

- **Vest repair is about 1.8× M6, below the 2× figure.** The owner accepted
  M7's speed as measured (ADR 0038 amendment 2026-10-13). On the `13a1176c`
  runtime, three native rounds, every sample valid and Current: HumanEval
  2.14×, Vest 1.82× (each Vest cycle about 28 s against 51 s). L18's cheap
  items cannot close the gap without new mechanism: rustc incremental is
  already on, and Sail's whole overhead is smaller than the gap.
  [Gate 7](validation/m7.md#gate-7-frozen-repair-performance); L22 and L23
  (rescue lane report, tag `archive/rescue-m7-98f6292`).
- **`sail verify` costs a little on small projects and unchanged runs.**
  Managed verification is the only `sail verify` (ADR 0046). On a native
  c7i.4xlarge (rc.5, [performance](performance.md)), a small project's edit
  takes 1.17 s against plain Verus's 0.88 s, an unchanged re-run 0.36 s
  against 0.12 s (vest: 0.97 s against 0.22 s), and a cold run 1–2 s more.
  Solver-heavy edits are faster than plain Verus (vest 27.8 s against
  42.1 s). Measured before rc.5, they were 1.4 s slower than the removed
  pass-through (26.4 s in the same rotation): about 0.8 s of receipt and
  input checks and 0.3 s of Verus's `--output-json --time`. Where the time
  goes is in
  [ADR 0047](decisions/0047-attribution-logs-on-request.md#measurements).
- **Some setups get no proof reuse.** Tests, benches, examples,
  `--target-dir`, Verus `--log` options, `--no-verify`, Cargo `--config`
  overrides and loader variables such as `LD_LIBRARY_PATH` are verified as
  plain `cargo verus verify` verifies them, with one `note[SAIL3112]` line,
  but nothing is reused or recorded. Jobs, `watch` and MCP report all but
  the loader variables as `SAIL3112` errors ([workspace verification](workspace-verification.md#setups-verified-without-proof-reuse)).
- **The first project on a machine verifies vstd.** As with plain `cargo
  verus verify` on a fresh target, the first `sail verify` on a machine
  verifies vstd (about 33–40 s natively). Later new projects and worktrees
  that resolve the same vstd start from Sail's dependency store
  ([ADR 0045](decisions/0045-dependency-store.md); 1.4 s for a new demo
  project on rc.5). Some workspaces get a store key of their own and verify
  vstd once more on their first run: on rc.5 the workspace of issue #121
  did (release-candidate finding 15; which key input differed was not
  checked; see also [#177](https://github.com/Chelis-Lang/sail/issues/177)). Shipping a
  pre-verified vstd in the archive is
  [#161](https://github.com/Chelis-Lang/sail/issues/161).
- **The repair benchmark's outcome check fails on output schema v2.** It
  hashes the per-function `tier` that schema v2 renamed, so every M6/M7 pair
  reports `SAIL7107` although verdicts and diagnostics match. Harness only;
  release-candidate finding 13,
  [#179](https://github.com/Chelis-Lang/sail/issues/179).
- **Reports omit the release suffix (low).** `sail_version` in reports is
  the workspace version (`0.1.0`), not the build's `sail --version`
  (`0.1.0-rc.5`). [#180](https://github.com/Chelis-Lang/sail/issues/180).
- **The deep stock oracle can report unseen JSON output-order variants as Different.** The
  `output_json` tests print `func-details` in stock Verus's hash order
  ([VI-0008](verus-issues/VI-0008-func-details-hash-order.md)); with 3–6
  orders in 11 plain runs, a Sail-side run often shows an unseen one. On
  rc.3, six of eight rounds reported one Different, each with identical
  stdout and stderr that is a line permutation of plain's. rc.5's one round had 0 Different and 4 Unstable `output_json` tests. The arm runs the
  stock source build, which Sail no longer runs (ADR 0043). Release-candidate
  finding 11. The new [deep fork gate](deep-verus-tests.md) compares the fork
  with and without the shim exactly, and retains the original stock-with-and-
  without-shim control with ADR 0042's diagnostic-message comparison.
  JSON output from stock is not normalized. Native acceptance of that new
  gate is pending; the historical stock-only results do not qualify it.
- **Fork JSON proof-note arrays are not deterministic.** Eight repeated
  runs of the same failing proof emit both note orders; the exact source-suite
  sample has three Differences. No verdict or rendered-diagnostic difference
  was observed. A reviewed fork correction is pending runtime qualification
  ([VI-0021](verus-issues/VI-0021-proof-note-json-order.md),
  [#185](https://github.com/Chelis-Lang/sail/issues/185)).
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
- **Intermittent shared HTTP deadline failures.** The 250 ms
  lookup/coalescing fixtures have failed in many complete gates and passed in
  others. Cause unknown; the deadlines are unchanged.
  [Gate 3](validation/m7.md#gate-3-workspace-gate-solver-suite-and-native-ci), [#76](https://github.com/Chelis-Lang/sail/issues/76).
- **Verifier wait under amd64 emulation.** Verification sometimes stalls under
  amd64 emulation, which is no longer used for performance or Linux
  validation (ADR 0038). [#68](https://github.com/Chelis-Lang/sail/issues/68),
  [reproduction](../corpus/repros/linux-emulation-verus-stall/).
- **Release-candidate wording gaps (low).** A failing managed summary leads
  with `0 verified` before `… passed by Verus's totals`; after a Rust compile
  error the managed count line reads `0 verified, 0 failed, 0 unknown`; and
  the README's Install assumes `curl` and apt package lists, which a minimal
  container lacks. Findings 5, 8 and 9 of the
  [release-candidate record](validation/release-candidate.md#rc1-findings).

## Product limitations

- **Repair selector inventories have a bounded source scope.** Compiler-family
  imports cover package sources and generated inputs with validated build-script
  contracts. An arbitrary external include may require a full run in a new
  selector context before Current evidence is available; changed external bytes
  still force verification. Closing #77 qualifies the original generated-input
  defect, not this unsupported external-path extension.

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
