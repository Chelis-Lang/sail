# Exact Vest shipped-vstd profiles (#563)

Status: implementation candidate under owner-approved ADR 0090. Native Linux
and Mac Rust gates, both actual factories and real pinned Vest first-Serve
pass. Three native Linux cold pairs improve; Mac pairs collected before
quiet-host correction remain diagnostic. Final runtime/release qualification
remains pending. No release acceptance is claimed.

The candidate uses the exact registry Vest closure and features already
captured in `retained-public-vest-inputs.json`, converted reproducibly by
`scripts/source-policies.py`. Default and Vest policies remain disjoint.
Neutral source admission keeps its existing default policy. No consumer
manifest, lock, feature, argument or tool selection is edited.

Schema 13 binds each unit to its own source policy and establishing run in
both unit data and the release manifest; the release index binds their bytes.
The eight-unit ceiling leaves the 16,384-member ceiling unchanged. The
factory's explicit list adds Vest default and limit-100 recordings to the
existing default and raised-limit profiles. Old schemas and unknown,
inconsistent or damaged policy/unit/index relations refuse serving.

The Python factory controls pass with compiler/runtime doubles, retaining
all existing source, run, archive, cancellation and corruption checks. They
are fixture evidence only. The default fourteen native command shapes remain
unchanged; three additional Vest-version shapes are independently listed.
The actual native factories below establish their producer controls. Native
compilation, checksum and provenance admission remain mandatory.

Indexed attempt: `80f64058`, 2026-10-30, native Clippy passes; ordinary
tests retain two invalid positive fixtures. The old release fixture still
expects rejection of a third unit and omits the new per-unit profile map;
ADR 0090 approves eight, and the fixture now retains every existing binding
corruption assertion while requiring eight admissions and ninth refusal.
The compiler-read fixture uses an invented policy digest; it now uses the
actual default policy whose source reads it tests. No assertion is removed.

## Executed native qualification

Source `3250ef65`, Rust 1.99.0, Verus toolchain 1.99.1, fork `47fb3be6`,
stock oracle `0.2026.10.18.7d82994`:

| Environment | Scope | Result |
|---|---|---|
| c7i.8xlarge, us-east-1 | Complete image gate and pinned solver | 1,265 ordinary tests pass, 54 skipped; all 51 solver tests pass |
| Same Linux host | PR conformance | All four arms: 33/33 Same, 311 functions; zero unstable/different/infrastructure errors or skipped projects; five resource-drift units separately |
| mac-m4.metal, us-east-1b, macOS 27.0 | Complete host gate | Clippy and all 1,254 ordinary tests pass, 54 skipped |
| Both hosts | Actual release factory and release index | Schema 13: three Linux units and four Mac units; exact default and Vest source-policy/run bindings |
| Both hosts | First cold call on the real pinned registry Vest project at rlimit100 | Successful Current diagnostic call; shipped proof reports `served` |

Linux's default profile carries its qualified rlimit witness; Mac needs its
separate default raised-limit unit. Both add distinct Vest default and
limit-100 units. The source policy digests are
`77cfe09e6aec9d1934662198c343800bcde4c53b6cda2ed1011fe43f180fc12a`
(default) and
`b2ef67ab61fa578d8666a697f12a53ceb301da306923ab2ceaa86b9b617ff46f`
(Vest). Each unit retains its actual establishing run, never a shared invented
producer. Consumer manifests, exact dependency versions, feature selections
and verifier arguments remain unchanged.

First-Serve calls include diagnostics and retain one sample per platform.
Their 47.83 s Linux / 30.8 s Mac wall observations are not paired speedup
measurements. The identical shipped-profile implementation also passes full active-source
E2E plus extra and all four full fast/standard arms in combined source
`7a750319`: 147/147 Same (3,451 functions) and 159/159 Same (13,166 functions),
zero unstable/different/infrastructure errors or skipped projects; 11/17
resource-drift units separately. These are combined-build results, not a
separate execution of the restacked branch. Deep/deep-rvt and the final
six-round timing/memory matrix remain release gates.

