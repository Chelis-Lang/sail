# Known issues and limits

Open Sail defects and current product limits on main. Fixed defects are
removed from this page; their issues and the [changelog](../CHANGELOG.md)
keep the history. The [roadmap](ROADMAP.md) tracks release work.

## Accepted integration limit

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
  differences remain failures. No private transport or new runtime is selected.

## Input coverage and cache integrity

- **Unchanged-call replay can return an old report after a compiler-tracked
  ghost input changes** ([#552](https://github.com/Chelis-Lang/sail/issues/552)).
  A caller environment variable read only in ghost code is missing from
  Cargo's dep-info and from the replay record, so changing only that variable
  replays the earlier verified report, where stock Verus fails. This is
  reproduced end to end on main `d5a5d58b`. Until
  [PR #554](https://github.com/Chelis-Lang/sail/pull/554) merges, set
  `SAIL_MANAGED_NOOP=0` to disable replay.
- **Proof reuse loses effective compiler-environment bindings**
  ([#551](https://github.com/Chelis-Lang/sail/issues/551)). Ordinary receipt
  reuse and retained-proof admission both normalize the effective
  ` __VERUS_DRIVER_ARGS__` to its base value, so a change in Sail's added
  observation options (for example turning attribution on) is not bound.
  Ordinary receipts also miss `SAIL_PROVENANCE_DIR` going from absent to
  present and ghost-only reads of `SAIL_COMPILER_INPUTS` and
  `SAIL_COMPILER_INPUTS_BINDING`, because freshness resamples the outer
  environment rather than the compiler child's values. This is a
  source-established gap, separate from ADR 0088's accepted visibility
  difference; no end-to-end wrong verdict is reproduced.
- **A forced run of a non-default profile fails instead of reverifying**
  ([#553](https://github.com/Chelis-Lang/sail/issues/553)). Managed package
  cleanup omits Cargo's `--release`, `--profile` and `--target` selectors.
  After a verified `--release` run, `SAIL_MANAGED_FORCE=1 sail verify
  --release` finds the release artifact fresh and ends Unknown with
  `SAIL3100`/`SAIL3107` (reproduced for `--release`; the other selectors are
  source findings). No false proof results.
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

- **The first unchanged call after a verification is slower than stock**
  ([#558](https://github.com/Chelis-Lang/sail/issues/558)). It takes the full path and records
  the replay: on macOS main `dca5cd43`, Anvil 2.07 s against 0.30 s, Vest
  0.79 s against 0.26 s, CapybaraKV 0.59 s against 0.22 s and IronKV 0.28 s
  against 0.21 s. Later unchanged calls replay in 0.03–0.09 s
  ([ADR 0082](decisions/0082-unchanged-call-replay.md)).
- **Unchanged calls that do not replay pay setup cost**
  ([#560](https://github.com/Chelis-Lang/sail/issues/560),
  [#559](https://github.com/Chelis-Lang/sail/issues/559)). When replay misses (for
  example after a `touch`, a recent ctime or different arguments), or for MCP
  and job calls, which never replay, unchanged Anvil takes about 1.6 s against
  stock's 0.24 s on macOS, with no verifier run. Unchanged `SAIL3112` setups
  take about 0.9 s on Anvil ([#561](https://github.com/Chelis-Lang/sail/issues/561)).
- **Tiny-project edits cost more than stock**
  ([#560](https://github.com/Chelis-Lang/sail/issues/560)). Input checks, receipts and
  complete reports add a fraction of a second to edits of small projects;
  one-line edits of the measured real projects are faster than stock.
- **macOS cold Anvil is slower than stock**
  ([#232](https://github.com/Chelis-Lang/sail/issues/232),
  [#466](https://github.com/Chelis-Lang/sail/pull/466)): 0.7 s on 100 s
  (100.2 s against 99.5 s), with ranges that do not overlap. Linux cold runs
  are at parity or faster.
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
  - loader variables such as `LD_LIBRARY_PATH` or `DYLD_LIBRARY_PATH`;
  - a Sail cache and package root that overlap.
- **Only Sail's Verus,** at the release it pins
  ([ADR 0043](decisions/0043-default-mode-runs-the-fork.md)). A project that
  needs another Verus release cannot use Sail yet. `SAIL_CARGO_VERUS` runs
  another `cargo-verus` only as a labelled development override
  (`warning[SAIL0108]`).
- **No single-file projects.** `verus file.rs` projects are not supported
  ([#164](https://github.com/Chelis-Lang/sail/issues/164)).
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
- **Managed compiler targets are removed only by `sail cache projects prune`.**
  There is no quota or automatic collection
  ([disk use and cleanup](workspace-hygiene.md#managed-compiler-targets)).
- **No `singular` feature.** Like the stock release, Sail's Verus has no
  `singular`, so `by(integer_ring)` proofs fail as they do in stock. The
  repair engine falls back to ordinary verification (`SAIL3124`) and cannot
  accept a repair for such targets
  ([details](proof-repair.md#verus-features-the-engine-lacks)).
- **Combined output order.** The managed relay guarantees final stdout
  completion, not one global order across stdout and stderr
  ([#215](https://github.com/Chelis-Lang/sail/issues/215)).
- **Summary wording.** A failing managed job summary leads with `0 verified`
  before `… passed by Verus's totals`, and after a Rust compile error reads
  `0 verified, 0 failed, 0 unknown`.

## Integration APIs (experimental)

- Positive native mutation kills are not established: the strong
  boolean-contract literal mutants return solver Unknown, so their outcome is
  inconclusive ([record](validation/neutral-mutation.md)).
- A failed Verus function can retain an unknown primary obligation; the
  neutral contract projection returns `NotProved` and keeps the unknown count
  and reason ([reproduction](../corpus/repros/neutral-contract-unknown/README.md)).
- Lowering version two preserves compound-field definedness; new workers
  require it and retained reads refuse affected version-one projections. The
  source-job hold stays until the correction is qualified
  ([#323](https://github.com/Chelis-Lang/sail/issues/323)).

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
