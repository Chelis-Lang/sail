# vstd compiler-boundary replacement

| Gate | Current state |
|---|---|
| Implementation | Real Cargo compiler-wrapper prototype; requires `SAIL_VSTD_BOUNDARY_RESEARCH=1` |
| Native prerequisites | Every native compiler executes normally; current-run successful provenance and actual linked bytes required |
| Reuse scope | Pinned, nonprimary vstd library only; no native units or Cargo fingerprints |
| Identity | Literal flags, actual runtime/sysroot/solver bytes, linked libraries/VIR, official source/feature closure and environment |
| Provenance | Complete successful proof required; typed origin and actual establishing invocation survive receipt reuse |
| Native performance | `50b116bb`: three fresh-project pairs, median 37.871 s without reuse versus 8.954 s with reuse (4.23×); experimental |
| Compiler-input boundary | New source checks compiler-retained vstd source checksums; full execution and macro/artifact mutation qualification remains open |
| Literal query comparison | Unqualified: two control compilations completed, but the external collector captured no traces |
| Selection | Disabled by default; neither release nor default replacement qualified; v1 remains quarantined |

## Product boundary

`SAIL_SHIPPED_RECORD` records an experimental bundle; `SAIL_SHIPPED_DEPS`
serves one. `SAIL_VSTD_BOUNDARY_LOCAL` instead uses a separate machine-local
bundle bound to both literal and canonical registry roots. All require the
research flag. Local mode never remaps VIR or source paths. Schema 6 and the
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

On c7i.4xlarge, us-east-1, Sail source `50b116bb`, normal cc7 runtime archive
`717680b4cb126f48397b02f32dcbf54e0f2a320fa0c16c86a97237a8c3eee8b1`,
Rust 1.99.0 for Sail and 1.98.1 for Verus, independent physical projects used
identical manifest, lock and source bytes with separate targets and proof caches.
These are Sail-with-reuse versus Sail-without-boundary-reuse measurements,
not a completed stock comparison.

| Round | Without boundary reuse | Fresh project with boundary reuse |
|---|---:|---:|
| 1 | 37.871 s | 8.948 s |
| 2 | 37.838 s | 8.954 s |
| 3 | 38.163 s | 9.047 s |

The initial fill took 39.752 s. Each fresh project compiled native prerequisites
and used one verifier invocation instead of two. Reused vstd retains actual fill
invocation `15973-1792240532331088455` and machine-local provenance. Replacing
the private native `cc` executable at the same path still failed with Cargo 101
in 1.839 s, before any verifier invocation. Summary SHA-256:
`eee54507754ad10346efbff41858bb5475c2f75d7f2ce25eedaf0194fd0a556d`.

The enclosing screen failed after all ten application rows: the two extra
trace-control compilations completed, but the external collector found zero
input files because the CLI sets its own trace directory. Query equality is
therefore unqualified. Original failed output is preserved. Timing observations
do not qualify the input model, stock verdict/diagnostic parity or default use.

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

Publication now requires every listed compiler source to be an admitted vstd
Rust file with exactly one matching compiler checksum and size. Missing,
foreign, duplicate, malformed and differently hashed inputs refuse publication.
Restoring a source file after the compiler read cannot restore the checksum
retained by that compiler. The existing whole-source SHA-256 and file-set checks
remain separate checks.

This is a narrow source-input check. Rustc separately reopens extra tracked
proc-macro, PGO and debugger files at dep-info emission; those checksums are
not evidence of their earlier reads. The prototype must not generalize this
source-map argument to arbitrary macro inputs, external artifacts or mapped
native libraries. Complete native qualification, source mutation controls,
macro/compiler builtin/span/diagnostic audit, full verdict and diagnostic parity,
and both-platform checks remain required before selection. Exported VIR alone
is not a complete proof-input key because Verus prunes private bodies.

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
- `50b116bb`: native lint and 24 focused tests passed; three actual reuse pairs completed; enclosing trace-collection gate failed as described above. Mac lint, 24 focused tests and receipt-origin test passed.
