# 0006: Conformance harness and corpus manifest

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Charter rule 1 asks for the same per-function verdicts and the same
diagnostics as plain Verus, with the differential harness enforcing this on
the whole corpus. The corpus survey proposed a manifest format compatible
with verita (`docs/survey/corpus.md` §4).

The Verus-internals survey found:
- `--output-json --time` gives per-function `success`, rlimit and time;
- `--no-report-long-running` removes timing-dependent diagnostics (§6, §8).

## Decision

- **Manifest.** `corpus/corpus.toml`, schema v1, is the survey's draft
  restricted to what the runner supports today:
  - `build = "verus"`, with `crate_roots` or `crate_roots_dir`, `exclude`,
    `extra_verus_args` (with `{out}` substitution), `env`, `requires`,
    `expected` and `expected_from`;
  - unknown fields are rejected;
  - `cargo-verus` and `script` builds are parsed but refused by the runner
    until they are implemented;
  - projects are fetched shallowly at their pinned SHA into
    `<cache>/corpus/<repo>-<sha12>`;
  - the manifest records the Verus release it was validated against, and the
    harness refuses to run if that does not match `corpus/toolchain.toml`.
- **A unit is one Verus invocation.** Both sides run with identical
  arguments, plus `--output-json --time --no-report-long-running`. Plain
  Verus runs with `VERUS_Z3_PATH` cleared, so the launcher uses its bundled
  Z3. The sail side sets `VERUS_Z3_PATH=sail-z3` and
  `SAIL_Z3_PATH=<pinned z3>`.
- **Comparison.** A unit is `same` only if all of these are identical:
  - exit code;
  - stderr, byte for byte;
  - the `verification-results` and `func-details` JSON;
  - the set of functions and each function's `success`.

  No normalization is applied. Per-function rlimit differences are reported
  as *drift*, not as differences, because Z3 itself produces them
  (`corpus/repros/z3-nla-rlimit-nondeterminism/`). Harness failures are
  `infra-error`, never a conformance verdict.
- **Reports.** `sail-conformance run` writes `report.json` (schema v1, with
  machine details) under `<cache>/conformance/<timestamp>/`. It exits 1 on
  any difference and 2 on infrastructure errors.
- **CI.** `.github/workflows/conformance.yml` runs on Linux:
  - PRs: a small subset of the fast tier;
  - nightly: the whole fast tier;
  - manual dispatch: any tier.

  It is not yet a required check; it becomes one once it has been stable for
  a week.

## Consequences

- **Per-function verdicts come from Verus's own breakdown.** The survey notes
  that `success` misses bit-vector-only failures, but stderr equality covers
  them.
- **Comparing stderr byte for byte is strict.** Any timing-dependent
  diagnostic that survives `--no-report-long-running` will show up as a
  difference to investigate, not to hide.
- `cargo verus` projects (the standard tier) need runner support. That is the
  next corpus step.

## Update 2026-10-08: `cargo-verus` builds and the standard tier

The standard tier needs Cargo projects, so the runner now supports
`build = "cargo-verus"`. `script` builds stay unsupported: no pinned project
needs one (see the memory allocator below).

- **Invocation.** The release's `cargo-verus` (next to `verus`) runs as
  `cargo-verus verus <cargo_verus_command> <extra_cargo_args> --
  <extra_verus_args> --output-json --time --no-report-long-running` in the
  package directory. The Z3 environment is exactly as for `verus` builds
  (plain: `VERUS_Z3_PATH` cleared; sail: `VERUS_Z3_PATH=sail-z3`,
  `SAIL_Z3_PATH=<pinned z3>`); Cargo passes it to the Verus driver.
- **`cargo_verus_command`** is `verify` (the default) or `focus`. Every
  standard entry uses `focus`, as verita does: only the listed package is
  verified, its dependencies are built with `--no-verify`, and the forwarded
  Verus arguments reach only that package. `verify` would also re-verify
  vstd from source in every unit (vstd is already its own fast-tier unit),
  and `cargo verus` refuses `--verify-module` outside `focus`.
