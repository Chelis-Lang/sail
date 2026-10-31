# VI-0040: Mac abstract-domain proof differs between stock and the fork arm

- **Found:** 2026-10-31
- **Runtime:** stock `7d829946`, fork `47fb3be6`, Rust 1.99.1, pinned Z3;
  native arm64 macOS 27, mac-m4.metal / us-east-1c
- **Status:** unresolved; cause and standalone reduction remain open
- **Tracks:** [#634](https://github.com/Chelis-Lang/sail/issues/634)

The [pinned-package reproduction](../../corpus/repros/abstract-domain-mac-verdict/README.md)
compares semi-persistent's `abstract-domains` crate without modifying its
proofs. Stock passes all three captured runs. The fork with the managed
verifier flags and shim fails
`semi_persistent_abstract_domains::tnum::Tnum::add_carry_bitwise_has_eq`:
a postcondition error at `abstract-domains/src/tnum.rs:729:17`, exit 101,
1,171 verified / 1 error versus stock's exit zero and 1,172 / 0.
The comparator covers 909 function records in this unit and remains failed.
This is not the accepted VI-0003 timing-entry exception.

The complete Mac candidate comparison reports 22 same / 1 different,
5,357 function records, no unstable/infrastructure/skipped result, with
resource drift in 13 units. Linux passes these pins. This verifier-level
comparison does not execute managed artifact-digest reuse. Completed controls pass with stock/bare-shim and stock/managed-flags, but
fail with fork/bare-shim and fork/managed-flags. The fork also fails with
the pinned stock Z3 executable used directly, without the Sail shim or
managed flags. This localizes the observed behavior outside the cache
prototype and shim; the exact fork/query change remains unestablished.

The original output and every stock confirmation remain external, archive
SHA-256 `388e2a007aa3dea05168d33876134451c4465c13322cea9137f1b7575ee212d8`.
Independent stock/shim and fork/bare controls are diagnostics, not alternate
acceptance profiles. No resource limit increase, proof port, output filter,
query normalization or Different-to-Same conversion is selected. Default
platform performance/release acceptance remains unqualified at this gate.

Complete final native Mac controls/reductions archive SHA-256:
`f71416129dfb411543e700962b76374bade14793830a58467b95b9cdd049bb30`.
The original failed profile remains the acceptance result; passing stock
diagnostics are not substituted for it.
