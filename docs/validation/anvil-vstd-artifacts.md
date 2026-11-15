# Anvil vstd compiler artifacts

Status: **selected-runtime prerequisite failed; experimental bridge passes**.
VI-0043's VIR producer correction is unselected. The registry macro remains a
source of native metadata variation; explicit bundled-macro experiments remove
it. Production routing has not changed.
No shipped focus reuse, speedup, Mac acceptance or #232 completion is established.

## Source, runtime and environment

Sail main `e612d881c3d1b139bd59f80632efbda7e77620c4`; Anvil
`a597f9de7dbeb056403bd60b0a2ca67f686a5e1f`, focused
`vdeployment_controller`; native x86-64 EC2 c7i.4xlarge, us-east-2c; CI core
image `6c19f5c8d397b61c`. Registry preparation uses the normal release-pinning
helper and preserves Anvil's Rust 1.98.0 project toolchain. Selected Verus is
fork `a0e11cffbdf768eac874996f35149385152f736e`, manifest
`1d23c074efd3335a60f3ca24f97ab5eb130fafd2454a06f5fc9d279da188c03c`.
The first Rust compiler argument is a discarded Cargo compatibility hint;
verification and compilation run inside the selected `rust_verify` process.

## Actual result and limits

The full focused baseline exits zero, with unchanged Cargo manifest, lock and
project toolchain. Passive sampling records the actual vstd compiler's native
argv, environment and cwd. `--no-verify` resides in the package-specific packed
environment arguments. Exact-argument replay fails its original-artifact
control before reaching the verifying comparison. A separate three-run
same-mode diagnostic produces varying VIR and Rust metadata bytes. Captured
argv/env values do not establish complete filesystem, inherited descriptor or
process-state identity; no stronger process-input equivalence is claimed.

Independent retained-byte analysis locates the VIR difference in its final
144-entry source-map serialization: preceding VIR bytes, crate ID, every key,
optional filename and start/end position match; only complete entry order
differs. Re-encoding each original order roundtrips exactly. This diagnostic
analysis does not normalize artifacts or waive their failed byte comparison.

The [small direct fixture](../../corpus/repros/verus-export-metadata-order/README.md)
also reproduces the variation without Cargo. The preliminary corrected fork
source is `a3109bed` plus the serialization patch, with
`import_export.rs` SHA-256
`704bc0f808625e2d84b57f621b2d97c6f1fb2997cd65dbc1c358dfe0a58f2d5f`.
Its exact native package passes all 30 library tests. Full runtime construction
verifies 2,059 vstd functions with zero errors. The exact corpus fixture gives
eight different selected-runtime VIR files, then identical candidate Rust/VIR
bytes across eight compile-only and two verification-enabled runs. This fixture
contains only spec functions: those last two runs check zero proof obligations.
The vstd proof result is a separate actual verification run.

Eight direct Anvil vstd replays using that candidate executable produce one VIR
hash, `9425fcb9171f4e0197e343bb6bc0c9920d8e07875c8df86bf8b18a4a2a05ae6c`,
but seven distinct Rust metadata hashes. That same-mode gate still fails. The
subsequent macro controls below identify the macro-library input as a separate
control point; this original failed gate is preserved.

The final formatting-only source is committed in the fork as
`1a7e2f6cb1be88fefd382ce05bae4aae11b0e8a5`, file SHA-256
`982186d6b80ca32d44ea1f040203e7d82962391565ed97eb96fa290258275c5c`.
Exact committed-source native validation also passes all 30 library tests, a
full runtime build with 2,059 vstd proofs, and the same eight compile-only / two
verification-enabled fixture comparisons. Removing only the serializer annotation
in temporary native source makes the regression fail with an actual assertion
and exit 101; restoring the exact source restores the pass. Earlier runtime
results remain bound to their original source. No runtime pin changes.

## Controlled macro and verification bridge

