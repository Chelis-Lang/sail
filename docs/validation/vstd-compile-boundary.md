# vstd compiler-boundary replacement

| Gate | Current state |
|---|---|
| Implementation | Real Cargo compiler-wrapper prototype; requires `SAIL_VSTD_BOUNDARY_RESEARCH=1` |
| Native prerequisites | Every native compiler executes normally; current-run successful provenance and actual linked bytes required |
| Reuse scope | Pinned, nonprimary vstd library only; no native units or Cargo fingerprints |
| Identity | Literal flags, actual runtime/sysroot/solver bytes, linked libraries/VIR, official source/feature closure and environment |
| Provenance | Complete successful proof required; typed origin and actual establishing invocation survive receipt reuse |
| Native performance | Direct-observation source `6ea94c8d`: three fresh-project pairs, median 39.840 s without reuse versus 9.336 s with reuse (4.27×); experimental |
| Compiler-input boundary | Native API and consumer run: 128 compiler-read source records in each GHOST/EXEC phase; complete input-model audit and mutation qualification remain open |
| Literal query comparison | Two separate cold controls match all 366 literal input streams / 2,061 checks each; this is not reuse or stock acceptance |
| Selection | Disabled by default; neither release nor default replacement qualified; v1 remains quarantined |

## Product boundary

`SAIL_SHIPPED_RECORD` records an experimental bundle; `SAIL_SHIPPED_DEPS`
serves one. `SAIL_VSTD_BOUNDARY_LOCAL` instead uses a separate machine-local
bundle bound to both literal and canonical registry roots. All require the
research flag. Local mode never remaps VIR or source paths. Schema 9 and the
bundle kind reject old all-unit bundles. Focused, engine and loader-variable
setups use the ordinary compiler path. Changed, missing or unsupported inputs
miss safely. No ordinary CLI, runtime pin or release asset is selected here.

The source policy admits 15 exact official packages, 662 archive file hashes
and their exact feature sets, excluding external macro providers and patches.
Each archive SHA-256 matches the retained resolution; source files are checked
directly, independently of mutable Cargo checksum sidecars. Native observations
bind the actual Verus launcher and embedded compiler driver, initially absent
declared outputs, generated inputs and successful current-run dependencies.
Cargo can pipeline a dependent after metadata emission, so observations require
completed producer provenance at completion, with a two-second arrival bound.
The compiler always runs; metadata readiness never establishes success.

## Actual native result

On native c7i.4xlarge in us-east-1, Sail source `6ea94c8d` and experimental
fork `0fe2bfb3` completed all ten application cases. Rust is 1.99.0 for Sail
and 1.98.1 for Verus. The actual Sail binaries are reused from the unchanged
source's passing Linux host gate (686 tests, 35 skipped, clippy). The new fork
build verifies vstd (2,060 checks, zero errors), passes all 15 compiler library
tests and produces a genuine normal release-style runtime. Its archive SHA-256
is `9444987774f0548f75508983843f8aea000ccf734999d8b032d6fa5ee44f06b5`.
No default runtime pin changes.

Independent physical projects use identical manifest, lock and source bytes
with separate targets and proof caches. These are Sail-with-boundary-reuse
versus Sail-without-boundary-reuse measurements, not a stock comparison.

| Round | Without boundary reuse | Fresh project with boundary reuse |
|---|---:|---:|
| 1 | 39.988 s | 9.413 s |
| 2 | 39.840 s | 9.125 s |
| 3 | 38.442 s | 9.336 s |

The initial fill takes 41.561 s. Every fresh project compiles native
prerequisites and verifies the requested target; only vstd is reused, with the
actual fill invocation retained. Its completed compiler document binds 128
loaded source hashes/sizes in each of the separate GHOST and EXEC phases, with
no extra tracked files. Both source sets are checked against the independent
archive policy. Replacing the same private native `cc` path fails with Cargo
101 in 1.808 s, before any verifier invocation.

