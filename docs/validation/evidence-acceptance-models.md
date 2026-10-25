# Evidence-acceptance model qualification

Status: expanded component and composed model proofs pass direct stock
checking on macOS arm64 and native Linux x86-64 at the frozen commits below.
Changed models and [production refinements](acceptance-production-refinements.md)
have separate [current native qualification](acceptance-production-refinements.md);
hosted checks and the service-wide theorem remain open. This record owns the actual scope of
[checking](../design/acceptance-proof-checker.md).

## Expanded model qualification

The captured-source manifest contains four roots and one shared context.
The individual drivers verify 59 query, 64 input and 62 authority checks; the
combined driver verifies 196 checks across all sources. Counts overlap because
the combined driver rechecks the actual components. All four positive
runs use pinned stock Verus, full-crate scope, explicit pinned
Z3 and `--no-cheating`, with zero errors.

The selected 24 guard mutations cover pairing/suppression, noisy publication,
corrupt/unrecorded recovery, source alias/scope bounds, capture epochs/domain,
solver projection/read freshness, whole/confirmed/complete coverage, stale or
misattributed records, omitted started child streams, and composed admission,
mode and fresh-projection guards. All 28 selected cases pass at frozen source
`7d1c5c9456a96a180b3f691bfc449d4e6b7cc7e8`: four positive roots with zero errors,
and 24 controls with ordinary verification failures, complete inventories and
normal termination. Resource exhaustion and timeouts never qualify controls.

| Platform | Environment | Cases | Checker wall time | Result |
|---|---|---:|---:|---|
| macOS arm64 | Native host, pinned stock archive/runtime | 28 | 131.21 s | Pass |
| Linux x86-64 | EC2 `c7i.4xlarge`, `us-east-1`, cached core CI image | 28 | 277.65 s | Pass |

The Linux `ci/local.sh proofs` wrapper takes 278.10 s, including launch and
artifact handling. The slowest verifier case takes 25.10 s. This
qualifies native packaging and the selected inventory within the fast job's
10-minute limit; hosted runner qualification is a separate PR check. The
checker uses the unchanged 120-second per-case timeout and normal solver
resource limit, with no narrowing flags.

Raw receipt SHA-256 values, outside the repository:

- macOS: `faf5c00f1ce735b7bd279a357ff73190a2ee013d6885f836a7e04a2bf70fda1e`.
- Linux: `3cdd0b2f2f365215396b99f352296aa55f3ca160ad0ac06345fa6e5643a988db`.

Both receipts contain the same eight source/manifest/checker/pin digests, all
actual driver/dependency/candidate hashes, full result documents and validated
platform runtime identities. The expanded checker has 12 passing failure tests.
The production runtime, corpus, comparator and compiler/solver inputs are
unchanged; these proofs claim no new corpus conformance or performance gain.

Frozen acceptance attempts:

| Date | Source | Gate | Result |
|---|---|---|---|
| 2026-10-24 | `4588f22f` | macOS complete selected-case checker | Refused pair-shift mutation because the lifecycle witness hit the normal solver resource limit; no pass claimed |
| 2026-10-24 | `4588f22f` | Native Linux complete selected-case checker | Refused pair-shift mutation after the 120-second per-case timeout; no pass claimed |
| 2026-10-24 | `7d1c5c94` | macOS host gate with owner configuration | Two tests refused the global `kache` compiler-wrapper setting; rerun uses an isolated Cargo configuration without altering that setting |

Exploration index: 2026-10-24, query solver-binding guard-removal experiments
were rejected after resource-limit diagnostics; neither experiment is in the
qualified mutation inventory. Ordinary development witness failures were
corrected without relaxing the normal solver resource limit. They do not
qualify any proof or production guarantee.

## Initial model checkpoint

The initial models passed on both platforms before the lifecycle expansion.
The following source and hashes preserve that narrower scope.

## Source, runtime and scope

The checked checkpoint is `8b0ae88540f8fb52a9ac2e443a94de124baa5b90`.
It contains the three component roots, complete function/count inventory,
six deliberate mutations and the reviewed independent Python checker. The
checker reads the selected commit's Git blobs and compiles those captured
bytes, records mutated-candidate digests, and checks normal termination and
full-crate scope. Its own implementation remains ordinary reviewed/tested
code; it is not formally verified.

The upstream stock pin is Verus `0.2026.10.18.7d82994`, commit
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust `1.99.1` and pinned
Z3 `4.16.0`. Each platform's archive and every extracted runtime file are
validated against `corpus/toolchain.toml`. The direct command always uses
`--no-cheating --output-json --num-threads 1`, an explicit pinned Z3 and a
sanitized environment. Neither Sail, its fork nor its caches establish these
proofs.

| Platform | Environment | Positive roots | Deliberately broken controls | Result |
|---|---|---:|---:|---|
| macOS arm64 | Local native host, pinned stock archive/runtime | 3 | 6 | All nine checks pass |
| Linux x86-64 | Native EC2 `c7i.4xlarge`, `us-east-1`, CI core image | 3 | 6 | All nine checks pass |

On each platform, positive verification counts are query association **24**,
input binding **22**, scope/result authority **28**: **74 total**, zero errors,
normal exit zero and whole-crate scope. Each negative control preserves the
entire function inventory and fails with one ordinary verification error,
normal exit one and whole-crate scope. No syntax/VIR error, signal, timeout or
missing document substitutes for the expected failure.

Raw receipts remain outside the repository:

- macOS receipt SHA-256: `81b9dc24f47aadc082cd48ab43939d16554151e753de15f5c145cff2dfa5c412`.
- Linux receipt SHA-256: `fda1a6921ce16aeaed65ad9ab50d9ea91f77bc6a45bf674777e159f4a07186d8`.

Each receipt includes the commit, exact source/runtime hashes, toolchain,
invocations and individual result documents. The nine checker failure tests
also pass locally. They exercise malformed evidence, omitted source/function
inventory, swapped runtime files, source-loading constructs and inherited
argument/loader settings; fixture tests alone do not qualify a real proof run.

## Remaining gates and limits

- The combined workspace gate and fresh hosted checks remain separate
  publication requirements. Existing runtime code, compiler/solver inputs and
  corpus selection are unchanged by this model slice; no new conformance or
  performance result is claimed.
- The query model still needs production parser/session/writer/recovery
  refinement and an admitted initial-store/writer contract.
- Input equality does not prove complete compiler-input observation or the
  Linux/macOS filesystem/process adapters. Digests are not assumed injective.
- This historical model checkpoint does not qualify the later source-shared
  production predicates or native report/receipt/job/MCP callers. Their owning
  record identifies implemented slices and remaining refinement.
- Runtime/checker/toolchain immutability is a trust assumption; pre/post hashes
  cannot establish atomic launch binding against transient replacement.
- At the initial `8b0ae885` checkpoint, actual proof invocations were local-only.
  The expanded implementation adds the direct proof CI job; its qualification
  is recorded above. The ordinary workspace gate includes the failure tests.

No complete gate attempt at the initial `8b0ae885` checkpoint failed or was
interrupted. The expanded-model failed attempts are indexed above.
Intentional mutation failures are successful negative controls, not successful
verification of the mutated models.
