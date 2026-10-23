# Signature-sized spec-formal runtime qualification

Status: both runtimes are built. Backend fixtures and the original failing Mac
compiler case pass; complete consumer, mutation and release qualification is
pending. The previous Mac release cohort remains failed. No rc.6 draft has
been created and release acceptance remains open.

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
| Native full standard corpus conformance | Running |
| Ordinary retained reuse on both new runtime manifests | Pass: fresh successful public Vest, edit, original proof restored, unchanged; zero verifier invocations on restored proof |
| Consumer workspace/solver/full E2E and Mac host gate | Native workspace/solver/full E2E including extra pass; Mac clippy and all 963 host tests pass |
| Shipped exact proof/default-100 profiles and signed replies | Both platform factories pass; Mac independently records both exact profiles; actual public Vest limit-100 proof served on both |
| Full selected mutation cohorts, deep source suite and release matrix | Pending |

| Platform | Runtime manifest SHA-256 | Runtime archive SHA-256 |
|---|---|---|
| Linux x86-64, native EC2 c7i.8xlarge, Ubuntu 24.04 | `35fdb4942460887104955f409a7dd1745207de2abb829d89061b0ab93ce0b2dc` | `0e7d1130e304debd292170e8312015d8e5faf1a611071f478138145f6aa5f682` |
| macOS 27.0 arm64, native EC2 mac-m4.metal | `f1dcf457ad0b4fd66073db3462179828944bbda3176c088348a5851f43fd339b` | `01b37a321e3883f2cf4eabcc7dc25a01acecb525ab7d057c02b679d6d9c06161` |

Both archives have matching checksums, valid complete inventories and clean
build-host home-path scans. Platform SDK builds use isolated toolchains without
rust-src; consumer runs use the documented minimal toolchain with rust-src.
The ordinary-reuse selectors now pass the actual successful public Vest
controls on both platform manifests. These are functional controls; their
elapsed times are not a release benchmark. Full mutation and release checks
remain separate.

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
The Linux capture precedes completion of full standard conformance; that result
is recorded separately when it completes.