| Raw log retained outside Git | SHA-256 |
|---|---|
| Native ordinary/solver gate | `787c5a6a54243d02a3011218c8645991c1225ba9ae94fe19822432afb3ae7022` |
| Native actual factory | `278b9c4414120b4c0dec638c1ed23472e2c00cd6da5292494feb4adb1624588b` |
| Native PR conformance and real first-Serve | `649d91d9ee6897dfee5b6583484d6bae4e531b91ee00989e393c10b8978c6a71` |
| Mac host gate and actual factory | `6d4681e8f4dfd3e6e94215bd33bdd299dcd7dd26375fff8d3c0018993332501d` |
| Mac real first-Serve and exact unit/run map | `c3f8974be208b99f21fd4fb6ecac3d2361b29245f9a4dcbd527664fc6c0ead78` |

Indexed attempt, `3250ef65`, initial native first-Serve setup: container copy
of the conformance helper fails because only development binaries existed.
Building the release conformance binary before the same setup succeeds;
this is tooling setup, not a verifier or admission failure.

## Repeated cold calls and actual project oracle

Three rotated independently cold before/after pairs retain the exact same
pinned Vest tree, dependency closure, flags and pristine digest
`6c48c625d35795d7d84d666ef178f386ddcb15db6844e2b8921c4e03158c07b9`.
There is no trace or memory sampler in these intervals. Before uses the
previous fallback; after serves the new profile. The combined after build
includes a disabled source-memo prototype.

| Environment | Before median, s | After median, s | Scope |
|---|---:|---:|---|
| c7i.8xlarge, us-east-1 | 67.992643 | 47.133901 | Three passing cold pairs; `7ceb6423` to `ee105bbe` |
| mac-m4.metal, us-east-1b, macOS 27.0 | 38.874897 | 29.906487 | Diagnostic only: sustained background CPU; `5468b163` to `8f08256e` |

The uncompressed staged bundle grows from 51,354,055 to 154,051,518 bytes on
Linux and 102,712,988 to 205,418,036 on Mac. These are complete staged trees,
not compressed archive sizes or an isolated proof-size estimate.

A later Mac actual-project control uses the independently built `d6786f96`
stage, the new profile, and stock Verus at rlimit100. Passing, deliberate
`assert(false)` and restored calls all agree on human stdout, complete
production-compared diagnostic messages and exit codes (0/101/0). The existing
strict schema/per-function checker agrees on all 3,127 functions before and
after restoration. The failing strict JSON comparison refuses: existing
incomplete attribution triggers the documented VI-0003 conservative policy
(0 verified, 1 failed, 3,127 unknown). The new false proof is explicitly failed;
no successful function is promoted. That failed comparison remains retained
and is not claimed as strict negative per-function acceptance. No comparator
or production verdict rule is changed. This profile adds no CapybaraKV or
Anvil closure; #563 remains open for its broader scope.

| Additional raw evidence outside Git | SHA-256 |
|---|---|
| Combined full E2E/fast/standard gate | `8ab9ae674857b5588dc71432d350a1a4e58232b0eb7926cc0a73e547d56cd35e` |
| Linux three cold pairs | `87fd837751ee51931ef3da9533abf3adf4585b6d76941cb18af20d75e9ded134` |
| Legacy Mac three cold pairs | `19c8840b238513fa949b51e42fcfcc92d4b1e7a6dab5c5ee1a6199176651815e` |
| Mac actual oracle, including refused strict negative JSON | `0dbfbfdc4584f20c972b5533d710e88cea340626ccd3bf5c0581a103d8455add` |
| Mac restoration and explicit conservative-failure controls | `f488ff4ffea9a87b7e7fe55abf0adf31e996b77af6a8ae4e5852bf36f6827531` |
