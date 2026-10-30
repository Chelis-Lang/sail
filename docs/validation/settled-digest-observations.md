# Settled digest observations (#560)

Status: independently controlled source and owned-target candidates under
owner-approved ADR 0091. Native component gates pass. Linux target pairs save
material wall time; Mac target publication refuses. Both remain off by default.
Final combined gates and release qualification remain pending.

`SAIL_MANAGED_SOURCE_DIGEST_MEMO=1` enables bounded local source observations;
the default retains fresh content reads. The fresh namespace and alias map
select the ledger; each original regular-file digest requires its exact full
stat stamp, a successful complete Current origin, and ctime already two
seconds older than that original run's start. Changed entries read afresh.
A namespace or binding change, malformed data, missing ledger or unsupported
platform keeps fresh reads. An unsettled original never ages into trust.

Original before/after capture still encloses every content read and memo
lookup. Only successful complete Current runs seed their original readback;
forced calls neither consult nor seed it. Retries discard the original
observation, read contents afresh and seed nothing. Each invocation slot
holds one bounded source ledger (100,000 files and 8 MiB). Existing slot
retention and pruning own its lifetime. It is never imported from a shared
store. Full stamps are retained from the original capture, not reconstructed
from a later stat paired with old bytes.

The ledger is scheduling state for full calls. No-op replay independently
binds the source vector and proof state, so it does not bind the ledger body;
creating/deleting that entry still changes the slot listing. This preserves
records across full calls under other environment keys. Every old Currentness,
dirty-clean, output and replay/recording check remains. No verifying-run
transcript or background recorder is created.

Unit controls cover settled exact digests, same-size edits with restored
mtime, changed bindings/namespaces, damaged/oversized ledgers, unknown or
unsorted entries, non-Current origins, future starts, symlinked ledgers and
non-promotion of an originally unsettled read. These controls pass in native gates below; their scope does not establish
release acceptance.

Indexed attempts: `6e243870`, 2026-10-30, native compilation refuses an
out-of-scope retry variable; `5d239054`, native Clippy refuses three style
errors. Retry snapshots now explicitly disable memo reads; method selection
and control flow are corrected without changing a check or allowing a lint.

## Owned-target candidate

`SAIL_MANAGED_TARGET_DIGEST_MEMO=1` independently enables this candidate.
It freshly enumerates the entire physical owned target and checks every
entry's full stamp, root and ancestor physical identity, complete source
observation, metadata/selection, context/compiler family and all exact receipt
bytes. Any alias, changed or unknown entry, missing field or changed binding
keeps complete reads. Only successful complete Current calls without retry
or force may publish the original digest-phase stamps, already settled before
the original call started. Unknown/external paths always read afresh. Original
stamps must still match after the phase and at publication. Dirty package
cleanup and all receipt admission remain unchanged. The local ledger has the
same 8 MiB/100,000-entry limits and is never shared.

The prototype saves post-invocation artifact/dependency reads and seeds pure
receipt projections under their exact existing selection/target keys before
ordinary dirty-package decisions. Every projection batch still validates its
original full source observation. Retries do neither. It needs its own complete active controls,
native before/after measurements and material gain before either platform
selects it. The independently measured target result below does not grant release
acceptance.

Indexed attempt, `ee105bbe`, 2026-10-30, active source-memo full E2E stops
at the generic compiler-environment witness's clean-caller tally. Its actual
stock and Sail streams agree on `1 verified, 2 errors`, exit101: the caller
explicitly supplied the opt-in variable tested by the macro. The original
`0 verified, 3 errors` assertion is retained under its clean-caller precondition;
a separate opt-in caller case requires the original strict stream/exit
comparison and the observed `1 verified, 2 errors` tally. Remaining E2E
controls still run with the optimization active. No oracle is weakened.

## Initial component results and publication refinement

The initial prototype (`ee105bbe` production Rust), ten rotated warm Anvil
pairs on c7i.8xlarge/us-east-1, is slower: fresh median 1.586891 s versus
memo 1.757142 s. All warm/timed calls are Current with zero verifier starts,
identical proof context and input revision. This does not qualify Linux
adoption; default fresh reads remain selected. The first setup attempt stops
before timing because the cold and first warm input revisions differ; the
retained rerun requires exact identity throughout warm-up and every timed pair.

The Mac's first setup omits the documented `OPENSSL_DIR`; its cold call is
Unknown and refuses timing. The rerun sets the original native-header hint.
Both failed setup logs remain retained; neither is counted as acceptance.

The publication refinement keeps a successfully admitted original Current
ledger when its complete path/stamp/digest vector is still identical. It
retains that original run and start rather than serializing, checksumming
and replacing the same content under a newer provenance on every full call.
Changed entries still publish only after genuine new bracketed reads; an
originally unsettled observation cannot age into trust. Every ledger read
still performs complete bounded decoding, namespace, origin, checksum and
stamp validation. The added control requires unchanged bytes and full inode
stamp, original run/start retention, and fresh publication after a same-byte
restored-mtime rewrite. Refined native gates and Mac timing pass below; no refined Linux source
speedup is established.