All three pairs have equal function-verdict multisets (one requested function
and 1,871 vstd entries), complete targets and empty diagnostics. Reused vstd
has unavailable per-obligation detail rather than invented attribution logs;
this is not full structured-output parity.

The two separate cold trace controls use the original shim through a
capture-only adapter with daemon reuse disabled. Each retains all 366 paired
sessions, 2,061 checks and 34,957,056 input bytes. Every recorded raw
input/reply/metadata digest and complete child status is independently checked;
input SHA-256 multiplicities match exactly, without filtering or rewriting.
These controls establish repeatability on the simple unchanged project, not
whole-corpus parity or the absence of skipped verification in the reuse path.

The enclosing screen remains **FAIL**: after all ten outcomes and trace checks,
a redundant final binary-retention step tries to recreate an existing directory.
The actual original binaries had already been retained before application work;
final source checks still complete on all 597 Sail and 1,480 fork tracked paths.
No timing or compiler case is repeated to replace that outcome. Complete raw
archive SHA-256:
`209bac00f8777be98ca5cb69d2e06666709746206cd8fca6ff93b283e6e32487`;
application summary:
`9f5842469485868958b094cfb4a33a01b0980879af575cb17579e979ab63e0a4`;
literal control comparison:
`f9f24a2986e797d7f2ffa268958e1c275df7aceaa6e36090e5243b9f491fedc2`.
Full stock verdict/diagnostic parity, source/artifact mutation refusal, complete
macro/compiler input qualification and both-platform runtime delivery remain
required before default selection.

The earlier `50b116bb` prototype measured 37.871 s versus 8.954 s (4.23×), but
its trace collector captured nothing because the CLI set its own trace directory.
That original enclosing failure remains failed; summary SHA-256:
`eee54507754ad10346efbff41858bb5475c2f75d7f2ce25eedaf0194fd0a556d`.

## Compiler-read source checksums

The pinned compiler is Rust commit `48a229ceaefd4985c50990b14116b6d856af0985`.
Its [SourceFile constructor](https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_span/src/lib.rs)
hashes the loaded bytes before BOM/CRLF normalization and retains that checksum.
Its [dep-info writer](https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_interface/src/passes.rs)
emits the retained checksum and original size for local source-map files.
Cargo already requests BLAKE3 checksums; this change does not alter compiler
arguments. Independent expected BLAKE3/size constants for 137 admitted vstd
archive files are generated from the separately SHA-256-validated archive,
using an offline generation tool, with no new product dependency.

The dep-info-only revision requires every listed compiler source to be an admitted vstd
Rust file with exactly one matching compiler checksum and size. Missing,
foreign, duplicate, malformed and differently hashed inputs refuse publication.
Restoring a source file after the compiler read cannot restore its retained
source-map checksum, but dep-info does not reliably identify that origin. The existing whole-source SHA-256 and file-set checks
remain separate checks.

The dep-info-only check is additive, not a complete input boundary. The driver
runs distinct GHOST verification and EXEC compilation passes, and later dep-info
can describe the EXEC read rather than the proof input. Rustc separately reopens extra tracked
proc-macro, PGO and debugger files at dep-info emission; those checksums are
not evidence of their earlier reads. The prototype must not generalize this
source-map argument to arbitrary macro inputs, external artifacts or mapped
native libraries. Complete native qualification, source mutation controls,
macro/compiler builtin/span/diagnostic audit, full verdict and diagnostic parity,
and both-platform checks remain required before selection. Exported VIR alone
is not a complete proof-input key because Verus prunes private bodies.

## Direct compiler observation prototype

The successor fork records source-map hashes and original sizes separately from
both actual compiler passes, retaining them in memory until successful GHOST
verification and EXEC compilation complete. Missing, failed or duplicate phases
latch refusal. Only then can it write one bounded completion document. With no
experimental request it returns without recording; the observation is not a
proof verdict or a substitute for successful child exit and complete verification.