- **Scratch copy.** Each `cargo-verus` unit runs on a copy of the fetched
  project (without `.git`) in `<cache>/work/<run>/<unit>/src`, because Cargo
  writes `Cargo.lock` into the tree and the fetched source is never
  modified. The copy is deleted after the unit. It lives under the cache,
  not under `--out`: CI's `--out` is inside this repository, and Cargo would
  take a copy there for a stray member of sail's workspace. Preparation
  refuses a scratch directory inside any Cargo project.
  `cargo metadata` resolves the lockfile once, before either side runs, so
  both sides start from the same tree. Both sides use the same
  `CARGO_TARGET_DIR` (`<cache>/work/<run>/<unit>/target`), emptied before each run
  and deleted after it. The same path keeps paths that Cargo prints (for
  example in a failed build script's report) identical on both sides, and
  emptying it stops Cargo from treating the second side's crate as fresh and
  skipping its verification.
- **`patch_verus_deps = true`** mirrors verita's `inject_verus_patches`
  (`tools/verita/src/dependencies.rs` at the pinned commit), implemented in
  `sail_verus::patch`: `vstd`, `verus_builtin` and the other Verus packages
  the project references are patched, via `[patch.crates-io]` and
  `[patch."https://github.com/verus-lang/verus.git"]` in the workspace root
  manifest, to the pinned commit's sources (fetched to
  `<cache>/corpus/verus-<sha12>`), and exact `=` pins on them are rewritten
  to the local version so that Cargo applies the patches. vstd is then built
  (not verified) from the same sources the release was built from.
- **Toolchain and Cargo environment.** Cargo runs with `RUSTUP_TOOLCHAIN`
  set to Verus's toolchain from `corpus/toolchain.toml` (overriding project
  `rust-toolchain` files, as verita's single-toolchain runs do), with
  `CARGO_TERM_QUIET=true`, `CARGO_TERM_COLOR=never` and
  `CARGO_TERM_PROGRESS_WHEN=never`, and without Cargo/rustc variables
  inherited from an enclosing `cargo run` (`CARGO`, `RUSTC`, `RUSTFLAGS`,
  wrappers and the like).
- **No stderr normalization.** Cargo's only timing-dependent output is its
  status lines (`Compiling ...`, `Finished ... in 2.3s`). `CARGO_TERM_QUIET`
  suppresses them at the source, so stderr holds only diagnostics and is
  still compared byte for byte. Repeated plain runs of `verified-ironkv`
  produced byte-identical stderr.
- **Several JSON documents.** Verus prints one `--output-json` document per
  verified crate (`rust_verify/src/main.rs`), with no crate name in it, so a
  `cargo verus` run can print several. `RunRecord` now keeps all of them in
  stdout order, plus any non-JSON text after the last one. A unit is `same`
  only if, in addition to the checks above:
  - both sides printed the same number of documents;
  - each document's `verification-results` and `func-details` match the
    other side's document at the same position;
  - per-function verdicts match, keyed by (document, module, function);
  - the trailing non-JSON stdout is identical.

  This is at least as strict as before: a `verus` run prints one document.
  Under `focus` only the verified package prints one, so standard units
  have one document each.
- **Requirements.** A project whose `requires` are not available is skipped,
  and listed in the report (`skipped`, `summary.projects_skipped`) and in
  the summary line. `singular` is available when `VERUS_SINGULAR_PATH` names
  a file, and `openssl` (headers and libraries for `openssl-sys`) when
  `OPENSSL_DIR` names a directory or `pkg-config --exists openssl`
  succeeds, `rust-target:<triple>` when rustup lists the target as
  installed for Verus's toolchain, and `program:<name>` when `<name>` is on
  `PATH`. Any other requirement is unknown and so unavailable.
  Requirements cover what a project's *build* needs outside Cargo. A build
  that fails for lack of one is not a conformance result, and its stderr is
  not even reproducible: a panicking build script's report contains the OS
  thread id (`thread 'main' (78167816) panicked at ...`), which differs
  between runs.
