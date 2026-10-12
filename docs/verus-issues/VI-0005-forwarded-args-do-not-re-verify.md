# VI-0005: changing forwarded Verus arguments does not re-verify a fresh crate

- **Status:** open in stock Verus (tracked here; not filed upstream);
  fixed in Sail's Verus fork by `441d6cd9` (fix: keep verification dep-info in the compile pass).
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

Fixed in [Chelis-Lang/verus](https://github.com/Chelis-Lang/verus) by
`441d6cd9` (fix: keep verification dep-info in the compile pass), one commit
with its regression test
`source/rust_verify_test/tests/sail_vi0005_cargo_rerun.rs`, which fails on the
pinned stock source and passes on the fork. Cause found: rust_verify does
record the forwarded arguments (`__VERUS_DRIVER_ARGS__...`) as dep-info
environment dependencies, but via Cargo it runs rustc twice and the second
(erase-macro compile) pass rewrote the same `.d` file without them. The fork
carries the verification pass's environment and file dependencies into the
compile pass, so Cargo re-verifies when they change.

The first version also did this for unselected dependencies. `cargo verus
verify` forwards its arguments to every package, including vstd, so Cargo
rebuilt vstd whenever the arguments changed; Sail's repair loop changes them
for every function (`--sail-repair-function`), and each repair rebuilt vstd
and lost Current evidence (`SAIL3114`). Follow-up `b35e9226` (fix: carry
dep-info into the compile pass only for selected packages) limits it to
packages selected on Cargo's command line (`CARGO_PRIMARY_PACKAGE`);
dependencies keep stock freshness, and the test also requires that vstd is
not rebuilt.

The fork builds only the optional interactive engine. Stock-mode workaround:
Still needed for default `sail verify` (stock Verus): an unchanged verified
package gives no results, and Sail reports that rather than forcing
re-verification.
