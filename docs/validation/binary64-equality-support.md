# Binary64 numerical equality support

Status: qualified standalone mathematics, extracted with identical source files.
No production Rust, four-model composition, runtime pin or native primitive
boundary changes. No fresh whole-main acceptance result is claimed.

At frozen `e37236f7`, direct pinned Stock checks the whole file with default
resources, a 120-second outer deadline, one thread and `--no-cheating`: 38 verified,
zero errors, all 35 user functions. All nine exact mathematical mutations fail
ordinary verification with the same 35 functions and verified+errors=38. They
cover NaN, signed zero, finite sign/fraction, hidden leading bit, normal numeric
scale, NaN reflexivity, infinity sign and complete comparison refusal. No
compiler/VIR/resource refusal is credited. Independent lead audit passes.

The runtime is the Stock pin in `corpus/toolchain.toml`, source `7d829946`,
Rust 1.99.1 and its pinned Z3 4.16.0, on native c7i.4xlarge Linux in us-east-1 with the
qualified core image. Final source/runtime mapping and all 333 retained Git
source hashes pass. Authenticated byte-identical PUT/GET preserves the raw log:
`experiments/2026-10-27/acceptance-ieee-denotation-e37236f7-controls-native.tgz`,
7,321,131 bytes, SHA256
`622b286ed11a4fcb79d82324111e18cdb80f2a5757d7be44a36780146e853d87`.

Earlier failed attempt: `8ff8bc72`, 20 verified/1 ordinary failed constructive
mask witness; no positive qualification. The universal/scale source is retained
and the corrected literal guidance qualifies in the later complete file.

The model proves independent binary64 numerical meaning over all u64 patterns.
Native Rust IEEE comparison, Serde variant/accessor/iterator behavior, actual
to_bits fidelity and the retained unproved Stock builtin bridge are separate.
There is no change to the original scoped request/DOM obligation or its failed
diagnostic. The broader compiler-origin and service proof remains open.
