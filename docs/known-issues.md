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
  differences remain failures. No private transport is selected.

## Input coverage and cache integrity

- The experimental managed Rust API can reuse an unchanged-call record when
  its explicitly supplied socket changes from one path to another
  ([#571](https://github.com/Chelis-Lang/sail/issues/571)); the key binds only
  whether a socket exists. The ordinary CLI derives that setting from already
  keyed caller/cache inputs; no ordinary CLI false-proof reproduction is claimed.
  The separate exact-path key repair remains open.
- `cargo xtask fetch-verus` can install in the default cache while validation
  follows a custom `SAIL_CACHE_DIR` ([#581](https://github.com/Chelis-Lang/sail/issues/581)).
  Use `SAIL_VERUS_HOME` pointing to an existing primary release's install parent
  for that scoped setup; the separate tooling fix remains open.
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

- **Unchanged calls that miss replay pay setup cost**
  ([#314](https://github.com/Chelis-Lang/sail/issues/314)). An unchanged
  `sail verify` replays in about 26 ms on Linux (60 ms on macOS) when every
  recorded stamp matches
  ([ADR 0082](decisions/0082-unchanged-call-replay.md)). When replay does not
  apply (for example after a `touch`, a recent ctime or a different
  argument), unchanged Anvil takes about 1.7 s against stock's 0.3 s, with no
  verifier run.
- **Tiny edits cost more than stock**
  ([#156](https://github.com/Chelis-Lang/sail/issues/156)). Input checks,
  receipts and complete reports add a fraction of a second to small edits.
- **macOS cold Anvil is slightly slower than stock**
  ([#278](https://github.com/Chelis-Lang/sail/issues/278),
  [#232](https://github.com/Chelis-Lang/sail/issues/232)): about 0.1 s on
  100 s, within run-to-run range. Linux cold runs are at parity or faster.
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
