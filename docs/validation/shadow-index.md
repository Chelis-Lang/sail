# Erasure shadow-check runtime

Fork `36d65c3f` ([PR #11](https://github.com/Chelis-Lang/verus/pull/11))
indexes erasure shadow checks once and uses only membership lookups. Traversal,
hints and solver input bytes are unchanged. There is no persistent frontend
reuse or query normalization. Linux and Mac runtime pins and their separately
qualified retained-proof manifests are selected together. Native and installed
functional qualification is complete. Overall performance acceptance remains
open because unchanged-call overhead and slower individual rows remain.

## Measured gain

A native c7i.8xlarge in us-east-1 profiled cached Anvil controller edits on
2026-10-24. Erasure setup was the largest individual self-time symbol (6.44%).
Parent `7d9d5410` and final candidate `36d65c3f` use Rust 1.99.1, identical
build configuration and pinned Z3. Six alternating Linux pairs verify 144
checks in 38 cached sessions each, with no solver starts; complete solver-input
hash multisets agree in every pair. Three rotated broader rounds retain all
144 Linux and 180 Mac positive/negative calls with matching exits and counts.

| Direct compiler comparison | Parent median | Candidate median |
|---|---:|---:|
| Linux cached Anvil | 33.549 s | 29.990 s |
| Mac cached Anvil | 13.694 s | 11.862 s |
| Mac Anvil body edit | 39.918 s | 38.204 s |
| Mac Vest body edit | 11.655 s | 11.530 s |

The Anvil reductions are 10.6% and 13.4%. Mac controls cover all five real
projects; Linux covers four. These calls qualify the compiler optimization,
rather than ordinary managed unchanged-call performance.

## Functional qualification

- Official Linux source runtime: all four FAST arms pass 147 Same / 3,451
  functions; STANDARD arms pass 159 Same / 13,166 functions. Deep passes
  all four mandatory binary corpus arms (159 Same / 13,166 functions each)
  and all 217 mutation/control comparisons. Optional Singular projects are
  outside these binary runtimes. Linux fixtures and cancellation/restart pass.
- Full fork source suite, direct/cache-fill/cache-warm: each 4,901 Same,
  with all 4,838 nonignored tests passing and 63 ignored. The original stock
  control retains four Unstable and one Different debug-map-order cases
  (VI-0008/VI-0021). Fresh actual stock controls reproduce that additional
  Sail output exactly; the existing classifier returns Unstable over the
  retained controls. No normalization or checker change is made.
- Mac official engine fixtures, cancellation/restart, changed library suites
  and all 18 fork/basic targets pass. All 320 solver streams are identical
  across three runs. Strict output remains 319/320 because the known
  `imo_1988_6` resource count varies from 82,318 to 82,320 with unchanged
  query buckets; see the [existing reproduction](../../corpus/repros/z3-nla-rlimit-nondeterminism/README.md).
- Both immutable fork runtime archives validate on anonymous download and
  both ordinary Sail archive factories pass. The frozen consumer passes full
  FAST and STANDARD, including the 21 bounded mutation/control comparisons.
- Latest cumulative source: main `fbd5d682` plus this runtime selection,
  code-qualified as `be573d46`; later changes are documentation only. On a
  native c7i.4xlarge in us-east-1 using the pinned MCP CI image, workspace
  1,066 tests, solver 42, full E2E plus extra and every PR arm pass
  (33 Same / 311 functions each). The arm64 macOS 26.5 host gate passes
  all 1,055 tests with Clippy clean.
- The newly merged scalar-witness feature also passes its actual source-owner
  RPC and configured pinned-runtime fixture: twelve cases, sixteen SAT
  assignments independently validated and eight UNSAT controls. This is
  bounded fixture compatibility, not native mutation-kill acceptance or
  qualification of another release archive.
- Passing public Vest controls on both platforms serve the shipped limit-100
  proof, verify an edit and restore the original workspace's own retained
  proof. Revert and unchanged invoke zero verifiers and remain Current and
  complete. These controls keep the ordinary corpus settings unchanged.

## Installed CLI matrix

Both factories stage Sail `fc7cd0c7` with the exact runtime pins and qualified
retained-proof manifests in this change. Each platform completes all 27
scripted scenarios in three rounds (162 samples). The installed binaries stay
frozen for their mutation cohorts. Later opt-in contract-profile changes on
main are checked separately by the cumulative integration gate; the timings
identify these frozen binaries rather than a later main build.

| Scripted scenario, stock to Sail medians | Linux | Mac |
|---|---:|---:|
| Small project first use | 33.170 to 13.215 s | 16.847 to 10.081 s |
| Anvil cold | 186.133 to 172.988 s | 99.876 to 100.734 s |
| Anvil body edit | 153.623 to 85.890 s | 79.433 to 41.113 s |
| Anvil spec edit | 153.056 to 33.283 s | 79.469 to 14.346 s |
| Vest cold | 74.849 to 71.548 s | 38.879 to 38.940 s |
| Anvil unchanged, no verifier invoked | 0.331 to 1.948 s | 0.283 to 1.827 s |

All Linux cold medians improve. Mac Anvil adds 0.858 s on a roughly
100-second run; Vest adds 0.061 s. Unchanged and tiny-edit overhead remains
visible: eight scripted Linux rows and ten Mac rows require review. A faster
compiler does not remove managed setup cost. No fixed percentage allowance
or implicit performance waiver is applied. Vest's mutation catalogue retains
the known default-limit failure in both arms; its passing limit-100 controls
are separate. Edit-loop qualification covers
four complete mutation cohorts on each platform. Those cohorts pass all
651 comparisons on each platform, with no false verified, different,
unstable or infrastructure outcomes. Each round retains one daemon and both
returns receive the original's own answers.

| Complete mutation loop | Linux speedup | Mac speedup |
|---|---:|---:|
| HumanEval | 2.71× | 2.87× |
| IronKV | 1.47× | 1.42× |
| Vest | 1.11× | 1.40× |
| CapybaraKV | 2.04× | 2.37× |

Individual slower edits remain explicit: one HumanEval and three IronKV
medians on Linux; one HumanEval and one IronKV median on Mac. Mac has 12
matrix rows requiring review, including ten scripted rows. Linux has ten rows requiring review, including eight scripted rows. Both
reports validate all 31 comparisons; a valid comparison does not accept a
performance loss.

The Mac install also passes with its source SDK moved aside: a fresh project
uses the released vstd proof, verifies its requested root and returns Current
and complete. Its unchanged call starts zero verifiers and reuses both
reports. Provenance is checked through the documented `dependency_build`
field, separately from ordinary report reuse.

## Raw records

Objects below are in the private evidence bucket under
`experiments/2026-10-24/`.

| Record | SHA-256 |
|---|---|
| `shadow-index-broader-linux-final.tgz` | `c1ea2addac0ce2182d8566c1a06f77ccc38d4645e0f5cf2f62f62593013ffbfa` |
| `shadow-index-broader-macos-final.tgz` | `6f5ab5b0292cb92978de1d96271cb80e783bc57d650609c7553d5eefa6b7625a` |
| `shadow-index-source-rvt-final.tgz` | `d78c4db3bf025c53a4b8a420528de26d5db97a9c3a82880e1872a72e59525953` |
| `shadow-index-consumer-gates-final.tgz` | `d1869546a5013900aa8665b66339da7922794f07e1ad3049fe84b0bf9d1dc530` |
| `sprint-final-main-native-gates.tgz` | `35295e8c1732a9ddcea5a9ae9224d46cb0baebba28bbcf28e9e7decc48ecfd2d` |
| `sprint-cumulative-native-gates-final.tgz` | `4ceb90e620e303f9712878c88363a0b64a1aed5e8f84f11b328ca24611756c72` |
| `shadow-index-macos-product-final.tgz` | `5d88653539967cb39febeb4bd778c075866d0351e8a1df5c81b7b7a0e4aa4116` |
| `shadow-index-macos-source-qualification-final.tgz` | `15c774ed2906a0af9b8977205ac82f78ba1d121276ce030a2dc2476ae27e2f19` |
| `shadow-index-linux-matrix-rerun-final.tgz` | `6ba663364b185a73265ee832e375836d5291378fa6aac00798b271fee83f57c4` |
| `shadow-index-linux-vest-final.tgz` | `46a4c6580edc59f32ef05a89c32a3ffd177935867a279fc55765c842e8e3c6f1` |
| `shadow-index-linux-human-eval-rerun-final.tgz` | `7b926ff8d8d3d638f6f0eab2425fccdd96804a3ee69561ee17df41356c473922` |
| `shadow-index-linux-capybarakv-final.tgz` | `e25430d2bdb99636120b7dcb2d029096722cb495f8c89121e3c3383d6cf2969a` |
| `shadow-index-linux-deep-ironkv-final.tgz` | `6659080b4a4f49694fa3b490ab89f12326c68c0984d6d611944c6c56a540c54b` |
| `sprint-final-main-macos-host.log` | `b9a85eb6ae59056163e56fef7973dd1d8b935d6e8971b239f860e52c2b9a0fd7` |
| `sprint-final-main-macos-watch-first.log` | `b164a552a1af81dca0059bb02deb31f6d422503130c1648244fdfcacb20e71d4` |
| `sprint-integration-macos-host.log` | `a70de7b536345919c5ee91035dcb2dcd3dbfbaa241b39632e5296262e3e53479` |

Indexed attempts, 2026-10-24–25:

- `d5929b81`: fork CI rejects shifted debug-message locations; final
  `36d65c3f` preserves them and passes, without weakening the guard.
- Initial AIR library run exits 128, cause unknown; explicit pinned-Z3 retry
  passes all 144 tests.
- Broader post-check expected failed-proof Cargo exit 1; both arms actually
  return 101. Correcting that temporary expectation passes the retained calls.
- Deep setup refuses two empty Cargo lock files left by source library builds;
  removing those regenerable files restores the exact manifest inventory,
  with every bound runtime digest matching.
- Initial cumulative copies omit documentation and are refused; the complete
  Git merge passes. The first local Mac run uses a shared target override and
  fails three fixtures; ordinary Cargo target selection passes all 1,045.

- The first Linux installed matrix and HumanEval raw records were lost after
  export failed. Replacement runs use the same immutable installed build;
  only those complete replacement records qualify the Linux result.
- Initial Mac install post-check expects a nonexistent `vstd_proof` field;
  checking the saved successful commands against the documented
  `dependency_build` provenance passes both cold and unchanged controls.
- After integrating `fbd5d682`, the first Mac gate sees an extra restart in
  the fake-Cargo unbound watch fixture. The unchanged isolated test and full
  host retry pass (1,055 tests). Cause unknown; the observation and original
  failed log remain linked in #306. No assertion or timeout changed.
- The extra scalar-witness probe initially runs an uninstalled source binary
  without a configured runtime and correctly returns Unavailable. Selecting
  the validated pinned runtime through `SAIL_VERUS_BUILD` passes all twelve
  RPC cases; the original attempt remains in the same raw record.