- **The memory allocator is tier 2.** The pinned release's `rust_verify` is
  built without Verus's `singular` feature (it reports "Found singular
  command when Verus is compiled without Singular feature"), so the
  allocator's `integer_ring` proofs cannot verify with the pinned release
  whether or not Singular is installed. verita builds Verus from source with
  `--features singular`. The entry is pinned in tier 2 with
  `requires = ["singular"]` until the corpus has a Singular-enabled Verus
  build.
- **Fork pin.** `page-table` is pinned to verita's fork branch
  (`parno/verified-nrkernel`, branch `z3-4.16.0`), which carries the Z3
  4.16.0 proof stabilization that upstream `matthias-brun/verified-nrkernel`
  does not have yet (11 commits ahead, 4 behind at pin time). ADR 0010
  (item 4) allows fork pins that are pinned by commit and archived locally,
  so the entry sets `archive = true` (a bundle under `<cache>/archive/`).
- **Build requirements found while pinning.**
  - verismo builds for `x86_64-unknown-none` (`source/.cargo/config.toml`),
    so it needs that rustup target for Verus's toolchain. verita's nightly
    gets it only because the svsm entry, listed earlier, installs it.
  - The `verismo` crate (not `bitflags_verus` or `verismo_tspec`) builds
    `hacl-sys`, whose build script needs `cmake` and `ninja`. verita's
    single verismo entry is therefore split into `verismo` and
    `verismo-main`, so the first still runs where those tools are missing.
  - anvil builds `openssl-sys`.

  The conformance workflow installs all three for dispatched non-fast tiers.
- **Benchmarks.** `sail-bench cold` selects units the same way: it skips
  projects with missing requirements and prepares `cargo-verus` units once,
  then reuses the prepared tree for every round.
- **Observed outcomes.** Plain Verus is not clean on every entry, and that
  is recorded, not fixed: `page-table` reports one `rlimit exceeded` error
  (in `allocate`, `impl_u/wrapped_token.rs`) with the pinned release, on
  both macOS arm64 and Linux x86-64, identically in every plain and sail
  run. verita's nightly passes the branch, but it builds Verus `main`,
  which is newer than the pinned release.

## Update 2026-10-08: where conformance runs

CI runs only the PR subset of the fast tier, as the `Conformance` job in
`ci.yml`. The whole fast, standard and deep tiers run locally with
`ci/local.sh conformance <tier>` (ADR 0013). This supersedes the nightly and
dispatch schedule above.

## Update 2026-10-09: rustc's long-type files

On the secondary Verus pin (Rust 1.98.0), rustc writes long type names to a
file with a random name (`<stem>.long-type-<random>.txt`) and reports that
name in its diagnostics, so plain Verus's own stderr differed between
identical runs. Three plain runs of `examples/iterators/skip.rs` gave three
different names.

This is fixed at the source, not by normalizing. `COMMON_FLAGS` gains
`-Z write-long-types-to-disk=no`, so rustc prints the type inline and stderr
is byte-identical across runs. Both sides still get identical flags, and
nothing sent to Z3 changes.

## Update 2026-10-09: a second kind of unit, the `rust_verify_test` tests

ADR 0019 adds `sail-conformance rust-verify-test`, which compares plain
Verus and sail on each test of Verus's own `rust_verify_test` suite, run
from a source build of the pinned commit. A test is not a corpus unit:
- **No `--output-json`.** The suite's harness chooses Verus's flags, so
  the comparison is libtest's outcome (`ok`, `FAILED`, `ignored`), the
  exit code, and stderr and stdout byte for byte. stderr holds every
  diagnostic Verus reported, as the harness renders it.
- **Two blanked fields.** A panic's OS thread id and libtest's `finished
  in` timing are blanked before comparing. Neither is Verus output, and
  both vary between identical runs. Nothing else is normalized.
- **Unstable, by line.** A test is also `unstable` when the plain runs
  vary and sail differs from the first plain run only on lines where the
  plain runs vary among themselves (random temporary paths in the
  harness's own messages). Corpus units keep the stricter rule.
- **Reports** go to `<cache>/conformance/rust-verify-test-<timestamp>/`
  (or `--out`), with a reproduction for each `different` test.
