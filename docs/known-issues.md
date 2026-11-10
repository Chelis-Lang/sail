# Known issues and limits

Open Sail defects and current product limits in published 0.2.0 and on
main. Fixed defects are
removed from this page; their issues and the [changelog](../CHANGELOG.md)
keep the history. The [roadmap](ROADMAP.md) tracks release work.

The accepted [shared parser](design/checked-stream-parser.md) has a complete
new-root Stock inventory and all ordinary failure controls. Its frozen native
source passes full fast/standard conformance, end-to-end and solver checks;
measured fragmentation and lexer costs are recorded. Current-main managed
runtime checks also pass. ADR 0086 accepts the bounded runtime adoption and
the documented 3.02×/3.70× fragmentation and 1.90× lexer cost tradeoff;
no general service-performance or completed-service claim follows.
Native capacity/lifetime, external owner/EOF and solver interpretation
relations remain open; source/hash binding does not establish them.

Resolved defects are retained in the [changelog](../CHANGELOG.md) and
[backlog reconciliation](reports/2026-11-04-roadmap-review.md). In particular,
0.2.0 includes the per-code explanation-note correction (#655), trailing-output
oracle completion (#611), and compound-field lowering/retained-read correction
(#323). Original failures and source-specific qualification stay unchanged.

## Accepted integration limit

- A hosted cached-session statistics control intermittently took the genuine
  solver path instead of its deliberately rewritten cache entry (#616).
  Exit status and genuine solver output were correct; the replay witness
  failed. Cause is unknown. Native solver checks pass; the strict assertion
  remains active. The fixture now binds its service completely before clients
  start; 100 unchanged-fixture and 100 corrected native repetitions pass, so
  the original cause remains unproved ([record](validation/cached-session-fixture.md)).

- **Sail's named integration settings are compiler-visible**
  ([ADR 0088](decisions/0088-compiler-visible-integration-settings.md),
  [#435](https://github.com/Chelis-Lang/sail/issues/435)). The owner accepts
  this bounded equivalence exception. The
  pinned fork and the `sail-z3` shim read these from the compiler's
  environment. Build scripts and compilers see `VERUS_Z3_PATH`,
  `SAIL_Z3_PATH`, `SAIL_SOLVER_MODE` and `SAIL_DAEMON_SOCKET`, and
  `RUSTC_WRAPPER` names Sail's driver alias. The verified compiler's
  ` __VERUS_DRIVER_ARGS__` carries Sail's observation options. The fork's
  shipped-build and compiler-input requests (`SAIL_RUSTC_DEV_SOURCE_BASE`,
  `SAIL_VIR_SPAN_REMAP`, `SAIL_COMPILER_INPUTS*`) reach compilers while those
  features are active. `SAIL_PROVENANCE_DIR` and
  `SAIL_NATIVE_COMPILER_INPUTS`/`SAIL_NATIVE_COMPILER_INPUTS_BINDING` are also
  observable while active. Code that reads these settings can compile different
  inputs and differ from plain Cargo-Verus. Default managed invocation also
  clears caller values of the five existing interactive-engine controls;
  selected interactive mode supplies its compiler-child values. The
  [divergence registry](divergences.md#compiler-visible-integration-settings)
  defines the exact scope. Solver correctness, actual-query cache identity,
  freshness, complete scope and truthful reporting remain required; unrelated
  differences remain failures. No private transport is selected.

## Input coverage and cache integrity

- Read-back currentness brackets its reads with metadata stamps of every
  recorded path (#387). A concurrent edit that restores every stamped
  field, including the inode change time, is not detected; that needs a
  privileged clock change or a filesystem without change times.
- Native-artifact store seeding can hide a changed native linker at the same
  `PATH` ([#187](https://github.com/Chelis-Lang/sail/issues/187)). The store
  is disabled under [ADR 0053](decisions/0053-disable-unqualified-native-store.md),
  so main is not affected. Its family key now binds the bytes of the native
  tools (`cc`, `c++`, `ar`, `ld` on `PATH` and any linker or C compiler named
  by the environment) and refuses the store when a linker is chosen by Cargo
  configuration or compiler flags. Re-enabling the store is still not
  qualified.

The service-wide acceptance theorem is not complete
([#368](https://github.com/Chelis-Lang/sail/issues/368), #371–#376); see the
[refinement guide](design/acceptance-production-refinements.md).

## Performance

Current numbers are in [performance](performance.md).

- The original data-representation experiment in the
  [broader algorithm/corpus review](validation/general-performance.md)
  remains unselected. The separate runtime and installed-output corrections
  are qualified and consumed on main. The corrected proof-edit/failure/fix
  catalogue passes six rounds. The generated binaries' missing prelude is
  corrected (#631), but the valid parallel workspace exposes malformed
  combined verifier JSON (#632,
  [VI-0039](verus-issues/VI-0039-concurrent-cargo-json-framing.md)). The strict
  comparison remains failed; a serial control also refuses two shapes because
  independent crate document order changes. Complete compiler-invocation
  identity is not established by matching aggregate counts.
  Runtime/installed corrections and their both-platform gates are delivered;
  complete parallel scaling acceptance remains open under #632/#629. The
  unselected optimization experiments establish no general speedup.

- **A first unchanged call can still take the slower full path**
  ([#558](https://github.com/Chelis-Lang/sail/issues/558)) when post-return
  preparation has not completed. Preparation ships in 0.2.0; an immediate call
  does not wait for it. Historical macOS 0.1.0 measurements (`0.1.0-rc.7`)
  recorded Anvil 2.11 s
  against 0.30 s, Vest 0.80 s against 0.26 s, CapybaraKV 0.59 s against
  0.23 s and IronKV 0.29 s against 0.21 s. Later unchanged calls replay in
  0.04–0.09 s on macOS and 0.03–0.06 s on Linux
  ([ADR 0082](decisions/0082-unchanged-call-replay.md)).
- **Unchanged calls that do not replay pay setup cost**
  ([#560](https://github.com/Chelis-Lang/sail/issues/560)). When replay misses (for
  example after a `touch`, a recent ctime or different arguments), or for
  forced jobs, which never replay, unchanged Anvil takes 1.63 s against
  stock's 0.25 s on macOS 0.1.0, with no verifier run. Unchanged `SAIL3112` setups
  took about 0.9 s on Anvil in 0.1.0. The qualified native-Cargo candidate
  measures 0.312 s Linux / 0.407 s Mac while remaining Unknown and no-reuse
  ([#561](https://github.com/Chelis-Lang/sail/issues/561),
  [scope and remaining gates](validation/unsupported-cargo-fallback.md)).
- **Tiny-project edits cost more than stock**
  ([#560](https://github.com/Chelis-Lang/sail/issues/560)). Input checks, receipts and
  complete reports add a fraction of a second to edits of small projects;
  one-line edits of the measured real projects are faster than stock.
- **macOS cold Anvil is not faster than stock**
  ([#232](https://github.com/Chelis-Lang/sail/issues/232),
  [#466](https://github.com/Chelis-Lang/sail/pull/466)): on 0.1.0 its median
  is 0.9 s slower on 100 s (100.3 s against 99.4 s), with overlapping ranges.
  Linux cold medians are faster on all four measured rc.7 projects.
  The later complete 0.2.0 composition measures 98.94 s Sail / 96.87 s stock
  (+2.07 s), with disjoint ranges; the regression remains active
  ([record](validation/release-expansion.md)).
- **Vest reverts sometimes miss the restore fast path in 0.2.0 and earlier**
  ([#595](https://github.com/Chelis-Lang/sail/issues/595)). In two of six
  0.1.0 macOS rounds the Vest revert and fix revert took about 12 s instead of
  1.1 s, with every check served from the solver cache; a Linux baseline
  reproduced it. Cause: a call killed a background preparation that had
  already taken the compiler target's build-script records, so that call
  rebuilt every dependency, vstd included, without the producer certificates
  instant revert needs. Verdicts were never affected. Mostly fixed on `main`
  after 0.2.0: running preparation now finishes, for at most 60 seconds,
  before the call proceeds. On a loaded Linux host, Vest revert misses fell
  from 81 of 120 to 14 of 120 ([record](validation/instant-revert.md)). The
  remaining misses have the same effect from a different start: a preparation
  that ends without a Current result, for example because an edit landed
  while it ran, still does not republish the build-script records it took.
  Separately, with preparation off, a loaded cold build once left its own
  dependency artifacts without producer certificates (2 of 40 reverts). With
  0.2.0, `SAIL_BACKGROUND_PREPARE=0` avoids most of these misses.
- Release 0.1.0 can miss shipped vstd through a symlinked cache (#601).
  The candidate resolves managed paths physically and passes installed-release
  first-Serve checks on both platforms ([record](validation/cache-path-selection.md)).
  Use a physical cache path with 0.1.0.
- **A miss re-solves the whole bucket**
  ([#158](https://github.com/Chelis-Lang/sail/issues/158)). Editing one
  function re-sends its module's whole solver session; on Anvil one bucket
  costs about 66 s.
- **The shipped vstd proof is not used with `rustc-dev`**
  ([#264](https://github.com/Chelis-Lang/sail/issues/264)). Its libraries are
  loaded by rustc, so the proof does not match; `SAIL3138` names them.
- **macOS resource-limit subsumption is unavailable.** Resource counts vary
  for one captured vstd query while inputs and answers agree. Releases ship
  exact default and limit-100 profiles; other limits verify vstd normally
  ([reproduction](../corpus/repros/z3-resource-count-variation/README.md)).
- **An engine verify after a repair re-verifies the selected crate.** A
  repair rebuilds that crate in the shared engine target, so the next
  `sail verify --engine interactive` with unchanged source verifies it again.
  The cause of the byte difference is not investigated.
- **Default and engine runs each verify vstd once in their own target.**
  Whether they can share a target is not measured.

## Product limits

- **A project toolchain installed on first use can fail verification.** On
  a fresh GitHub-hosted Ubuntu 24.04 runner, `sail verify` in a project whose
  `rust-toolchain.toml` named an uninstalled toolchain (1.99.0 with
  `clippy`) exited 2 with `SAIL3104`, quoting rustup's
  `component download failed … could not rename 'downloaded' file`, while
  rustup installed that toolchain on first use (Sail 0.2.0 archive with this
  branch's `sail`, job log sha256
  `cd70f95917cd44affa779d24a9f3ad46b81ba5d91b16fbca94c99a6b9c21b9f2`). The
  cause is not established. Installing the toolchain first
  (`rustup toolchain install` in the project) avoids it; the
  [GitHub Action](github-action.md) does that.

- **Setups verified without proof reuse** (`SAIL3112`). These verify exactly
  as plain `cargo verus verify` does, with solver caching, but record and
  reuse no proof receipts
  ([workspace verification](workspace-verification.md#setups-verified-without-proof-reuse)):
  - a Cargo outside the qualified 1.93.0 to 1.99.1, for example 1.92 or a
    newer release from a project's `rust-toolchain.toml`, `RUSTUP_TOOLCHAIN`
    or `CARGO` (`SAIL3139`, [ADR 0084](decisions/0084-qualified-cargo-range.md));
  - tests, benches and examples; `--target-dir`; Verus's `--log` options;
    `--no-verify`; Cargo `-v` and `--message-format`;
  - `--sysroot`, `RUSTC_WORKSPACE_WRAPPER`, Cargo `--config` and
    configuration includes, and compiler or sysroot overrides Sail cannot
    identify;
  - loader variables such as `LD_LIBRARY_PATH` or `DYLD_LIBRARY_PATH`, which
    verify in the project's own Cargo target: the first such run in a
    project verified only with `sail verify` builds that target as a cold
    `cargo verus verify` does
    ([#577](https://github.com/Chelis-Lang/sail/issues/577));
  - a Sail cache and package root that overlap.
- **Only Sail's Verus,** at the release it pins
  ([ADR 0043](decisions/0043-default-mode-runs-the-fork.md)). A project that
  needs another Verus release cannot use Sail yet. `SAIL_CARGO_VERUS` runs
  another `cargo-verus` only as a labelled development override
  (`warning[SAIL0108]`).
- **Published 0.2.0 supports Cargo and standalone files.** Standalone
  verification through CLI, watch and MCP is delivered on native Linux/macOS.
  Standalone watch requires source and discovered inputs beneath its working
  directory; CLI/MCP support external inputs.
  See [standalone verification](standalone-verification.md) and
  [#164](https://github.com/Chelis-Lang/sail/issues/164).
- **Instant revert applies to qualified dependency profiles only.** Retention
  keeps up to 16 revisions per target (1 GiB), with the first verified
  revision pinned. It is skipped for unqualified profiles and for targets too
  cheap to be worth retaining; `SAIL3146` says why. A first run without a
  retention plan (for example a forced run) gets no instant revert until the
  dependencies are rebuilt
  ([#285](https://github.com/Chelis-Lang/sail/issues/285)). See
  [instant revert](instant-revert.md).
- **Current is about inputs, not the verdict.** In doubt a result reads back
  Unknown, never Current. A few rare changes during a run are not detected;
  see [freshness](freshness.md) and [trust](trust.md). On macOS, a Cargo
  configuration file created and deleted again within one run is not
  detected (ADR 0041 note of 2026-10-14).
- **A focused check is not acceptance.** A successful focused repair check
  still needs a complete Current full-target verification.
- **Repair selectors have a bounded source scope.** An arbitrary external
  include may need a full run in a new selector context before Current
  evidence is available; changed external bytes still force verification.
- **Managed cache budgets are opt-in soft caps.** Busy workspaces and the
  newest workspace stay protected, so configured usage can exceed the cap.
  Manual pruning and idle-daemon budget maintenance are available in 0.2.0
  ([disk use and cleanup](workspace-hygiene.md#managed-compiler-targets)).
- **No `singular` feature.** Like the stock release, Sail's Verus has no
  `singular`, so `by(integer_ring)` proofs fail as they do in stock. The
  repair engine falls back to ordinary verification (`SAIL3124`) and cannot
  accept a repair for such targets
  ([details](proof-repair.md#verus-features-the-engine-lacks)).
- **Combined output order.** The managed relay guarantees final stdout
  completion, not one global order across stdout and stderr
  ([#215](https://github.com/Chelis-Lang/sail/issues/215)).
- **A failing build script leaves no diagnostic in the result.** When a
  requested package's build script fails, the `sail verify --output json`
  document has exit code 101 and neither a target nor a diagnostic; Cargo's
  message is on stderr only. `sail report` then says that verification failed
  and that the result holds no error diagnostic. Observed on Linux with the pinned Verus at `b059ce3f`;
  other failures before compilation were not tried.

## Integration APIs (experimental)

- Positive native mutation kills are not established: the strong
  boolean-contract literal mutants return solver Unknown, so their outcome is
  inconclusive ([record](validation/neutral-mutation.md)).
- A failed Verus function can retain an unknown primary obligation; the
  neutral contract projection returns `NotProved` and keeps the unknown count
  and reason ([reproduction](../corpus/repros/neutral-contract-unknown/README.md)).

## Tests and environment

- **Short fixture deadlines under heavy parallel load on macOS.** Exec of a
  freshly written executable has taken up to 10.5 s during
  `cargo nextest run --workspace -j 32` on an 8-core Mac. Fixtures whose
  1–10 s bounds include such a start fail there and pass alone
  ([#280](https://github.com/Chelis-Lang/sail/issues/280)); the rule for
  fixture waits is in [testing](testing.md).
- **Unclassified missing-file error in macOS watch**
  ([#306](https://github.com/Chelis-Lang/sail/issues/306)). Seen once in a
  host gate; not reproduced, cause unknown.
- **Shared HTTP deadline fixtures.** The 250 ms lookup and coalescing
  fixtures have failed intermittently; cause unknown.
- **Verifier wait under amd64 emulation.** Verification sometimes stalls
  under emulation, which is not used for performance or Linux validation
  ([reproduction](../corpus/repros/linux-emulation-verus-stall/)).

## Verus and Z3

[The Verus tracker](verus-issues/README.md) lists every locally recorded
upstream issue, with its reproduction and workaround. Sail preserves raw
diagnostics and refuses uncertain attribution or coverage. Z3 4.16.0 can vary
nonlinear rlimit consumption on identical input; see the
[reproduction](../corpus/repros/z3-nla-rlimit-nondeterminism/README.md) and the
[determinism policy](decisions/0009-solver-nondeterminism-and-unstable-results.md).
Seed-dependent proof discovery is available only through the experimental
portfolio, with separate keys. Stock Verus prints JSON `func-details` in hash
order ([VI-0008](verus-issues/VI-0008-func-details-hash-order.md)), so the
deep stock control can report unseen orders as Different; the fork's own
deep gate compares exactly.

## Resolved by decision

- **Error order across modules.** A cache-warm run can print a unit's errors
  in another module order
  ([VI-0007](verus-issues/VI-0007-nondeterministic-error-order.md)).
  Conformance compares messages as a multiset
  ([ADR 0042](decisions/0042-diagnostics-compared-as-a-set.md)).
- **No Singular engine variant.** A target that needs a Verus feature the
  engine lacks falls back to stock verification (`SAIL3124`), and the engine
  arm counts it as `engine-lacks-feature`
  ([reproduction](../corpus/repros/engine-without-singular/)).

The unqualified-Cargo early-refusal optimization ships in 0.2.0. It remains
limited to installed native Rustup Cargo programs/proxies; custom wrappers and
external path toolchains keep late no-reuse fallback. Both-platform component
costs and composed release controls pass; this is not full-path parity
([qualification](validation/unsupported-cargo-fallback.md)).

The native macOS memory-observer and pinned-stock source-suite corrections are
qualified and included in 0.2.0. Earlier macOS27 failures and strict stock debug
order failures (#627) remain historical evidence, not unmerged release blockers.
The independently qualified observer and ADR 0093 retain their exact scopes
([observer](validation/benchmark-memory.md),
[source suites](validation/general-performance.md),
[stable release](validation/stable-candidate.md)).

## Settled digest candidates

The ADR 0091 source switch remains off by default. Linux uses the guarded
target path by default, with explicit `=0` opt-out;
macOS keeps fresh target reads by default. Corrected Linux source pairs
regress warm Anvil by 120 ms; the refined Mac source pairs save about 48 ms
with only a bounded startup quiet check. The corrected target memo saves
about 430 ms on Linux but refuses publication on Mac
because build-script directory full stamps change during unchanged calls.
The two-second guard is retained. None of these component results establishes
full-path parity or release acceptance
([qualification](validation/settled-digest-observations.md)).
