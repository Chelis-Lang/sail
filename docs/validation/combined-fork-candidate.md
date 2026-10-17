# Combined deterministic fork candidate

**Status: built and qualified in the scopes below; matching fork runtime
prerequisites are published, while Sail selection remains pending.** The combined `8a373f81a507b7ef4f144767ccd237d3d3de6c48`
runtime has subsequent passing Linux source-suite, determinism, STANDARD and
default non-Singular DEEP evidence. A matching normal macOS runtime, PR subset
and private bundled-install smoke also pass. The earlier full-suite failure
remains indexed; it is not the latest result. Current-main re-pin integration
and staged delivery remain required before selection. Sail's accepted pins,
repository visibility and draft release remain unchanged.

## Identity and measured scope

The composite contains stable definition-path closure names (`8d03ab54`),
deterministic imported-closure tie ordering (`3f7f6c51`), opt-in prebuilt-source
path mapping (`b4450da9`) and ordered proof-note JSON serialization (`5793253c`).
It contains no M8 frontend memo, pruning experiment or transcript-completion
footer. Source review covers the full 12-file diff from accepted fork
`73dea02112d730e7ab52f58158012d440fb89512`; imported identical trait-default
closures are exercised in the genuine passing source suite. Rustc 1.98.1 seeds
its local definition hash with `StableCrateId`; its within-crate collision check
and the two-crate fixture are not a global 128-bit uniqueness theorem.

