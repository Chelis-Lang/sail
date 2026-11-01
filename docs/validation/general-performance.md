# General performance qualification

**Corrected runtime candidate (`0ad405eb`):** both native runtimes are built
and locally packed. All 41 focused regressions pass on Linux and macOS;
Mac's broader comparison matches all 23 units / 5,357 function records,
including the original abstract-domain proof. All three complete Linux fork source
arms pass 4,907 Same / 4,844 nonignored passing tests / 63 ignored, with
zero differences, unstable results, infrastructure failures or failed tests. The source-suite capture `source-0ad-complete-evidence.tgz` has SHA-256
`ca4b0e61e554a2032fbb99da284f9389dddfc7596b3f73bf24e68024fbe8a2f6`.
Sail `b28d14a8` passes the native Mac host gate (Clippy, 1,285 tests /
54 skipped); its indexed factory and installed qualification are running. The first Mac
packaging wrapper selected examples without rebuilding release bins; that
capture retains its older copied binary identities and is not candidate
acceptance. The corrected wrapper builds bins explicitly before staging.
This stages the corrected artifact identities, not release acceptance.

Review found that `4cd6bd5f`'s companion omitted the original solver-version
check. A recursive-spec-only negative control forwards the actual pinned Z3
but changes its version response: the old runtime rejects it, while `4cd6bd5f`
reports an error yet exits zero. The correction copies the original expected
version to the companion. The version request was already unconditional;
no SMT command, query or limit is added or rewritten. A new regression checks
both rejection and the existing explicit opt-out.

Both platforms' bounded research-route Vest cycles pass all 16 stock/Sail
calls, including failure/fix and both returned A reports. Both targets remain
complete and Current, keep their original establishing runs and exact report
payloads, and start no verifier, shim or solver on either return. Fresh stock
comparisons agree on all 3,127 Vest and 1,871 vstd functions for both returns.
The ordinary installed route now passes on Sail `b28d14a8` on both
platforms: shipped vstd is served, only one project verifier runs cold, and
both complete returned reports match fresh stock per-function results.
Mac's explicitly rebuilt release binaries also pass the actual four-unit
factory, installed output loop, full managed suite, fast 147 / 3,451 and
broader 23 / 5,357 comparisons. Its complete functional and ordinary-route
capture is `b28-mac-functional-ordinary.tgz`, SHA-256
`f938f06cac772d134f19e9e778afca0ddf9af34863c28c79bdf85f87b3f19ea6`.
Linux's corresponding raw capture follows its isolated measurement cohort;
its indexed bundle/SDK is already retained separately. All 147 Mac fast-tier
units are deterministic over three actual `0ad405eb` runs. The completed
Mac compiler/AIR/focused-regression build record and original manifest
inventories are retained in `source-0ad-mac-native-evidence.tgz`, SHA-256
`2f86f112d00a46fa4253f4b32f47b3e7fe9880e89718989c84bc38306d787f26`.
Final observer controls, edit loops and product costs remain separate gates.
Raw captures: Linux `retained-0ad-linux-evidence.tgz`, SHA-256
`9f7c2bf871f26d0098bf1fd5af0bd8f25c02220c576121c480569b92e39a9248`;
Mac `retained-0ad-mac-evidence.tgz`, SHA-256
`d1055339b6518ffa0800b960f36ff1f5348609d17525e652cc19e2bbe82e55e4`.

The earlier `d18961a1` / `4cd6bd5f` results below retain their actual identities.
Its complete six-round Linux matrix has 432 rows and passes complete-message
reprocessing under ADR 0042, preserving all six original IronKV ordered-headline
failures. Raw capture SHA-256
`96f673a82cab54dea84bf040eb2ec4d17457465521179ed7ab7018deac221fbd`.
Cold medians are faster on all four projects; every first-unchanged and
replay-disabled median remains slower. These timings do not establish parity
or qualify the corrected runtime's final artifact.