A direct Rustc reduction using the actual registry libraries produces eight
metadata hashes in eight runs. Keeping the same registry builtin and vstd,
changing only the macro-library argument to the bundled fork macro produces one
hash in eight runs. The seven-field enum is retained with
[VI-0001](../verus-issues/VI-0001-nondeterministic-declaration-order.md). The first
reduction lacked its vstd extern and failed to compile; it is retained as an
invalid experiment, not a producer failure.

The actual captured Anvil vstd invocation then runs eight times per macro arm
with the final `1a7e2f6c` compiler fixed in both arms. Registry macro: six distinct
Rust metadata hashes and one VIR hash. Bundled macro: one Rust metadata hash and
one VIR hash. Only the macro-library argument changes across these arms. This
locates a repeatability control point; it does not select a production routing
policy or prove complete original process-input identity.

On that **explicit bundled-macro setup**, two compile-only control runs reproduce
the prior artifacts exactly. Two further runs remove exactly the package's
`--no-verify` token, keep all other argv/environment/cwd values, and each report
**2,060 verified, zero errors, entire crate, success**. Both produce exactly the
same Rust metadata and VIR as the compile-only controls: Rust metadata SHA-256
`c3b7d08c20265de1e5fb39eda678f04b1743576e6066257390706afa352f687d`,
VIR `9425fcb9171f4e0197e343bb6bc0c9920d8e07875c8df86bf8b18a4a2a05ae6c`.
Each verifying case retains 443 AIR files and 443 SMT transcripts, with 2,060
`check-sat` requests and 2,060 `unsat` responses, zero `sat`/`unknown`. The captured
solver path names Sail's built shim; this is the observed compiler/shim pipeline,
not a separate direct-stock-Z3 or conformance qualification.

Original compiler streams, counters, dep-info and owning AIR/SMT logs are retained;
this compares the two artifact types only, not identical compiler diagnostics or
all output files across verification modes. The fresh bounded jobserver preserves
descriptor numbers, not the original kernel pipe or token state.

All 2,140 macro-control/bridge files (1,293,353,156 raw bytes), including
original streams, artifact bodies, invalid setup, argv/environment values and
owning AIR/SMT logs, are independently lead-verified and externally retained:
archive `3f8b3ceccc044018b0798771f94efe323f717ec2d69442d84a044a64bc9588d9`,
index `3429c3eaecb4a65bb06f4c048b13f0772e6bbc8a01b3cb0df86f09b172a97515`.
Earlier library bodies are bound to their exact original complete-capture index;
subsequently overwritten live outputs are not substituted for those inputs.

A fresh independent audit **approves this bounded artifact-pair bridge** and
checks every new packet body, exact argv/environment changes, original failures,
reused input bodies, real compiler outcomes and solver transcript counts. Report
SHA-256 `04f19af54b2b0bd834ea1f15302d01cf83fbfeca826d77df2decc28ae3f207cf`.
It grants no production routing/reuse, full-process closure, direct-Z3,
conformance, performance, Mac, runtime-selection or release acceptance.

The experimental bridge passes without relaxing the original artifact checks.
It applies to a different, explicit compiler/macro setup. Original selected-runtime
failures remain failed. Source-bound routing, certificate/cache integration,
whole-project conformance, paired performance, Mac qualification and runtime
selection remain open.

## Retained evidence