Both platforms bind upstream `426d8b01e7ffb910a36c15e1f870ff985bb61eef`,
Rust 1.98.1 and source lock
`80c71fdd00ef5073cd93edf6b5e37a635f7b4114fba8f6553ba2125eecb1395e`.
The pinned Z3 version is 4.16.0, with distinct platform binaries: Linux SHA-256
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`,
macOS SHA-256
`edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb`.
Linux uses native c7i.8xlarge in us-east-1; shared developer macOS checks are
correctness checks, not clean performance measurements.

The existing Vest incidental-edit timing has 90 actual samples: three arms,
three rounds and ten cases, with 45 edit/restoration events. The helper-insert
median changes from 36.217 s with accepted fork 73 to 25.811 s with the composite
(28.7% lower); plain stock takes 39.232 s. Other unchanged/comment/blank/move
medians stay around 25.3–25.6 s; local rename is 25.982/26.131 s and cold is
72.407/72.839 s for old/new. This is the composite's observed Vest gain, not an
isolated closure-only gain or a universal speedup. Its restored Sail executable
is bound to source `168ab7ab`, not current main. All timing rows retain actual
exit/verdict evidence. The separate 60-row solver-work stage failed on empty
producer log placeholders and remains unqualified; timing/session counters are
not exact re-solved-check counts. The measured compiler change can be reviewed separately from the outstanding
work-count and wider catalogue deliverables.

## Best completed gates

| Gate | Actual source and result | Material limit |
|---|---|---|
| Linux normal builds | `b9816738`: engine 13 artifacts/0 tests, distinct suite 13 artifacts/154 hashed test executables; 2,060 vstd functions/0 errors | Build/private packaging, not application acceptance |
| Linux BASIC | `af339f8a`: gate 644/35 skipped, solver 35, all seven E2E phases plus extra, four PR arms 33 Same/311 functions, exact proof-note repetitions and selector controls | Private matching runtime overrides; actual source predates current main |
| Full FAST | `6880cb9b`: four arms, each 147 Same/3,450 functions, zero Different/Unstable/infrastructure/drift/skips | Earlier exact source; not relabelled as an `af339f8a` execution |
| Genuine fork source suite | `af339f8a`: 4,709 Same, 58 ignored, 4,651 actual passing nonignored tests; zero failed/unknown/Different/Unstable/infrastructure | Proper separate suite; baseline must actually pass, identical failures cannot qualify |
| Determinism | `af339f8a`: all 147 FAST units, three runs each, 577 session buckets, zero varying buckets/failures | Exact solver-input buckets; volatile timing stdout hashes are retained, not claimed identical |
| STANDARD | `af339f8a`: four arms, each 159 Same/12,726 functions, zero Different/Unstable/infrastructure/skips | One or two units per arm have recorded rlimit drift under existing policy |
| DEEP | `af339f8a`: four arms, each 159 Same/12,726 functions, zero Different/Unstable/infrastructure | Default non-Singular scope: two optional Singular-required project skips per arm; recorded rlimit drift |
| macOS normal build | `af339f8a`: normal xtask builder, 14 runtime artifacts, 2,060 vstd functions/0 errors; source/runtime unchanged | Matching local Git mirror used through private build configuration; no performance claim |
| macOS engine/cancel | Exact 8a runtime: unchanged 11 engine fixtures and cancellation/no-seal/restart controls pass | Separate shared-Mac correctness evidence |
| macOS PR subset | `af339f8a`: four arms, each 33 Same/311 functions; zero Different/Unstable/infrastructure/drift/skips | Explicit matching local runtime, not current-main full tiers |
| macOS delivery | Normal pack/install, release stage and unchanged release-smoke pass; empty cache, no runtime override, genuine demo/edit/repair accepted. Literal assertion diagnostics then match stock exactly, both exit 101; final source/runtime/four-PR guards pass | Private local archives only; no publication or rollout; an overwritten first checker snapshot is not cited as passing evidence |

The genuine Linux engine manifest is
`0dbd3caa928d1e5b3d830c90380fdf77ca0401c90124ad12cade1ba3030db7a4`;
the suite manifest is
`a765909b9286eca63d8fc67e37ee3aa3180e15b2af2bba8115ee736ae8b97e90`.
The actual Linux packer is source `b9816738`, SHA-256
`4378cbe5dd4e86fec9fb585ea98960c18cda1c51715abfbdbc318f6eba7d9827`,
with equal source observations immediately before/after packaging. Its private
schema-2 Linux pin JSON hash is
`b084ceffe9bb64c9bf9062d5fb31b6c9603bed86297db6e53177c95e1b199d89`.
Both exact runtime archives are now available as the fork's immutable
`engine-runtime-8a373f81-linux` and `engine-runtime-8a373f81-macos`
prereleases. Anonymous downloads returned HTTP 200 and matched the archive
digests below. These prerequisites do not select Sail's runtime or publish a
Sail product release. The matched macOS manifest is
`8de0f9d879084a633b56c07478e519026298e16e01f1f79772e99a5ac8faf51c`.

Raw evidence stays external under `combined-8a-native-candidate-2026-10-16`
and `combined-8a-macos-2026-10-16`:

| Artifact | SHA-256 |
|---|---|
| Linux engine runtime, `native-result/engine-runtime-8a373f81-linux.tar.gz` | `6f7b30091efb80185fe2b2af15bbb3f4faf25f6daaafd7da59f59384adfa736c` |
| Linux suite runtime/test executables | `a3d62ea3dd0433ac9b540cdd1a52063f69c8f2d9e227fa1d6cd4f54af366729d` |
| Linux original build/package/source observations | `565de9e5af9cfc24e310a41663b3e412e5c0218591c62c45d09d5d112a9efb7b` |
| Latest Linux BASIC final archive | `dd39093b23b8c970c687db18014edd90d4c41e8baadb2bd22365ab26fb86e569` |
| Earlier full FAST final archive | `1fc047a94bebc33b44d063c593d044815da646df1ca28c5533af43f0b9c79fd6` |
| Genuine passing full fork source suite | `b2304b6dd70355fd69a0ad6241c3f2a00c25156e8443ffb357b80005ce165faf` |
| Passing 147×3 determinism final archive | `68b4b9ecf56dba75780e721e1a8ff41b1ed1d087752b7b4e402bd661c7b382b0` |
| STANDARD final archive | `8c82fb9faaf8e44611123d802b4efc8ea72f4239ad4c1f2c66e142b3346f66a1` |
| Default non-Singular DEEP final archive | `5451a898744189378eeceedf0e7e1794ad6cddc01e576bf271c48db4199dcaa6` |
| Vest 90-row timing archive | `593d2b217f84e99439c230d6d38f2dc0511d63b107a1234b9e1719ab9099700f` |
| macOS normal build result/source observations | `a54c0d2a03433ffbdf3b8e617eebbdb762b6eb24e528aac3f4dbf204368875a2` |
| macOS engine/cancel result | `3a63d4b03180072b2c8ad16bb1ae96f6d096a447da9e953f40fb99590b66b165` |
| macOS PR result | `83f0f0b5498ea33dc1ba59e3b922ca5abf1fdb0ef080687b7bf4b41769d85792` |
| macOS runtime archive | `1f792b588f2c4e2d277aaf46336d3e00a18639a4fb15d5cff4bee6c3919e31bc` |
| macOS prospective pin | `550b903f42d3660bdd25d4e4d6df5553a120e7794e4a0cf57feed2ca93b67a48` |
| macOS private staged Sail archive | `756904aa0b93da6a33c5356355603339f9ffc3c0bd689b65b5d77ae538711ef6` |
| macOS pack/install/default bundle smoke result | `661e7937f52c9a8dafbf22930b332cac24079ecfc0ed0836ebfb62f7acaa88df` |
| macOS final diagnostics/source/runtime/PR guards | `84de6458fce8fd3886f689234840f8e60384592771ddc6e8f0938c243eca0cb3` |

## Remaining delivery and selection gates

1. Bind a current-main implementation candidate to this exact reviewed fork,
   source lock and both actual runtime archives. Review both prospective pins,
   source/release metadata, ADR 0049 status and source-mapping scope together;
   imported VIR and vstd artifacts must match the new closure representation.
   Run markers/links and confirm no unrelated source changes.
2. On native Linux, validate/install the retained matching runtime with the
   current packer, then stage an empty-cache bundled default CLI smoke with no
   runtime override. Run the current joined workspace gate, solver, every E2E
   phase plus extra, real changed-linker fail-closed control and PR subset.
   Explicitly map old qualified source/binaries to joined code; repeat FAST and
   STANDARD on changed binary/behavior rather than silently applying old results.
   Recheck matching macOS default delivery if joined product code changes.
3. Review the measured composite gain and every limitation above. Obtain all
   required green checks on the exact current branch before lead selection.
   The matching runtime assets are available for the unchanged image installer;
   the fork default-branch fast-forward, immutable pin tag and Sail re-pin
   selection remain pending that review. No 0.1.0 publication or
   repository-visibility change is authorized here.

These gates deliver the measured deterministic compiler improvement. The wider
three-project [incidental catalogue](incidental-edits.md), exact solver-work
counts, shipped-vstd design and any new performance claim remain separate.
The existing native-linker defect [#187](https://github.com/Chelis-Lang/sail/issues/187)
remains visible; this fork does not fix it. The separately qualifying fail-closed
containment must be composed explicitly, not assumed from old runtime evidence.

## Failed and interrupted attempts retained

| Source/date | Actual result and limit |
|---|---|
| `b9816738`, 2026-10-16 archive collection | Initial host-user Git ownership refusal, exit 128; retained exact-path read-only retry, no build/source change |
| `05674c60`, 2026-10-16 BASIC | Two fixture failures (642 passed) under explicit runtime overrides; no assertion changed. Archive `b7983611d6ef77191a50d26cf8c14dd64afce1dce89e5c6d01fecf798643070a` |
| `05674c60`, corrected BASIC | Interrupted after host expiry; no completed gate/corpus claim. Index `05a0e31b89616d7b9fa5e6be58f977f16432c020ca6ee7d14762c9600df1713a` |
| `6880cb9b`, source-JSON | Refused explicit-suite selection with SAIL3040 before tests. Archive `6fbbb1f78bd96633654e40f7e40be20247590ceea6d767b4208062a67714962a` |
| `168ab7ab`, full fork suite | 4,708 Same/1 Different: duplicated progress option made both original regression arms fail, which stayed a strict failure. Archive `c4e3826df19353f4935ad4d2b80689d2b1c754f61233f9b9aa15d1026608624a`; later source-bound correction passed the genuine full suite |
| `af339f8a`, first determinism stage | Exit 1 before qualification; retained failed archive `eec84add58f8e7bc103ec3c5e940019fa976d5cf39623415ba016dbd48971fc2`. Later exact runtime/input witness with source-bound oracle correction passed; no silent reclassification |
| `c0e7e0f5`, independent stock suite | Strict stock control 4,691 Same/3 Unstable/1 Different; not a qualified control. See [deep-tier record](deep-tier.md) |
| `168ab7ab`, Vest solver-work stage | Markerless empty logs refused; all-hit solver-work acceptance remains pending despite separate successful timing rows |
| macOS delivery checker, 2026-10-17, first diagnostic attempt | Local checker expected exit 1 while actual Cargo failure is 101; no product disagreement. Actual smoke script/log and repair JSON retained; initial passing snapshot overwritten by the next checker is not a separate passing capture |
| macOS delivery checker, next attempt | Incorrect stock path refused locally; corrected final diagnostic check matches actual stock bytes and exit 101 |
