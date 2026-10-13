# VI-0005: changing forwarded Verus arguments does not re-verify a fresh crate

- **Status:** open (tracked here; not filed upstream). A fork fix was tried and
  dropped (see below).
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

## Sail fork

Not fixed. The cause is known: rust_verify records the forwarded arguments
(`__VERUS_DRIVER_ARGS__...`) as dep-info environment dependencies, but via
Cargo it runs rustc twice, and the second (erase-macro compile) pass rewrites
the same `.d` file without them. The fork briefly carried those dependencies
into the compile pass (`441d6cd9`, then `b35e9226` for Cargo-selected
packages only) and was dropped (2026-10-13, rescue L12, ADR 0039): Sail's
repair passes the selected function as a forwarded Verus argument
(`--sail-repair-function`), so binding the forwarded arguments in dep-info
invalidated the build on every repair. Each HumanEval repair rebuilt vstd and
ended Historical, about 34 s against 6 s without the change. A future fix
would bind only result-affecting arguments; not now.

Sail's stock-mode workaround is unchanged.
