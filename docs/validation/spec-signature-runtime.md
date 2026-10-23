# Signature-sized spec-formal runtime qualification

Status: both rebuilt runtimes pass backend and installed-consumer functional
qualification, including all four complete mutation cohorts on each platform.
The matrices are complete; Mac cold and unchanged overhead keep performance
acceptance open. The previous failed Mac cohort remains failed. No rc.6 draft
has been created.

Source: reviewed fork `7d9d5410159a7baa2c74868ce1f591678788eb23`, whose complete
source tree equals squash merge `a6240baab374c6d687e919730c957a0a8512943f`
([fork PR 10](https://github.com/Chelis-Lang/verus/pull/10)). It retains upstream
Verus `7d829946231d6413a398f6e4160cdfe951261dad`, Rust 1.99.1 and the previous
source lock. [VI-0034](../verus-issues/VI-0034-spec-formal-solver-context.md)
records the cause, reproduction and limits of smaller reductions. No default
query normalization, solver flags or accepted verdict exceptions change.

| Gate | Result |
|---|---|
| Original full-crate Mac HumanEval mutation | Stock and rebuilt compiler both verify; all 331 function statuses equal |
| Native Linux source build and release-style vstd | Pass; 2,059 verified, zero errors |
| Native Linux rust_verify/VIR/AIR libraries, every fork regression and basic | Pass |
| Native Mac libraries and fork regressions | Pass: 18/13/144 library tests; all 18 fork/basic targets |
| Engine and cancellation fixtures, both platforms | Pass |
| Mac determinism, three runs over 320 direct inputs | All solver streams stable; strict output check: 319/320, one existing NLA resource-count variation on `imo_1988_6.rs` (82,316 vs 82,314), unchanged inputs and verdict |
| Native full fast corpus conformance | Pass: each arm 147 Same; engine arm 3,451 functions, zero different/unstable/infra |
| Native full standard backend corpus conformance | Pass in all four arms; consumer standard and its mutation subset pass |
| Ordinary retained reuse on both new runtime manifests | Pass: fresh successful public Vest, edit, original proof restored, unchanged; zero verifier invocations on restored proof |
| Consumer workspace/solver/full E2E and Mac host gate | Native workspace/solver/full E2E including extra pass; Mac clippy and all 963 host tests pass |
| Shipped exact proof/default-100 profiles and signed replies | Both platform factories pass; Mac independently records both exact profiles; actual public Vest limit-100 proof served on both |
| Full selected mutation cohorts | Pass on both platforms: HumanEval 165, IronKV 153, Vest 165 and CapybaraKV 168 Same each; zero false verified, different, unstable or infrastructure errors |
| Deep source suite and release matrix | Pass: deep corpus 159 Same in each of four arms; fork pass-through, cache-fill and cache-warm each 4,901 Same; both matrices 31/31 valid, performance acceptance open |

| Platform | Runtime manifest SHA-256 | Runtime archive SHA-256 |
|---|---|---|
| Linux x86-64, native EC2 c7i.8xlarge, Ubuntu 24.04 | `35fdb4942460887104955f409a7dd1745207de2abb829d89061b0ab93ce0b2dc` | `0e7d1130e304debd292170e8312015d8e5faf1a611071f478138145f6aa5f682` |
| macOS 27.0 arm64, native EC2 mac-m4.metal | `f1dcf457ad0b4fd66073db3462179828944bbda3176c088348a5851f43fd339b` | `01b37a321e3883f2cf4eabcc7dc25a01acecb525ab7d057c02b679d6d9c06161` |

Both archives have matching checksums, valid complete inventories and clean
build-host home-path scans. Platform SDK builds use isolated toolchains without
rust-src; consumer runs use the documented minimal toolchain with rust-src.
The ordinary-reuse selectors now pass the actual successful public Vest
controls on both platform manifests. These are functional controls; their
elapsed times are not a release benchmark. Installed mutation and matrix
results are recorded in the [release matrix](release-matrix.md).

Raw bounded Mac compiler controls:
`experiments/2026-10-23/human004-compiler-controls.tgz`, SHA-256
`2e7ead547b5991e2bb526bf43afa84722dd2f686c37f83d090f6b44f3dd6b5b0`.
Native full backend build/regression log:
`experiments/2026-10-23/signature-native-build.log`, SHA-256
`7006f159f192df22d6b66198f56d379a0524c58ea2244c7dee9282ddda7f670d`.

Installed Mac host/factory/retry/public-Vest and three-round HumanEval regression
controls: `experiments/2026-10-23/signature-mac-controls.tgz`, SHA-256
`c00c1e2b648d9ad22f4d079c8a5d5f3282c96f44db97acf31fd83f2c5969e8e5`.
Native Linux workspace, solver, full E2E, release/fast checks and public-Vest
control: `experiments/2026-10-23/signature-linux-controls.tgz`, SHA-256
`92910b2af5c4acac0590edc57a35d7511e9a75dd6cf0ddde40e597ec20a10357`.
The Linux capture precedes completion of full standard conformance. Complete
consumer gates and actual Linux archive/Claude Code MCP checks are in
`experiments/2026-10-23/signature-consumer-complete.tgz`, SHA-256
`870f63fc777ee341502af2c0aafdd074b88f37a04323a94f3568e7e05b9b4755`.

Completed three-round installed mutation cohorts: Mac HumanEval 165/165 and
IronKV 153/153 Same, with zero differences, unstable cases, false verified or
infrastructure errors. One daemon serves each round; ordinary and forced
returns get the original's own answers. Edit-loop speedups are 2.84×
[2.14–3.87] and 1.41× [1.34–1.50], respectively. Raw reports, headers and every
sample: `experiments/2026-10-23/signature-mac-human-iron.tgz`, SHA-256
`da7c8662a7e7c74248da61dc2e6b67b0378236342e3bb4674b662d518b725d4b`.
Linux IronKV likewise passes all 153 comparisons, with a 1.48× [1.38–1.59]
edit loop: `experiments/2026-10-23/signature-linux-ironkv.tgz`, SHA-256
`ea81442700e2a838ae6b1588d0128b9e78755313eaeb99805fe7b3a3e628ecf2`.

The full fork source-suite pass-through has 4,901 Same, 63 ignored, 4,838
nonignored tests passing, zero differences or infrastructure errors. The
stock-only control retains VI-0008/VI-0021 JSON debug-order instability
(4 Unstable and 1 Different); its two failing libtests match on both sides.
That command stopped before the fork cache rounds. Separate unchanged
cache-fill and cache-warm runs each pass all 4,901 comparisons; all 4,838
nonignored fork tests pass. Raw source-suite reports and logs:
`experiments/2026-10-23/signature-deep-rvt.tgz`, SHA-256
`5f964efc198b720d6ecec7b5df131acbeefdb958fbbf9611ad197f906d985bd6`.

Complete backend deep corpus, cache-fill/cache-warm and source-build logs:
`experiments/2026-10-23/signature-backend-qualification.tgz`, SHA-256
`1fec6da9529a2d56c7f60c7a59bdc80eae8c6f898073a387d49dd9292bc45e9a`.
The two optional Singular projects are excluded from this binary-runtime
qualification. Stock-only debug-order failures remain recorded separately.
