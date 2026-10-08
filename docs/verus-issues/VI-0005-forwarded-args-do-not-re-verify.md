# VI-0005: changing forwarded Verus arguments does not re-verify a fresh crate

- **Status:** open (tracked here)
- **Verus:** `0.2026.10.04.426d8b0` (pinned)
- **Found:** 2026-10-08, building `sail verify --output json` (ADR 0014)
- **Impact on sail:** when Cargo considers a package up to date, `sail verify
  --output json` gets no results from Verus. Its document then has no crates
  and says why. sail does not force re-verification.

## Description

In a package that verifies:
1. `cargo verus verify -- --output-json --time --log-dir A` verifies and
   prints the JSON results.
2. Running it again with `--log-dir B` prints nothing and only `Finished`.

Cargo considers the crate fresh, although the Verus arguments changed. The
same applies to arguments that change results, such as `--rlimit`. A package
that fails verification is never fresh, so this affects only verified
packages.

## Workaround

To get results for an unchanged, verified package, change a source file or
run `cargo clean -p <package>` first. A future `sail` option could do this on
request.