The Sail consumer creates a fresh private request, binds the actual spawned
compiler PID and key/run nonce, and requires both phases' admitted source hashes.
Any extra tracked file refuses publication. Both phases' tracked environment
values must match the actual invocation; their union also binds subsequent reuse,
so an EXEC-only dep-info file cannot erase a GHOST-only declared input. Schema 7
rejects older entries lacking this evidence. The native prototype above builds
and exercises this API; complete runtime qualification and selection remain open.

The `1d0f1907` dep-info-only native screen passed its host gate (683 tests,
35 skipped, plus clippy) and repeated three fresh-project pairs: median
41.431 s without boundary reuse versus 9.887 s with reuse (4.19×). The same-path
native compiler replacement still failed before verification. The enclosing
trace-control gate failed again with no captured input files. Source inspection
shows the cached shim returns before its observation tap, so setting the tap's
directory alone does not qualify that capture. Raw summary SHA-256:
`03a227b0cf3bbc291a91724a4bc5a5347e36f1f306dbe71283e89cfa89cb0731`; neither failed trace attempt is reported as query parity.
The actual Sail and original shim binaries from this screen are also retained.

Inspection of all three earlier `50b116bb` pairs finds equal function-verdict
multisets (1 user function and 1,871 vstd entries) and empty diagnostics. Reused
vstd has unavailable per-obligation detail; its baseline JSON run recorded that
detail. This is an explicit detail limitation, not full structured-output parity.
The new consumer does not invent the absent attribution logs.

The first Mac source check of the direct-observation consumer stopped at clippy
on a string-extension check and two expanded functions' lengths. No tests ran.
The source now uses the explicit path extension and extracted publication/cache
validation helpers; the original failed check remains retained.

## Prior attempts

All entries below are on 2026-10-17; failed or interrupted attempts remain failed.
Raw compiler streams, source packets, keyed observations and results are retained
outside the repository. No unit fixture establishes real-project acceptance.

- Initial source: 15 native tests passed; workspace lint failed on documentation.
- `930dfea8`: native compile failed on environment-pair types; application unrun.
- `930dfea8` corrected: 17 Mac tests passed; workspace compile found missing origin field.
- `05c4377e`: portable fill refused registry-bearing VIR; fresh attempt missed; changed native compiler failed before verification.
- `dcc2bb9e`: unrestricted local hit 8.456 s versus 37.892 s, one sample; arbitrary first-fill macros remained admitted, so unqualified.
- `b3f23535`: native provenance refused the presumed sysroot compiler identity; fill missed, later cases unrun.
- `67c7d0ea`: Mac clippy and 669/670 tests passed; error-code registry failed. Documentation correction passed its targeted check.
- `67c7d0ea`: native transitive provenance refused; fill missed, later cases unrun.
- `a9856b8f`: native records demonstrated Cargo pipelining; fill missed, later cases unrun.
- `def9c258`: native provenance reached proc-macro compilation but refused ordinary `-C prefer-dynamic`; fill missed. Complete Mac host check passed 671 tests, 35 skipped, plus clippy.
- `991d134d` fork / `6ea94c8d` Sail: Linux host gate and normal verifier build pass, but a compiler library test uses an obsolete source-name constructor; application unrun. Test-only `0fe2bfb3` corrects the pinned API.
- `0fe2bfb3` fork / `6ea94c8d` Sail: all ten application outcomes and literal controls complete; enclosing final retention fails as recorded above.
- `50b116bb`: native lint and 24 focused tests passed; three actual reuse pairs completed; enclosing trace-collection gate failed as described above. Mac lint, 24 focused tests and receipt-origin test passed.

The corrected direct-observation consumer passes the complete Mac host gate:
clippy and all 675 enabled tests, 35 skipped, with unchanged source and an owned
private target removed afterward. This qualifies source compilation and tests,
not complete runtime or default-mode acceptance; the separate native screen
above covers the experimental fork API and actual reuse.

