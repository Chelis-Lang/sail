# Compiler-read origin Linux runtime construction

Status: the Linux candidate is built, locally packed and freshly installed.
It is **unselected**. The paired [compiler-read origin acceptance](compiler-read-origin.md),
ordinary useful reuse, full conformance, determinism and macOS qualification
remain pending. This record qualifies runtime construction only; it does not
close [#551](https://github.com/Chelis-Lang/sail/issues/551).

## Frozen inputs and actual output

The 2026-10-28 native run used one EC2 c7i.4xlarge in us-east-1. Sail build
source `4b3f7aa699d528e6c944b9e96f628bd3c79bdf86` is the frozen consumer
checkpoint `7caa519df1b57acc7d90fedb0a2419d78fd12a44` plus only the fork pin.
The source producer is the publicly fetched
`47fb3be6fc73dd9eec77ffec31866b3157f430df`, tree
`15900d58e8d63d0363f961c1063e6968933584c7`, based on upstream
`7d829946231d6413a398f6e4160cdfe951261dad`.

- Source lock: `04efc53f189f719c2ecd206ce8e15fe7d3c02643efc3f5a7835864c0dc594ee6`.
- Verus Rust toolchain: 1.99.1, compiler `58cf8f9c35b355199727627d07ae34a4e2846d67`.
- Pinned Linux Z3: `e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
- Actual variant: `7d829946231d6413a398f6e4160cdfe951261dad-b15bd93b3729520d`.
- Actual manifest: `d6096d5bd4952df54f8fdd0fdc157a1bb610298318166495604ccb7201dbe44b`.
- Packed archive: `8da9b48752d3e758ca2e501d6748e2d3fc43036ea3838e27ac6b0ae3a36786d2`.

The variant records the exact fork, host, source-lock and
`vstd-build:cargo-verus-release` features. Fresh isolated Rustup, Cargo and Sail
cache roots contain no `rust-src`; the real compiler sysroot and installed
components are checked. The bootstrap CI image uses the frozen consumer
checkpoint's valid old pin pair; candidate source and build roots are separate.
No existing Vargo output is relabeled and no manifest is synthesized.

## Executed scope

The normal build command runs before the separate locator, because
`--print-path` returns without building:

```sh
cargo xtask build-verus-engine --force --keep-intermediates
cargo xtask build-verus-engine --print-path
python3 ci/engine-artifact.py pack --platform linux \
  --build "$SAIL_RUNTIME_BUILD" --archive "$SAIL_RUNTIME_ARCHIVE" \
  --pin "$SAIL_RUNTIME_CANDIDATE_PIN"
python3 ci/engine-artifact.py install --archive "$SAIL_RUNTIME_ARCHIVE" \
  --pin "$SAIL_RUNTIME_CANDIDATE_PIN" --cache "$SAIL_RUNTIME_INSTALL_CACHE"
```

`SAIL_BUILD_JOBS=8` bounds the build. The isolated `CARGO_TARGET_DIR` is supplied
only to the xtask build/locator process; xtask clears it from Verus compilation,
and it is absent from later pack/install and acceptance commands.

| Gate | Actual result |
| --- | --- |
| Normal xtask source build | PASS; real manifest sealed, build 150 seconds |
| Vargo vstd verification | PASS; 2,059 verified, zero errors |
| Release-style Cargo-Verus vstd | PASS; 2,059 verified, zero errors; virtual Rust source paths and no local rust-src paths |
| Real pack and fresh install | PASS; all manifest-bound runtime files agree before/after and after installation |
| Source, compiler, lock and solver guards | PASS; 1,010 Sail inputs and 1,496 fork inputs match frozen source, exact fork version reported, toolchain bytes unchanged |
| Paired e2e/conformance, useful reuse and cost | NOT RUN in this construction gate |
| Engine differential/cancellation and determinism | NOT RUN in this construction gate |
| macOS runtime and both-platform selection | Pending |

The proposed Linux pin and `LINUX_MANIFEST` guard name these exact bytes so the
paired acceptance branch can exercise ordinary reuse. They are not production
acceptance. The descriptor's prospective release asset has not been published;
use the locally packed archive through normal validated installation. The
macOS pin and guard still identify the old schema1 runtime and do not qualify
the schema2 consumer. No public asset, release or final selection is made here.

## Evidence

The completed native archive is
`experiments/2026-10-28/acceptance-env551-runtime-4b3f7aa6-attempt1-native.tgz`,
41,675,914 bytes, SHA-256
`af93b2961d370d21355484963002158a533894df719d6a4b32056c12706c04ad`.
Authenticated upload/readback and independent download validation agree. It
contains executed controllers, exact commands and logs, source/toolchain/runtime
manifests, the actual `sail-build.json`, candidate pin and packed archive.
There are no failed attempts in this construction gate.

The separate producer source gate remains preserved at
`experiments/2026-10-28/acceptance-env551-producer-47fb3be6-attempt5-epoch-native.tgz`,
SHA-256 `a0c87c244a87f31a19e63699de17995641f5b3faeae9357140144486d7a2d876`.
It passed all 162 unfiltered fork targets (4,839 passed, 63 inherited ignored,
zero failed or filtered). Its raw rust-src Vargo build is not this release-style
runtime; its failed preparation/build attempts retain their original scope in
[fork PR #13](https://github.com/Chelis-Lang/verus/pull/13).
