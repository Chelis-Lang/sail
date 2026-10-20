# VI-0032: vest_lib exceeds the default rlimit on Verus 0.2026.10.18.7d82994

- **Status:** open (tracked here; behaviour difference between Verus releases)
- **Verus:** `0.2026.10.18.7d82994` (pinned). `0.2026.10.04.426d8b0` verified
  the same commit with no error.
- **Found:** 2026-10-19, during the 7d82994 re-pin (standard conformance tier
  and the instant-revert requalification on native Linux)
- **Project:** corpus entry `vest` (`secure-foundations/vest` at `11af01dd`,
  crate `vest_lib`)
- **Impact on sail:** none on verdicts. Every plain and Sail run fails the
  same way, and conformance reports the unit Same. The corpus entry records
  the failure as its expected outcome. Measurements that need a passing Vest
  pass `-- --rlimit 100`, as upstream verita does for this project. With that
  flag, all of Vest's calls in the instant-revert and cold-gap measurements
  exit 0. The flag differs from the shipped vstd proof's recording, so a run
  with it verifies vstd itself.

## Reproduction

With stock 0.2026.10.18.7d82994 (`cargo xtask fetch-verus`), in the corpus
checkout (`sail-conformance fetch --tier standard --project vest`), either
with its Verus dependencies patched to the pinned source (as conformance
does) or with its workspace `vstd` requirement moved to crates.io
`vstd = "=0.0.0-2026-10-18-0147"`:

```sh
RUSTUP_TOOLCHAIN=1.99.1 cargo verus verify -p vest_lib
```

Exit 101 with 3,236 verified and 1 error:

```
error: function body check: Resource limit (rlimit) exceeded; consider rerunning with --profile for more details
   --> vest_lib/src/cbor/format.rs:471:1
    |
471 | / fn parse_cbor_with_child<'i, const DET: bool, P>(
```

The same command with `-- --rlimit 100` exits 0.

## Description

`parse_cbor_with_child` in `vest_lib/src/cbor/format.rs` verifies under the
default rlimit (10) with Verus 0.2026.10.04.426d8b0 but exceeds it with
0.2026.10.18.7d82994, on the same Vest commit. The solver's resource use
for this body appears to have grown between the releases. Possible causes
are the new `prelude.rs` definitions and vstd's added preconditions, but the
cause is not isolated.

## Evidence

- Standard conformance, native EC2 c7i.8xlarge (us-east-1), source
  `70a5ae9d`, runtime `9c0ca7e7`: `vest/vest_lib` plain exit 101 (3,236
  verified, 1 error), Sail identical; the unit is Same in all four arms.
  Report archive `rq1-conformance.tgz` sha256
  `d9bac1132cafa17a92e3353ee5f558050c98ce9eccaada860a860ee15a1cbf73`.
  `vest-generated/vest_tests` (`--verify-module tls`) still verifies (2,720).
- The [instant-revert requalification](../validation/instant-revert.md#verus-7d82994-runtime-requalification)
  first reproduced the failure and then ran with `--rlimit 100`.

## Follow-up

- Re-check on the next Verus pin.
- A fragility census of `parse_cbor_with_child` (ADR 0018) would show whether
  some seeds pass under the default rlimit.