**Earlier composed status (`d18961a1` / `4cd6bd5f`):** broader tooling and the owner-approved ADR 0092 bounded
prototype are implemented. Both optimization experiments remain unselected;
platform defaults are unchanged. The installed-output repair (#633) passes its
original controls on both platforms. Guarded fork `4cd6bd5f` passes all 40
focused regressions and 146 AIR tests on Linux and Mac. All three complete
Linux source-suite arms (pass-through, cache-fill, cache-warm) report 4,906
Same cases, including 4,843 passing nonignored tests and 63 ignored tests;
there are no differences, instability, infrastructure failures or failed tests.

Corrected stock source qualification passes 4,867 Same cases: all 4,804
nonignored tests pass, with 63 ignored and three explicitly recorded
owner-approved ADR 0093 debug-order exceptions. The source builder generates
the release's own Cargo-Verus scaffolding data for stock variants. Earlier
matching-failure and strict-debug-order results remain separate.

Candidate Sail `d18961a1` passes the complete native Linux workspace gate
(1,295 tests / 54 skipped), 51 solver controls, complete E2E including extra
and all four fast arms (147 Same / 3,451 function records). All four standard
arms also pass (159 Same / 13,166 function records), with all 18 mutations
and both return controls matching. Deep and final expanded-corpus
qualification remain in progress. All six additional Linux rounds of four
Hekate proof edits and six TLA edit/failure/fix states pass their strict audit;
these are functional controls, not isolated performance measurements. Fresh macOS 26.7 qualification passes
Clippy and 1,284 host tests / 54 skipped, the actual factory, installed-output
loop, complete managed suite and fast comparison (147 Same / 3,451 function
records). The broader Mac comparison is
23 Same / 5,357 function records: 22 passing units and one matching incompatible
RLSF compilation. These are candidate results, not release acceptance.

Ordinary retained proof reuse passes the Vest fail/fix/return cycle on both
platforms. Both returned A calls preserve their complete Current reports and
original establishing provenance with zero verifier, shim or solver starts.
Linux's installed first run serves vstd and invokes only the requested-target
verifier. A separate public installed-bundle Mac first-use control also serves
vstd and starts one verifier; its earlier unbundled functional wrapper does
not establish bundled cold performance. See the [reuse record](instant-revert.md).

The corrected observer passes its fixed ten-pair Linux experiment: all 440
samples / 22 strata satisfy the original noise criterion. This qualifies the
observer on the measured unchanged capsule, not new-runtime performance.
The fixed six-round Linux release matrix and expanded native screen are
running on isolated measurement hosts. Its first IronKV failing-edit block
exposes an overly strict benchmark order check: complete diagnostic bytes and
counts match under the already accepted ADR 0042. The corrected benchmark
adapter uses the authoritative Rust splitter, with body/multiplicity negative
controls; original ordered-headline failures stay retained.

Mac 27 quiet-host checks remain refused at about two background CPU cores,
with repeated App Store scheduled activity observed in retained traces.
Fresh macOS 26.7 functional qualification passes on the same native hardware;
its fixed baseline, edit, determinism and observer controls precede new timing.
There are zero accepted new Mac timing rows. Earlier captures remain separate,
with unchanged measurement and conformance requirements. No release or new
performance-default acceptance is claimed.

The [review](../design/general-performance.md) defines the unchanged authority
boundaries. The [evaluation guide](../performance-evaluation.md) defines the
expanded corpus, explicit runtime selection and separate diagnostic controls.
The existing [v0.2.0 matrix](v020-performance.md), observer-overhead failures and
strict stock source-suite failure retain their original scope and requirements.

| Gate | Current evidence | Limit |
|---|---|---|
| Broader selection | 18 workloads, four new repository pins; final Linux four arms each match 23 units / 5,357 function records | 22 passing units plus one matching incompatible TLSF compile; original Mac runtime has 22 same / 1 different (#634); guarded candidate has 23 same, with remaining qualification separate |
| Run-scoped observations | `32f7bba6` bundle evaluated in complete native real-project and generated whole-call controls | Unselected; no material gain, with small warm-call regressions. Runtime paths retain `715c0f5e` behavior |
| Linux functional controls | Final `6cfa1606`: ordinary 1,288/54 skipped, solver51, complete E2E plus extra, four fast and standard arms and 18 mutations/reverts pass | Full release and strict stock source-suite requirements remain separate |
| Algorithmic scaling | 2 generated units / 4,314 functions compare same; 84 complete whole-call samples match stock bytes, with Current/no-verifier warm checks | 8/64-package full-call medians remain flat/slower. The earlier 54× lookup component result does not justify selection |
| Per-file artifact observations | Approved bounded prototype built/tested on Linux/Mac; 280-row Linux on/off audit passes but every warm median is flat/slower | Unselected on Linux; Mac correctness gates prevent timing/default selection; off everywhere |
| macOS | Composed `d18961a1` host, factory, installed-output, complete managed, fast and broader gates pass on macOS 26.7; earlier Mac 27 has 40 passing fork regressions and ordinary retained reuse | Final controls and quiet-host timing remain |
| Compiler reuse | No persistent frontend reuse implemented or selected | ADR 0052 complete-input/output shadow qualification remains open |

Raw records stay outside git. Add their source/runtime/environment identities,
scope and SHA-256 here after completed captures; do not describe a build or
matching failed baseline as a passing real-project workload.

## Continued installed-output repair (#633)

The fix in `7627b9bd` preserves non-authorizing script observations when a
compiler first discovers a genuinely new source alias. Every old alias must
remain identical; script-package/declaration changes still reject reuse.
The establishing proof remains Historical and publishes no proof receipt.
No Currentness or complete-source checks are relaxed.

The same three-call control reproduced on native Linux with a symlinked
`TMPDIR`. Old `6cfa1606` (Linux) / `715c0f5e` (Mac) runs start one verifier,
then two, then none. Candidate `64fa9a0e` starts one, then none, then none.
All calls exit zero; both unchanged candidate calls have stock's empty output.
The full installed edit/no-op loop passes on Linux and Mac, retaining every
original mutation, restored-mtime, JSON, partial-scope and revert check.
The complete unbundled Mac managed suite also passes. Linux's complete image
gate passes 1,289 tests / 54 skipped; Mac's host gate passes Clippy and
1,278 tests / 54 skipped. These are the repair's functional results, not
final composed release qualification.

An independent seal compares all 419 Rust implementation/manifests, fork and
artifact pins, and observer-source files with committed `64fa9a0e` on both
hosts. Linux's original checkout metadata is `c975b8a7` plus the exact changed
files; Mac is clean `64fa9a0e`. This records tested-input equivalence without
claiming a new execution at a later commit. Native Linux is c7i.8xlarge /
us-east-1; Mac is mac-m4.metal / us-east-1c, macOS 27, Rust 1.99.0 / verifier
1.99.1, fork `47fb3be6`, pinned Z3.

| External evidence | SHA-256 |
|---|---|
| Linux old/new three-call outputs, producer reports and 419-file seal | `b44d24eec9352370a8ef16ed233005901b8c4718c34e3b8a0e4b44621327cc69` |
| Linux complete installed-output loop log | `8152f7f4d7e51980dd126f4386b2a3ee26dfe2f7e2ed2315113c028c5eff36b8` |
| Linux full gate/factory log, including later controller setup failure | `2af1eb6c008a22a2b33763ce9d0bdc20a4642a16b336ad1017060b475381df38` |
| Mac controls, installed loop, managed suite, factories and 419-file seal | `c47e077f2906062b7de6c176c24be58ed28e6d494f7061fa622ea6995f0d2de5` |
| Mac host/build preparation | `8377a3a09d50f3d32675c14226bfde328096c64e0e5c759081cc54898e8345a5` |
| Mac old/new controls, complete installed loop and managed controller | `229af3bd89e40c7945f43f5f0bc6e70fa1936f94e3a7bdbbc978d9c2045ba6a3` |

The earlier failures remain historical evidence. One Linux controller attempt
completed the gate/factory but omitted the required E2E-build directory;
its corrected setup and the installed test subsequently pass. No check or
output expectation changed. The standalone repair has not yet merged.

## Final native functional scope

Final producer `6cfa1606` on c7i.4xlarge / us-east-1 passes the complete
ordinary image gate (1,288 tests / 54 skipped), 51 solver/fault controls,
complete E2E including extra, all four fast arms (147 same / 3,451 functions)
and all four standard arms (159 same / 13,166 functions), plus all 18
mutations, ordinary revert and forced revert. Conformance reports zero
different/unstable/infrastructure/skipped results; resource drift remains
visible in 11 fast / 17 standard units.

Its additional candidate-manifest pass-through, cache-fill, cache-warm and
engine arms each report 23 same / 5,357 function records, zero failures,
and resource drift in 13 units; the engine focuses 22 units. RLSF remains a
matching failed compilation, not a passing allocator baseline. These arms
compare fork/solver integration and do not themselves exercise the CLI's
artifact memo; actual memo consumers are covered by E2E and the separate
producer-audited whole-call experiment.

Complete final conformance archive SHA-256:
`1a93ca651ea534bebbbfe6fb97adea36c0c71823f753fb7358dbe3ac5d690562`.
Factory archive SHA-256:
`a9ccb391de0536ed7ddd286a8b2710c2406be2e4a6a42cb3a807150160a5761c`.
Full gate log SHA-256:
`851a38137925d1f0782037c1bdc61ffb747abd4c923f912bfcba525897059b4a`.
Broader-controller log SHA-256:
`f18df18880ecc1d1dc953602642d2546411fc1ed661124e801abcc6d643c2430`.

## Original Mac qualification on fork `47fb3be6`

Native mac-m4.metal / us-east-1c, macOS 27 arm64, source `6cfa1606`, Sail
Rust 1.99.0 and verifier Rust 1.99.1. Strict host Clippy and all 1,277 tests
(54 skipped) pass, including the eight finer-observation controls. All 13
memory, 19 command, five quiet-host and eight evaluation-harness controls
pass. Current and previous `715c0f5e` native builds and actual release
factories use the same fork `47fb3be6`, Mac runtime manifest
`ea6853a815475820ad3cfeb2a6d9b684e4fff82506eac8fba0cc938ef6359996`.
The complete native managed suite passes in its original unbundled-runtime
context, including restored-mtime failures, fresh copies, native compiler
replacement, missing receipts, prune, jobs, MCP and generated/ghost inputs.

Installed output E2E fails its external-input discovery case on current and
previous Sail; the default baseline with the prototype absent also fails.
The checked-in one-function reduction independently reproduces the extra
second verification, with the following call a no-op on both sides (#633).
This remains a failed installed-output gate, not an accepted integration
exception or evidence of a wrong solver answer.

The wider Mac comparator reports 22 same / 1 different over 5,357 function
records. The abstract-domain proof passes three stock runs and fails the
fork arm (VI-0040, #634). Independent whole-package controls give:

| Runtime / integration | Result, 909 function records |
|---|---|
| Stock / bare shim | Same, no resource drift |
| Stock / managed flags and shim | Same, no resource drift |
| Fork / bare shim | Different, same postcondition |
| Fork / managed flags and shim | Different, same postcondition |
| Fork / direct pinned stock Z3, no managed flags or Sail shim | Different, same postcondition |

The failing comparisons retain all three stock confirmations and their
original messages. This places the observed difference outside the cache
prototype and shim; the exact fork/query change and standalone reduction
remain open. No proof, limit, output or comparison is altered. Mac performance
and memory selection stops at correctness, with every default unchanged.
The earlier complete release matrix and sampler-overhead failures retain
their original requirements; no acceptance is inferred from this host gate.

Full Mac factory/functional/corpus/isolation/reproduction archive SHA-256:
`f71416129dfb411543e700962b76374bade14793830a58467b95b9cdd049bb30`.
Isolation log SHA-256:
`48520bca2fe4d5141d3c71eddea6deaef35eb659af14113c387d93a768817a44`.
Checked-in parameterized reproduction execution log SHA-256:
`2cbff1c6c9decb76a29ca0615ea56a3ff5fced745105dadd0e4643fe2c0abf1b`.

## Whole-call selection

Source `32f7bba6`, previous Sail `715c0f5e`, fork `47fb3be6`, stock
`7d829946`, native c7i.8xlarge / us-east-1. Six rotated blocks retain every
stock/current/previous sample on pmemlog, Verus-TLA and held-out containers:
288/288 rows, no refusals, unchanged source and executable identities. Values
below are Sail medians in seconds; first and full-path rows are distinct.

| Workload | Previous first | Candidate first | Previous full path | Candidate full path |
|---|---:|---:|---:|---:|
| pmemlog | 0.18195 | 0.19471 | 0.13351 | 0.15038 |
| Verus-TLA | 0.17861 | 0.19039 | 0.13757 | 0.14869 |
| Containers | 0.92503 | 0.90681 | 0.55707 | 0.54127 |
| Anvil, separate control | 1.82556 | 1.91660 | 1.09161 | 1.08376 |

Anvil contributes 28/28 successful rows: two fresh contexts per version,
with ten repeated full-path calls per version, not ten independent cold
rounds. It has no stock timing-acceptance claim. Cold medians in the three
paired projects remain broadly flat; their paired stock controls are retained.
Stock full-path medians are 0.10078 / 0.09892 / 0.20572 respectively. The
bundle does not remove the first/full-path parity blocker and is unselected.

The separate serial-Cargo generated control uses source
`6f9241658d4fee01155a4919d480087d47302f91`: chain 8/64 packages, four
modules/package, one root binary and every package a default member. Its
strict stock/fork comparison reports 2 same / 4,314 functions, zero
different/unstable/infrastructure/skipped results, with resource drift in two
units. All 84 timed rows match stock stdout/stderr/exit exactly. Every warm
Sail observation is Current with zero verifier starts and complete requested
targets. Two fresh contexts/mode supply ten repeated full calls per size;
these are not ten independent cold rounds.

| Packages | Stock full path | Previous Sail | Candidate Sail |
|---|---:|---:|---:|
| 8 | 0.11203 | 0.17735 | 0.18255 |
| 64 | 0.17443 | 0.32924 | 0.32855 |

This complete whole-call control does not close the separate parallel
workspace/all-targets framing/identity failure. Archive SHA-256:
`72f783b6adb904ddd7f2a4528f16d30691de7d5e3a78e58e5deba7b99942e421`.
It retains reports, every sample, raw outputs and Current producer records.

## Linux functional scope

On native c7i.4xlarge / us-east-1, source `ab5fcddb` passes 53 ignored tests
(51 solver/fault controls plus two component diagnostics),
the complete E2E command including the local extra cases, and all four fast
and standard conformance arms. Each fast arm reports 147 same / 3,451
functions; each standard arm reports 159 same / 13,166 functions. All report
zero different, unstable, infrastructure errors or skipped projects; resource
limit drift appears in 11 fast / 17 standard units. Standard mutation/revert
checks also pass. Its earlier ordinary Clippy failure remains a failure.

The corrected source `32f7bba6` changes only the two Rust comment spellings
relative to that production implementation, and passes the complete ordinary
gate on c7i.8xlarge / us-east-1. Its release stages and strict clean-source
intake control build and pass; the later generated comparison refuses as
recorded below. No complete controller success or release acceptance follows.

Conformance archive SHA-256:
`cdc12f0bf5a9146b99cf4ce223e4a371dcafe6a4865cef057e0d5549db15412f`.
Its gate log SHA-256:
`0e9b0212600f1ffa46bdedfbcb00367330c3c28d9f8007fd22c77d74c253de03`.
The corrected ordinary-gate/build/control log SHA-256:
`327debb625c64a8036d4bfc7c69fad35c1b6f44cb61f0a17a1d84234133cd153`.

## Linux per-file selection

Source `6cfa1606`, native c7i.8xlarge / us-east-1; current runtime versus itself
with exact ADR 0092 opt-in off/on. Two fresh cold contexts per project provide
ten rotated repeated warm full-path pairs, not ten independent cold rounds.
All 280/280 rows pass completeness, exit, immutable source/binary and producer
audits. Every controlled warm call is Current, starts zero verifiers, retains
unchanged input/context and requested-target proof reports, and has no
no-reuse or infrastructure error. The off arm creates no fine ledger; every
on sample has eligible original entries. Ledger presence records observation,
not proof that a digest was admitted.

| Workload | Off full path, ms | On full path, ms | Median paired change, ms | On faster, pairs |
|---|---:|---:|---:|---:|
| pmemlog | 135.912 | 141.608 | +4.769 | 1/10 |
| verus-tla | 139.576 | 143.018 | +2.952 | 2/10 |
| semi-containers | 554.900 | 556.609 | +1.696 | 3/10 |
| anvil | 1090.744 | 1105.969 | +14.844 | 1/10 |
| vest | 437.646 | 439.760 | +1.534 | 5/10 |
| ironkv | 140.278 | 144.665 | +5.956 | 2/10 |
| capybarakv | 333.401 | 334.998 | +3.481 | 3/10 |

No workload shows a warm median gain. The finer memo is **unselected on
Linux** and remains off by default. Cold/first calls are one observation per
flag/project and establish no acceptance; first pmemlog increases from
180.613 to 223.869 ms. No first-after-cold parity, edit-cycle, memory or
release acceptance follows. Mac has a different existing whole-tree default
and is qualified independently.

The full native archive includes every earlier compatibility, edit, generated,
failed/interrupted and diagnostic control plus this seven-project run.
Archive SHA-256:
`e01fa4cd6b571c3e53bcd463aa6dc0bdce0be965a66f079733d3aa6fda502580`.
Audit log SHA-256:
`e11589827b5232a8eb8daf6d3fe7bf73fffd554bb1612b37c31c5660b6b495d4`.
Exact producer source bundle SHA-256:
`22c23ff8fc27ffc908a6b550ae8a4b22a22c1aa395dcb23c2beed863507aec63`.

## Approved prototype controls

The broader prototype source `e6c6ddf4` passes its complete native controller
on c7i.4xlarge / us-east-1: ordinary gate (1,292 passed / 56 skipped),
51 solver/fault controls, complete E2E including extra, all four fast arms
(147 same / 3,451 functions each), all four standard arms
(159 same / 13,166 functions each), and all 18 mutation cases plus revert.
All conformance arms report zero different/unstable/infrastructure/skipped
results; resource drift is retained in 11 fast / 17 standard units. The eight
new prototype controls cover explicit selection, provenance and settlement,
namespace churn, full stamps and ancestor replacement, malformed records,
phase/publication mutation, and the actual Runner source/receipt bridge.
This source still includes the unselected initial runtime bundle; it is not
qualification of the narrower final composition or a speedup claim.

Prototype conformance archive SHA-256:
`c368f2d797cb54ac027c7ee69dca43bb56ffbb2b138d95a190545833013ab056`.
Factory evidence archive SHA-256:
`8fc3045a6fa0405d51f036fdbe8bf49e9e1fc225354b73c5bc0e264fdc27991d`.
Complete controller log SHA-256:
`1d741ab34b62620c08fe441382e474abb3fe91010fa2cbfa2f137c4cfa3f0191`.

## Compatibility findings

The initial successful baselines use stock `0.2026.10.18.7d82994`, its pinned
Z3 and Rust 1.99.1 on c7i.8xlarge / us-east-1. Verus-TLA uses scratch source
dependency adjustment; hekate-math runs its original 18 standalone proof roots.
Both arms of every passing unit exit zero with zero verification errors;
hekate-math reports 3,050 verified occurrences per arm across its roots. These
are compatibility/comparator controls, not timing acceptance.

The corrected selected-fork comparison at `32f7bba6` subsequently passes
all 23 candidate units: 5,357 function records, zero different/unstable/
infrastructure/skipped results, with resource drift in 13 units. Twenty-two
units have passing plain and fork baselines. The RLSF unit retains its
matching compilation failure and zero verified functions. This compares the
fork's managed verifier flags/shim with stock; it does not execute managed
receipt reuse or establish that the finer artifact memo is used. Source and
binary/manifest end seals remain unchanged. Mac and final-composition
qualification stay separate.

Initial intake/component archive SHA-256:
`1b28f575e3317810594778665e628f017ab8267ad0a6fcd94eb426ff7aeecc67`.
It retains both intake attempts, the original comparator reports, selected
binary digests and all component samples, including slower JSON samples.
Source is the recorded working tree based on `715c0f5e`, before the complete
workspace preparation fix; it is not a clean-head release acceptance record.

Semi-persistent exposes a harness preparation defect: patching only the
selected package leaves conflicting exact vstd pins in other Cargo workspace
members. The candidate now discovers the complete workspace with Cargo's
offline, dependency-free metadata and adjusts each scratch member consistently
on both sides. The clean `b9466ece` rerun passes all three selected packages: containers,
abstract domains and AU, with 2,430 / 909 / 29 function records compared.
Fetched sources and proof bodies are unchanged. Selected-fork held-out
timing and proof edits remain separate gates.

The TLSF pin does not compile with the pinned vstd: `Set::new` returns an
`Option<Set<_>>`, and `Map::new` expects a set rather than its predicate argument.
Both arms report the same two E0308 errors and compare zero functions. This
is an incompatible candidate, not a successful allocator baseline. No proof
port or runtime substitution is included.

## Evaluation controls

The subsequent seven-control harness passes natively on c7i.4xlarge /
us-east-1. It additionally refuses changed or missing measurement inputs.
Two actual syscall-observer executions preserve an unset child `LC_ALL` and
an explicitly supplied `C.UTF-8` value. Their native commands exit zero and
produce complete syscall records. Archive SHA-256:
`9d3feb9245421e9cdd6e0db9fbef44ebe50cc3c54078737e275e41f83e6842f9`.
This is harness correctness, not a product speedup. The compiler operation
[audit](../design/frontend-lowering-boundary.md) is static source review;
it supplies no complete-key or restoration acceptance.

## Failed attempts

- 2026-10-31, `6cfa1606`, native Mac installed output E2E:
  external-input discovery adds vstd/target totals on an unchanged call;
  unchanged baseline and default-baseline controls reproduce it (#633).
  The original E2E collector reaches its unchanged 1,024-file ceiling;
  critical original streams remain, archive marked incomplete.
- 2026-10-31, `6cfa1606`, Mac broader manifest: 22 same / 1 different;
  abstract-domain postcondition fails only in the fork arm. Bare-shim and
  direct-pinned-Z3 controls repeat it; cause remains unestablished (#634).
- 2026-10-31, `6cfa1606`, first Mac managed-suite invocation:
  the prune counter expects two verifiers in its unbundled test context,
  but the controller supplies an adjacent shipped proof bundle and observes
  one. The original assertion remains; the unbundled-context rerun passes.
- 2026-10-31, `6cfa1606`, Mac controller setup: streamed bootstrap does not
  execute the Rust/gate steps after Homebrew; uploaded-file execution completes
  them. A subsequent stock-path assumption fails before verifier work; the
  installed path is used for the completed runs. Neither is product acceptance.

- 2026-10-31, `32f7bba6`, initial selected-fork broader comparison:
  23 different because the private controller supplies a relative engine
  path, which cannot be found after the harness enters a scratch directory.
  Sail arms exit 127; the unchanged comparator correctly refuses them.
  The absolute-path rerun above passes; the failed reports remain retained.

- 2026-10-31, `b980b77c`, active ADR 0092 E2E: the compiler-environment
  witness's clean-caller block leaves the new opt-in setting present. Stock
  and Sail have identical streams/exit 101 with 1 verified / 2 errors, but
  the unchanged clean-caller assertion requires 0 verified / 3 errors. Extend
  that block's existing unset list and its separate ambient-caller case to
  the new switch; retain both strict tallies and byte comparisons. The failure
  collector reaches its existing 1,024-file limit; relevant original streams
  are captured but the archive is explicitly incomplete. No limit is raised.

- 2026-10-31, fixed generated source `7e7ec297`, candidate `32f7bba6`:
  serial Cargo retains all targets and produces complete JSON streams, but
  independent crate order differs. Chain compares same; diamond/shared
  compare different in function details (1 same / 2 different). Equal
  aggregate verified totals do not close per-function or parallel acceptance.

- 2026-10-31, fixed generated source `7e7ec297`: all three parallel
  workspace/all-targets comparisons still refuse. Every arm exits zero, but
  repeated stock output fails JSON parsing after 1–7 complete documents;
  function/verified totals are incomplete. Preserve 3 different / 0 same and
  the additional plain controls. A serial-Cargo control is separate; see
  [VI-0039](../verus-issues/VI-0039-concurrent-cargo-json-framing.md), #632.

- 2026-10-31, `ab5fcddb` / `0ea95898`: composed ordinary gate stops at
  Clippy on two unquoted Rust identifiers in the closing-barrier comments.
  Its controller continued the independent solver/E2E/conformance checks;
  those results do not establish an ordinary-gate pass. Quote the identifiers
  and rerun the complete ordinary gate.

- 2026-10-31, `b9466ece` held-out capture: Git exited zero while warning
  that a build-output directory was unreadable. The initial runner treated
  this as a clean checkout; full clean-source inventory was not established.
  The interrupted capture remains limited evidence. The runner now refuses
  Git warnings, with an actual native warning-refusal and clean-source control;
  composed-candidate timings require a new sealed capture.

- 2026-10-31, `b9466ece` generated source `0f63559f`: all three shapes
  refuse the full workspace/all-targets comparison. Generated binary targets
  lack the required Verus prelude; parallel Cargo failures produce different
  trailers and partially completed crate output (3 different, 0 same). The
  generator now imports the prelude and checks a root-library call. Original
  comparator checks remain unchanged; the valid rerun exposes the separate
  framing/identity failures above (#631, #632).

- 2026-10-31, `b9466ece` benchmark with first external loop catalogue: the four
  arithmetic edit expectations and first five temporal-logic expectations
  pass; the final refinement matches stock's compiler failure and refuses
  its expected-positive outcome. Stock's minimal reproduction
  reports E0308 (`nat` addition assigned to `int`), zero verified/functions
  and zero verification errors. The matching nat local passes a native minimal
  control. The corrected four-edit/six-state catalogue passes six complete
  rounds: 84 same comparisons, every expected stock outcome matched, zero
  different/unstable/infrastructure errors. This qualifies exact-query-cache
  edit behavior, not a managed-receipt speedup. Do not
  count the matching compiler failure as passing edit qualification. See
  [VI-0038](../verus-issues/VI-0038-literal-addition-infers-nat.md).

- 2026-10-31, `b9466ece` diagnostic controller: perf discovery did not follow
  the installed kernel-tools directory symlink; the retry then lacked an
  explicit shipped vstd lock beside its wrapper. Both refused before a timed
  invocation. Corrected the diagnostic controller paths.
- 2026-10-31, `b9466ece` first syscall diagnostic: setting the tracer's locale
  also changed the child's `LC_ALL`, causing receipt invalidation and verifier
  work. Interrupted that capture and retained it as an invalid unchanged-call
  diagnostic. The helper now restores the original child's locale; the next
  profile sets one consistent locale for cold and all subsequent calls.

- 2026-10-31, `8f298701`: workspace preparation composition failed to compile
  because registry-adjustment logging lost its repository-root parameter
  during extraction. Restored that explicit parameter before native rerun.

- 2026-10-31, working tree based on `715c0f5e`: first native candidate gate
  failed Clippy's needless-borrow check in the benchmark CLI. Corrected in the
  subsequent native subset gate; no check or baseline changed.
- 2026-10-31, working tree based on `715c0f5e`: first intake refused all six
  selections because independently installed tools looked for a sibling shim.
  The runner now passes explicit executable paths; the rerun retains all units.
- 2026-10-31, working tree based on `715c0f5e`: semi-persistent's three package
  attempts refused Cargo resolution due to incomplete scratch workspace pin
  adjustment. TLSF reports matching compiler errors. Both remain explicit.

## Completed candidate captures

Composed candidate `d18961a1`, guarded fork `4cd6bd5f`, native c7i.8xlarge /
us-east-1: the complete image workspace gate, 51 solver controls, complete
E2E including extra, all four fast arms and all four standard arms pass.
Fast/standard have 147/159 Same units and 3,451/13,166 function records;
all report zero different, unstable, infrastructure or skipped results.
Resource drift remains visible in 25/32 units. All 18 standard mutations,
ordinary return and forced return match. Complete raw capture
`d189-linux-core-and-standard.tgz`, SHA-256
`4ac157c68db31c6bf6d5dd4821efd2ec61da519d65a203a401136e609c38385a`.
Six additional rounds retain all four Hekate edits and six TLA loop states per
round, with their expected verdicts, complete sample sets and exact comparison
counts. All pass; functional jobs share this host, so their timings do not
qualify costs. Audited capture `d189-linux-broader-edits-audited.tgz`, SHA-256
`fd336e8b6b0d0d8a24fa887892247ba91bab6c3621e46c1b6e9d3b769c13295d`.
Deep, expanded-corpus and measured-cost requirements remain separate.

All three complete exact fork arms and the corrected stock source control
pass. Counts, source/runtime identities, the three explicit stock ordering
exceptions and complete archive hashes are in the
[source-suite gate record](deep-tier.md#guarded-candidate-source-suite-gate).

Mac 27 candidate `d18961a1` host/factory/installed/managed/ordinary checks and
retained diagnostic records are in `mac-d189-complete-evidence.tgz`, SHA-256
`e1b7399e1a58b3dc6d16a5067cfce0036c464627e17de334574883a353bdadc2`.
Its busy-host timing refusals remain refusals. Bundled first-use and unbundled
functional cases are explicitly distinguished in their raw captures.

Fresh macOS 26.7 / native mac-m4.metal / us-east-1c, Sail `d18961a1`, Rust
1.99.0, verifier Rust 1.99.1, exact fork `4cd6bd5f` and runtime manifest
`efe2d7e…9253028e`: Clippy and all 1,284 host tests pass (54 skipped), along
with 13 memory, 19 command, five quiet-guard and eight corpus-harness controls.
The actual factory, installed-output loop and complete unbundled managed
suite pass. Fast conformance matches 147 units / 3,451 function records;
broader conformance matches 23 units / 5,357 records, with 22 passing units
and the matching incompatible RLSF compilation. Both comparisons have zero
different, unstable, infrastructure or skipped results; resource drift remains
visible in 25/13 units. These are functional gates, not timing acceptance.
Complete capture `mac26-d189-functional-evidence.tgz`, SHA-256
`954eb2953e011eaf160e7cf5ba86efdd7ee11fa73f67fe884655705a422a11d7`.

## Composed candidate attempts

- 2026-11-01, `d18961a1`, macOS 26.7: the initial SSH host gate has child-launch
  timeouts; an isolated daemon case also fails before its socket appears.
  The unchanged case passes in 0.3 s from a Terminal configured under
  Developer Tools; the complete unchanged host gate then passes there.
  The SSH control remains failed. The first subsequent factory attempt
  cannot fetch the unpublished fork source; a local mirror of the exact pinned
  Git objects permits the existing notices and factory checks to pass.
  Original logs remain in the fresh Mac capture above.

- 2026-11-01, `9a24d39e`: Linux workspace and Mac host gates fail Clippy because unconditional
  scaffolding generation leaves an unused private `fork_tests` argument.
  The argument and its call-site value are removed; no lint or test changes.
  The composed gates and ordinary-route checks require the corrected source.

- 2026-11-01, `d18961a1`: the first private Linux ordinary-reuse audit
  assumes two cold verifiers and a cold-run vstd origin. The actual bundled
  run correctly starts one verifier and has a shipped vstd origin. The
  installed-bundle audit explicitly requires served vstd, its shipped origin,
  one requested verifier and exact preserved per-target return provenance;
  all checks pass. Original samples and failed assumption remain retained.
- 2026-11-01, `d18961a1`: the first IronKV matrix negative edit fails the
  ordered-headline check although all four complete diagnostics match under
  ADR 0042. Their original ordering and raw logs are retained in the
  [minimal output fixture](../../crates/sail-conformance/tests/fixtures/benchmark-mixed-diagnostic-order/README.md).
  Full-message checking and unchanged ordered-remainder checks pass.

The authoritative splitter adapter additionally rejects non-object JSON and
non-integer schema versions, retaining a failed benchmark row rather than
aborting the capture. Native Linux's complete gate passes 1,296 tests / 54
skipped, including 34 Python matrix controls and the real mixed-diagnostic
Rust fixture. The same 34 Python controls pass on macOS 26.7; its previously
qualified Rust helper is unchanged. The native diagnostic checkouts contain
`d18961a1` plus the exact diagnostic implementation and fixture, separately
sealed; frozen measurement checkouts and binaries remain `d18961a1`.
Linux capture `benchmark-diagnostics-r2-gate.tgz`, SHA-256
`ebfec6dbf1cb9ed719d26b5db1a25eac552fde26058bd401515b23e519be4ce7`;
Mac control capture `mac26-benchmark-diagnostics-r2.tgz`, SHA-256
`d36015528c4e51621ef1f8e08bbc883041f67a67991289fee7812a6b87c28ef9`.
- 2026-11-01, fork `f2e551f`: the new version-control regression's negative
  half passes, but its positive half uses an invalid bare opt-out option.
  The fixture now uses the existing `-V no-solver-version-check`; all five
  termination controls and both platforms' 41 focused regressions pass at
  `0ad405eb`. The original failed test results remain retained.