All 285 files / 165,386,092 raw bytes of the closed preliminary package, runtime,
fixture and command captures are independently lead-verified and externally
retained: archive SHA-256
`cfdfe848e0bb3913ec61ec50fda6824004979380a95351f90f5b6019e600ba1a`,
index `66ba7814aebac61068088cc0d7e8175e240ac2bb3a50b13780d288a8a02ec030`.
The earlier independent Anvil artifact audit's 92 files and four symlink metadata
records are likewise verified and retained; report SHA-256
`183bc75cf853afaf74d7e1af2e6ad8cdcbb075eca3081e043057e9170bde9e5f`.
Complete available original preparation, project/cache/toolchain, failed replay,
compiler-output, source/build and final committed-source captures are also
independently lead-verified and externally retained: 54,970 files / 9,836,031,059
raw bytes, 10,758 directories and eight symlink metadata records; archive
`f6eacb870df38f244b65748bef6eca1a1f258f24c8603c6df83525e909cb11b9`,
index `80405f0b6ddbe8e3006bf7fbfb773a5a11b3cda85dfdd4e5adf3c8c0445da8a7`.
This includes all failed controls and both source revisions. System image inputs
remain bound to the recorded image identity; complete original process-state
closure is not claimed. The later macro reductions and Sail workspace gate
have separate capture packets. All 1,570 available gate source/output/binary
files (2,674,674,887 raw bytes) are independently verified and retained: archive
`62c569426295922fd8ccdbb12fab54ce689dd2eeb589db908fe6fbc1fe6c074e`,
index `680140fe264b39da3d1d70a73160b683d5c06bc469bc9d63bef8fe0cedea6bc2`.
Compiler intermediates and runner-deleted temporary files are not claimed retained.

| Attempt | Source/date | Result and cause |
|---|---|---|
| Preparation 1 | `e612d881`, 2026-11-15 | Invalid helper compile: called iterator method directly on a vector |
| Preparation 2 | same | Failed registry/Git transitive closure conflict |
| Preparation 3 | same | Failed Rustup post-install self-check after actual toolchain installation |
| Preparation 4 / focused baseline | same | Pass: actual project toolchain, aligned registry closure, full focused verification |
| Replay 1 | selected fork, same date | Strict original-artifact control fails; also lacks inherited jobserver descriptors |
| Replay 2 | selected fork, same date | Fresh jobserver at original descriptor numbers removes warning; strict control still fails |
| Diagnostic 3 | selected fork, same date | Three compiler exits zero; both artifact kinds vary; strict gate fails |
| First native regression command | preliminary corrected fork, same date | Invalid test target: binary compiled, zero tests executed; never counted as a test pass |
| Correct library regression / full library / runtime | preliminary corrected fork, same date | One targeted test, then all 30 library tests and 2,059 vstd proofs pass |
| Final committed-source negative control / package / runtime | `1a7e2f6c`, same date | Annotation removal fails the actual regression; restored source passes all 30 tests, 2,059 vstd proofs and ten stable fixture exports |
| Native metadata reduction 7 | actual registry libraries, same date | Invalid setup: omitted vstd extern; compiler rejects the fixture |
| Direct macro reductions 8 / 9 | final fork libraries, same date | Eight registry metadata hashes versus one bundled hash; reduction 9 changes only the macro library |
| Actual Anvil macro control 10 | `1a7e2f6c`, same date | Registry arm varies; bundled arm has eight identical Rust/VIR artifact pairs |
| Experimental bridge 11 | same | Two compile-only and two entire-vstd verification runs have identical Rust/VIR artifacts; actual verification count 2,060 each |
| Original capture collector 1 | exact committed fork, same date | Rejected two generated toolchain-manifest files as unexpected untracked files; collector 2 requires that exact generated-file set and retains their bodies; tracked source stays unchanged |
| Candidate Anvil diagnostic 5 | preliminary corrected fork, same date | Eight compiler exits zero; VIR stable, Rust metadata varies; overall strict gate fails |

Fresh independent review **approves the bounded correction**, including
serialization, legacy decoding, complete values, actual native originals and
formatting-only source equivalence. All 344 reviewer files are independently
lead-verified and externally retained; report SHA-256
`62a76f5a48d37c9f3158c957eff1106097f6fa124dd729fe47314a175be31158`.
The later exact-commit native run is separate from that review packet.

A separate native full Sail workspace gate passes 1,443 tests with 59 declared
skips on diagnostic source `5e9b71d1`; subsequent changes are documentation and
additional corpus reduction only. All workspace Rust, manifests and lockfiles
remain unchanged. The book source checks, build and local link checks pass.
No long-corpus run is claimed for this diagnostic-only Sail change. This
result does not qualify standard/deep conformance, performance, whole-project
reuse, Mac behavior or release selection.