Independent source review finds an extra diagnostic on damaged-entry fallback:
the prototype prints `SAIL3133` before running the real compiler. The successor
removes that output path and keeps refusal reasons in the optional trace.
Its 19 focused Mac boundary tests and all-target `sail-verus` clippy pass,
including damaged libraries, malformed destinations and unreadable manifests
with preserved refusal reasons and no installed outputs. Actual byte-for-byte
fallback output remains a native qualification gate.

The same review finds no additional steady-state input omission within the
closed official machine-local source/feature model. It does not attest native
artifacts' mapped bytes: before/after pathname hashes alone cannot exclude a
different macro library being consumed and then restored. A targeted actual
consumption control remains required; unrestricted hostile local cache writers
are outside the trusted machine-local cache model. This is an unresolved
qualification boundary, not a demonstrated wrong verdict or a default-mode
acceptance claim. Portable recording and serving remain separately unqualified.


## Private native inputs: implementation, native qualification pending

The native substitution control in [#222](https://github.com/Chelis-Lang/sail/issues/222)
uses Sail fixture `c0cb340b` (product `5bd9ca39`) and the genuine `0fe2bfb3`
runtime. Clean A completes with 1,871 vstd function records. Replacing the
builtin-macro output after keying and restoring A after the child exits produces
a different vstd metadata/VIR pair under the same schema-7 key. The vstd child
exits zero and publishes complete separate compiler-read phases; the unchanged
requested project then fails to import vstd with E0463. The complete units and
all artifact digests verify. The raw archive is
`5ee394306cac9f9c11f9e90deb8ff8a1010ea5eb61e9aa62c123dac83ae1dd39`.

The extra proof-canary assertion did not fire: the fixture instrumented
`verus_keep_ghost`, while this source uses the ordinary `verus` entry. That
control remains failed, and no clean consumer or wrong-verdict claim follows.
Independent review treats the controlled metadata failure as sufficient reason
for protective copies and a differential repeat, not as successful canary or
replay qualification. The preceding coordination attempt stopped before input
mutation on the actual Unix argument encoding; archive
`45ce80508a452fe9f267516c60c04222458347c35acc2f58dcb0e4c70ed00fca`.

The successor copies exactly the three admitted direct native artifacts into a
fresh owner-only per-invocation directory, checks their bytes against successful
producer provenance, and gives the actual compiler those private paths. It keeps
basenames, source paths and all other compiler arguments. Copies survive through
publication and are removed with the call. Original source/provenance rechecks
remain; copy failures decline the cache silently. Schema 8 rejects earlier
unsnapshotted records. Additional externs, adjacent VIR and foreign native
metadata graphs remain refused.

The actual Linux macro libraries import only libgcc, libc and the ELF loader,
with no target-library dynamic dependencies or search paths; builtin has no
non-sysroot dependencies. This closes the recorded three-input shape, not
arbitrary native graphs. macOS loader coverage, protected substitution, literal
output/query parity and full native acceptance remain pending. The earlier
4.27× measurement does not qualify this successor's performance.

The private-copy successor passes 32 focused boundary tests and all-target
`sail-verus` clippy on macOS. These include original-path substitution, private
copy damage, missing input binding, symlink refusal and rejection of the older
schema without placing outputs. They are source checks, not native acceptance.


### Protected native comparison: pass

Source `361a5cf7`, with coordination-only fixture `b8be218c`, and old fixture
`c0cb340b` were built genuinely on one c7i.4xlarge in us-east-1 with the retained
`0fe2bfb3` runtime. All five control outcomes meet their expectations:

- Old clean A completes with 1,871 vstd functions.
- Correcting the old control's ordinary `verus` entry produces the actual B
  marker and a 1,872nd vstd proof function. It still publishes under A's key,
  and the unchanged requested project fails with E0463 after A is restored.
- New clean A and the protected substitution both complete and remain current,
  with 1,871 vstd functions and identical per-function verdicts and diagnostics.
  The protected compiler's three actual private input files have the same bytes
  as A; B's marker and proof function are absent.
- A fresh consumer reuses that protected entry as machine-local evidence and
  completes the project with the original verdicts and establishing run.

Every one of the 366 literal cold solver input streams matches, including
multiplicity, between old clean A, new clean A and protected substitution.
The original native input hashes match across all four pre-substitution arms.
All retained unit payload hashes verify. Source checks pass on 603 new and 599
old tracked paths; both pairs of compiled binaries and the runtime remain
unchanged. The stored dep-info names no private native-copy paths in this shape;
ordinary unchanged-run acceptance remains separate. The native host is terminated.

Raw archive SHA-256:
`42628f6a3636ae78a29cef28381add07d74755d67a8288ec9b633f3382317c88`.
Independent retained component review:
`73dcad690bd07e51369bcb24081efd923f9f80c75ad913300d96b9fb37c2322f`.
This qualifies the controlled Linux input-consumption fix, not whole-corpus,
macOS runtime, performance or default-mode acceptance. The earlier failed
canary and argument-format attempts above remain failed.

- `88a30630` complete Mac host run: clippy passes; 686 tests pass and one
  fails because the removed corruption warning remains in the active error-code
  registry. The successor removes that unused registry entry and records its
  retirement in the changelog; the registry check remains strict.

## Native producer consumption: successor, unqualified

The schema-8 private-copy implementation at `ef918024` completes three native
fresh-project pairs on c7i.4xlarge in us-east-1: without reuse
38.462449 / 38.296493 / 38.229097 s; with reuse
9.349908 / 9.368559 / 9.307884 s. Medians are 38.296493 s and 9.349908 s
(4.096×). The fill takes 39.521885 s; an unchanged same-project run takes
1.171538 s with every receipt reused. All pairs have matching complete current
function verdicts and empty diagnostics, retaining the actual establishing run.
These compare experimental Sail with and without boundary reuse, not stock.
Workspace, solver, all local E2E phases, PR conformance and FAST complete with
zero differences; STANDARD and final source/runtime checks are still running.
No final archive or complete acceptance claim follows yet.

A subsequent combined source review finds two remaining producer boundaries:
a native source or transitive native library can be replaced during compilation
and restored before endpoint checks. This is distinct from the passing direct
vstd input-substitution control. Schema 9 requires the new native compiler
protocol from fork `067d0d7c`, independently copied complete native dependency
graphs and qualified current-run producer records. It remains disabled by
default; the preceding measurements do not qualify this successor.

The pinned Rust compiler API witness compiles the exact observer modules and
ordinary callback wrapper, with only the unrelated source-base configurator
stubbed. Four module tests pass. Four actual compiler controls show unchanged
unobserved/observed library bytes, retained source B checksums after restoration
to A, actual child-PID binding, and no completed document after a failed compile.
A separate actual `include_str!` control shows README in both source-map and
extra-tracked-file inventories with its retained checksum. Three equivalent
Rust feature-cfg spellings really select the same branch; admission now refuses
noncanonical spellings instead of ignoring them. These are API/source controls,
not a genuine full-fork build or real-project acceptance.

Failed source attempts remain visible: the first API witness build omitted
`rustc_private` and its dependent launch was unrun; the first expanded consumer
clippy run failed the test-length lint; an all-tests-in-one-process Mac run
passed 227 tests but the interrupted-engine fallback test returned ENOENT with
unknown cause. That exact fallback test passes separately under both original
and short temporary paths. The complete Mac host gate then passes clippy and all 693 enabled tests,
with 35 skipped, using short `TMPDIR`. The isolated failure's cause remains
unknown; no check or expected output was weakened. The retained source-check
index SHA-256 is `b9526835096e8facffb6c5840b8ff04df7fdaa4a646c9f2a727cde4db73fd6f9`.
Full runtime/native mutation controls, default parity and both-platform runtime
qualification remain open.
