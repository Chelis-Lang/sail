# Kani fork: deterministic output and stock conformance

Status: **passes.** The bundle of Sail's Kani fork, `Chelis-Lang/kani`
release `sail-kani-0.68.0-1`, gives byte-identical CBMC input for 49 of 49
harnesses across ten rebuilds from one checkout path (S3's rule needs 99%).
On the fast corpus and s2n-quic-core it agrees with stock Kani 0.68.0 on
every verdict and on every check of all 100 harnesses (44,623 checks), with
the vtable sanity-check description as the one masked field. Sail's Kani
runtime is now this bundle, with stock 0.68.0 kept as the oracle
([ADR 0187](../decisions/0187-kani-deterministic-output-fork.md)).

## The fork and its bundle

| | |
|---|---|
| Repository | `Chelis-Lang/kani`, public GitHub fork of `model-checking/kani`, default branch `sail/0.68.0` |
| Branch | `sail/0.68.0` = tag `kani-0.68.0` (`0d2328a9`) + `320178f6` (the S3 patch, `kani-0.68.0-deterministic-goto.diff`, applied unchanged) + `ecf42b98` (fork note in the README, `.github/workflows/sail-bundle.yml`) |
| Upstream workflows | all 14 disabled on the fork; the three push-triggered runs started by the first push were cancelled |
| Bundle build | `sail-bundle.yml` run 46136353516 on `ubuntu-22.04`: upstream's setup scripts (rustup, CBMC 6.11.0 `.deb`, Kissat 4.0.1, cvc5), the `charon` submodule, then `cargo bundle -- 0.68.0`; 5 min 47 s end to end |
| Release | prerelease `sail-kani-0.68.0-1` at `ecf42b98`: `kani-0.68.0-x86_64-unknown-linux-gnu.tar.gz`, 140,334,843 bytes, sha256 `b0b2561a4dfa6deaba292bd1775f5c217371dbaed6cc1be942efd6f50c0b97cb`, and its `.sha256` file |
| Sail pin | `engine/kani/fork-bundle-linux.json` (all 120 files and the empty `scripts/` directory); stock stays in `engine/kani/bundle-linux.json` |

The fork archive has the same 120 file names, modes and empty directory as
stock. Three files differ:

| File | Stock | Fork | Why |
|---|---|---|---|
| `bin/kani-compiler` | 11,415,008 bytes | 11,447,496 bytes | the patch |
| `bin/kani-driver` | 8,485,320 bytes | 8,484,328 bytes | the patch |
| `bin/kissat` | 529,600 bytes | 529,600 bytes | built from source by upstream's setup; the only differing string is its embedded build date and runner host name |

`cbmc`, `goto-cc`, `goto-instrument`, `goto-analyzer`, the Kani library and
every Rust library are byte-identical to stock: the hosted runner reproduces
upstream's library build exactly.

## Environment

| | |
|---|---|
| Host | one `c7i.8xlarge` (32 vCPU, 64 GiB), us-east-2, Ubuntu 24.04.5, kernel 7.0.0-1018-aws, lane `kani-fork`, 2026-11-28 |
| Stock (oracle) | `cargo install --locked kani-verifier --version 0.68.0`, `cargo kani setup`; all 120 installed files match `engine/kani/bundle-linux.json` |
| Fork | `KANI_HOME=~/forkhome cargo kani setup --use-local-bundle` with the release archive (sha256 checked); all 120 installed files match the fork pin |
| Rust | nightly-2026-08-21 (rustc 1.100.0-nightly 8925ea358), installed by Kani setup for both arms |
| Projects | arrayvec 0.7.8 pilot (Sail `b22a60f7`, `spikes/kani-pilot/pilot`, its `Cargo.lock`); rust-osdev/x86_64 `9fc13798` (`--all-features`); DataDog/lading `7b3d3f78` (`lading_throttle`, `--solver cadical`); aws/s2n-quic `e11382fd` (`quic/s2n-quic-core`, S3's `Cargo.lock`, sha256 `5b7738d1…6cd2b3`) |

## Method

S3's method, unchanged: a wrapper installed as each bundle's `cbmc` records
the sha256 of the exact file CBMC reads (L2) and runs the real binary, or
replays one recorded output in byte-only builds. L1 is the `*.symtab.out`
file `kani-compiler` writes. Every build removes the target directory and runs
`cargo kani ARGS -Z unstable-options -j N --output-format terse` from one
checkout path; builds 1 to 5 use 32 Cargo jobs and `-j 16`, builds 6 to 10
use 3 and `-j 5`. `RUSTC_WRAPPER` was empty.

Per-check comparison uses S3's `compare_checks.py`: per harness the CBMC exit
code, `cProverStatus` and the multiset of (property, description, file, line,
column, function, status) from the raw `cbmc --json-ui` output, plus Kani's
summary lines. Install prefixes (`~/.kani/kani-0.68.0` and
`~/forkhome/kani-0.68.0`) are mapped to one token; then the description of
the "Correct CBMC vtable size" check is masked.

## S3 rule: rebuilds from one checkout path

| Project | Builds | L1 identical | L2 identical |
|---|---|---|---|
| arrayvec pilot | 10, all real runs | 9 of 9 | 9 of 9 |
| s2n-quic-core | 10: build 1 real, 2 to 10 byte-only | 40 of 40 | 40 of 40 |
| lading_throttle | 2 real | 17 of 17 | 17 of 17 |
| x86_64 | 2 real | 34 of 34 | 34 of 34 |

Stock across its two real builds: arrayvec 1 of 9, lading 9 of 17, x86_64
26 of 34 at L2. No harness's goto file name varied in either arm.

## Stock versus fork, per check

Build 1 of each arm, real runs.

| Project | Harnesses | Checks | Kani summary | Exact | Prefixes mapped | Vtable description masked |
|---|---|---|---|---|---|---|
| arrayvec pilot | 9 | 2,127 | identical (6 verified, 3 failed by design) | 2 of 9 | 2 of 9 | **9 of 9** |
| lading_throttle | 17 | 4,374 | identical (17 verified) | 9 of 17 | 9 of 17 | **17 of 17** |
| x86_64 | 34 | 6,757 | identical (34 verified) | 34 of 34 | 34 of 34 | **34 of 34** |
| s2n-quic-core | 40 | 31,365 | identical (40 verified) | 1 of 40 | 20 of 40 | **40 of 40** |

Statuses are equal in every project (for example arrayvec 2,040 `SUCCESS`
and 87 `FAILURE` in both arms). The masked checks number 22, 48, 0 and 100
and are `SUCCESS` in both arms; masking hides their description only, and
their status, location and property stay compared. Stock prints, for example,
`(MIR type RigidTy(Adt(AdtDef(DefId { id: 768, …` where the fork prints
`(MIR type arrayvec::CapacityError<u8>)`.

The oracle does not reproduce that field itself. Stock against stock, build 1
against build 2: arrayvec 2 of 9 exact and 9 of 9 masked, lading 9 of 17 and
17 of 17, x86_64 34 of 34. Fork against fork: exact on every harness of all
three projects.

## Release version bumps (investigation, nothing built)

S4 found that each s2n-quic release commit, which bumps every crate's
version, misses every harness. Question: could a fork change name symbols
without the version-derived hash and keep user-visible output identical?

**Where the version enters.** Cargo hashes the package version into the
`-C metadata` value it passes to the compiler, and rustc derives the crate's
`StableCrateId` from it. Kani's GOTO names reach that identity four ways:
function symbols are rustc's v0 mangled names (`…Cs<disambiguator>_10kani_pilot…`);
type tags are `tag-_<type_id_hash>`; constant allocations and `global::N`
names carry the codegen-unit name (`kani_pilot.<16 hex>::kani_pilot::…`); and
the fork's allocation fingerprints hash definition paths, which include the
crate identity. Kani's own check IDs (`KANI_CHECK_ID_<cgu>_<n>`) carry the
codegen-unit name too, but `kani-driver` strips them before printing.

**Experiment.** Fork arm, one checkout path, real runs. The arrayvec pilot
goes from version 0.0.0 to 0.0.1; s2n-quic-core from 0.92.0 to 0.92.1 (its
nine exact `=0.92.0` requirements follow). Symbol tables are dumped with
`goto-instrument --show-symbol-table` and compared field by field.

| | arrayvec pilot | s2n-quic-core |
|---|---|---|
| L1 and L2 identical across the bump | 0 of 9 | 0 of 40 |
| Symbol tables identical after masking the four identity token families | 3 of 9 | 0 of 40 |
| Per-check records identical (check-ID tags masked) | 9 of 9 | 28 of 40 |
| The same, vtable description masked | 9 of 9 | 35 of 40 |

Masking tokens after the fact is not enough: the residue is unnamed
`global::N` allocations whose numbers now name other allocations (31 against
36 bytes), because Kani numbers them in code-generation order and that order
follows the hashed names. S4's renumbered check IDs have the same cause.

To test a version-free identity without building anything, a
`RUSTC_WORKSPACE_WRAPPER` replaced Cargo's `-C metadata` for workspace
members with a hash of the crate name, crate types, `--test`, the `--cfg` set,
edition and crate root path, leaving `-C extra-filename` alone:

| With the version-free identity | arrayvec pilot | s2n-quic-core |
|---|---|---|
| L1 and L2 identical across the bump | **9 of 9** | **40 of 40** |
| Per-check records identical across the bump, exact | 9 of 9 | 40 of 40 |
| Per-check records against the Cargo identity at the same version (check-ID tags and vtable description masked) | 9 of 9 | 34 of 40 |

Kani's printed output already carries the crate identity. Across the pilot
bump, Kani's regular output differs in 1,380 lines: CBMC's progress lines
name loops and goto files by mangled symbol (`Unwinding loop
_RNvNtCskqJ6IaSJL2h_10kani_pilot9harnesses7any_vec.0`). In the per-check
records, the six s2n-quic-core harnesses that differ name some properties by
mangled symbol, print `sizeof(struct _<type-id hash>)` in CBMC's `memcpy`
check descriptions, and attach a vtable sanity check to a different
monomorphisation, since only the first function generated carries it.

**Verdict.** A fork change can remove the miss but not while keeping
Kani's printed output identical to stock, so it is not clean and nothing is
built:

- *How it would work.* `kani-compiler` already rewrites the rustc arguments
  it receives (it appends its required flags in `kani_compiler::run`), so it
  could replace `-C metadata` as the wrapper did. The simulation shows that
  removes every release-bump miss on both projects, with verdicts and checks
  unchanged across the bump.
- *Why it is not output-preserving.* Stock prints rustc's crate identity in
  property names, CBMC `memcpy` descriptions, loop names and goto file names.
  A fork with its own identity prints different hashes there: 6 of 40
  s2n-quic-core harnesses would differ from stock in fields the conformance
  compares exactly today, and Kani's regular output would differ on every
  harness. Conformance would have to mask hash tokens inside those fields.
- *Other costs.* The replacement must stay unique (two versions of one
  registry crate differ only in their source path), it changes every
  `TypeId` value a program can observe, and no stock run can produce its
  names. It would need its own ADR under ADR 0049's conformance rule and
  whole-corpus validation. The gain is S4's release-commit misses: 16 of 60
  s2n-quic commits, none on Firecracker.

## Sail's runtime path

The changed `sail` binary, built on the same host, installed the fork runtime
with `sail engine install --backend kani`: it downloaded the release asset
named by the pin, authenticated every file and reported
`"runtime_release":"sail-kani-0.68.0-1"`. `sail verify --backend kani` then
ran two pilot harnesses through the staged fork bundle:
`harnesses::p1_push_pop_roundtrip` was `bounded_success` (exit 0) and
`harnesses::f1_push_never_panics` was `counterexample` (exit 1), matching
Kani's summary for both arms above.

## Limits

- One host, one checkout path. The cross-path loss S3 found on bolero
  harnesses (`CARGO_MANIFEST_DIR` in the program) is unchanged.
- s2n-quic-core builds 2 to 10 replayed a recorded CBMC output; their L2
  bytes are real, their verdicts are not.
- Kani's own regression suite was not run on the fork.
- No harness here uses contracts, stubbing or loop contracts; Firecracker and
  the rest of the standard tier were not compared.
- No macOS bundle exists yet.

## Attempts

| # | Date | Result | Cause |
|---|---|---|---|
| 1 | 2026-11-28 | Pilot version-bump runs with Kani's regular output failed at once | `--jobs` requires `--output-format=terse`; rerun without `-j` |

## Evidence

`2026-11-28-kani-fork.tar.gz`, 4,494,814 bytes, sha256
`9c7772790ab6aa45b7e10b38e33ded3626e532063af177822c9fef5088acfcfe`, kept
outside the repository. It holds the two fork commits as patches, the bundle
workflow log, every script and wrapper, both install inventories, per-build
metadata, hashes at both levels, driver logs, compact per-check records of
every real run, the version-bump dumps' analysis, Kani's regular output for
the pilot bump, the Sail runtime log and results, and the Kissat string diff.
Goto binaries and raw CBMC JSON were not retained. The host was terminated
after the copy (about one hour up).