Indexed source-review finding, `7a750319`: CLI force already nulls the ledger
path, but the environment force control was checked later than speculative
source observation. The source switch now refuses environment force before
any snapshot; subprocess controls retain enabled/zero-force behavior and
require force1 refusal without unsafe environment mutation. This narrows the
candidate and changes no key or admitted proof. Fresh native gates are pending.

## Executed independent qualification

All builds use Sail Rust 1.99.0, Verus Rust 1.99.1, fork `47fb3be6` and
stock oracle `0.2026.10.18.7d82994`. Linux is native c7i.8xlarge/us-east-1;
Mac is mac-m4.metal/us-east-1b/macOS 27.0.

| Candidate | Native scope | Result |
|---|---|---|
| Refined source `7a750319` | Complete image gate, pinned solver, source-enabled full E2E plus extra, full fast/standard | 1,268 ordinary tests and 51 solver tests pass; every E2E phase passes; all four fast arms 147/147 Same (3,451 functions), standard 159/159 Same (13,166 functions); zero unstable/different/infrastructure errors or skipped projects, 11/17 resource-drift units separately |
| Same refined source | Complete Mac host gate | Clippy and 1,257 ordinary tests pass, 54 skipped |
| Target `d6786f96` | Complete image gate, pinned solver, target-enabled full E2E plus extra, full fast/standard and all 21 native mutants | 1,271 ordinary tests, 51 solver tests and all E2E phases pass; all four fast/standard arms agree with the same counts and zero disagreements/errors/skips above; every mutant is detected |
| Same target | Complete Mac host gate and actual factory | Clippy and 1,260 ordinary tests pass, 54 skipped; four exact schema-13 units built |

Ten rotated on/off warm Anvil pairs retain identical proof context and input
revision, Current results, and zero verifier starts. Replay is disabled; no
trace or memory sampler runs in the command interval. Each comparison enables
only the optimization named in its row.

| Platform / implementation | Fresh median, s | Memo median, s | Consequence |
|---|---:|---:|---|
| Linux initial source `ee105bbe` | 1.586891 | 1.757142 | Regression; retain fresh reads |
| Quiet Mac refined source `7a750319` | 1.375511 | 1.327397 | About 48 ms saved; still far above stock, no release parity claim |
| Linux target `d6786f96` | 1.593258 | 1.151233 | About 442 ms saved; still far above stock |
| Mac target `d6786f96` | — | — | Publication refuses before timing; no admitted target ledger |

The Mac target refusal is an observed full-stamp mismatch, not an entry/byte
ceiling or alias refusal: 2,777 entries, no symlinks, estimated serialized tree
about 1.25 MiB. Build-script directories including anyhow, libc and serde_json
receive ctime changes about 1.18 s after the unchanged call starts. Their
complete original stamps cannot meet ADR 0091's pre-start two-second rule.
The guard remains intact. No claim is made about which Cargo operation causes
those writes. Mac target reads remain fresh. This failed component attempt is
retained and supplies no timing sample.

Indexed attempts: refined Mac setup first refuses timing on a cold/first-warm
revision difference; the rerun establishes identity at the first warm call and
requires it throughout all warm/timed calls. Combined force control `b040e113`
then fails native Clippy because its subprocess-only test returns an unnecessary
Result; `76b159d6` keeps every assertion and uses a unit return. Fresh combined
ordinary/solver gates pass; full joined qualification is recorded when complete.

| Raw evidence retained outside Git | SHA-256 |
|---|---|
| Refined source Linux full gates | `8ab9ae674857b5588dc71432d350a1a4e58232b0eb7926cc0a73e547d56cd35e` |
| Refined source Mac host/factory | `9d55a170f8dddb3db7d6ff036f450f480f64a076af25fa08921c5f99661fe628` |
| Refined source quiet Mac pairs | `7c7d9ee5d643cfd167459d33d055360d6bf9915c2a5915542197b1152c2f38ca` |
| Refined Mac raw archive | `3cc27c1f93262fec2df3bb7e3d212c6a8b65b54a22ccea3fa052eabee4f9b8bd` |
| Target Linux full gates | `3c26834ccd80abb93d8034b4ac2329f1614d1420b7c78308ae255ba96064de35` |
| Target Linux ten pairs | `07bbf4069f4f8d6905f7dd069992012704a7df1b99dfa25e9439790b934ce1d5` |
| Target Mac host/factory and publication refusal | `db5778459164fa48f69f882fefd01198896b2d08e205e97c18b734a91c66e80d` |

Indexed static review, `d6786f96`: the target binding contains fresh source
contents and aliases but omits the original source stat stamp and observed
path set. The corrected candidate binds both before target admission. A new
production-caller lifecycle control establishes a settled target, requires a
real ledger hit, then requires refusal for a changed path set and a same-byte
restored-mtime source rewrite. Previous component timings and gates describe
the earlier candidate; corrected native qualification remains pending.

Indexed attempt, `3b37f266`, 2026-10-30: native Clippy compilation rejects an
unqualified Duration name in the new regression test. The fixture now uses the
fully qualified standard-library type; all lifecycle assertions remain.
