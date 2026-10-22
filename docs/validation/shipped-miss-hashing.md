# Hashing on a shipped-proof miss (#286)

**Status:** measured; the toolchain-component miss goes from 3.8% slower than
stock to 0.8% slower (−1.04 s). The bundle hit and the lock-closure miss do
not regress (−0.29 s and −0.36 s). Admission is unchanged: every refusal the
new check makes is one the complete identity makes too, and every run's
output matches main's. The per-unit driver rehash that the
[miss-replies record](vstd-miss-replies.md) attributed most of the cost to is
smaller than estimated and is left unchanged (see below).

| Field | Evidence |
|---|---|
| Source | Main `2293db1f` (before the shipped solver replies of #287; this change does not touch them) against branch `perf/miss-hashing` (this PR), each staged by `ci/local.sh release` with its own shipped vstd proof (archives sha256 `07f3eda6…` main, `00af742b…` branch). Both proofs have identity `0391f755…`; the branch's manifest also records `sysroot_shape` |
| Runtime | Fork `f8589ed4`, variant `7d829946…-81186034d2e137a2`, pinned Z3 4.16.0; stock Verus `0.2026.10.18.7d82994` x86-linux |
| Environment | Native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04, lane `hash1020`. Rustup with default 1.99.0 and pinned 1.99.1 |
| Toolchains | *Minimal*: 1.99.1 minimal profile plus `rust-src` (676 MB). *Developer*: the same plus `rustc-dev` and `llvm-tools` (1.6 GB) |
| Arms | Hit: minimal toolchain. Component miss: developer toolchain. Lock miss: minimal toolchain, `cargo update -p proc-macro2 --precise 1.0.106` after `sail new`. Stock runs `cargo verus` on the toolchain of the arm it is compared with (minimal for hit and lock) |
| Isolation | Fresh `HOME`, `CARGO_HOME` and `SAIL_CACHE_DIR`, own project name and root per run; human output |
| Scope | Demo `--lib` project, first `verify` after `new` (not timed). Three rounds, arm order rotated per round. Linux only |

## Results

Wall seconds of `verify`: median (min–max) of three rounds. All 24 runs exit
0; no sample is dropped.

| Arm | Stock | Main | Branch | Branch − main | Branch vs stock |
|---|---:|---:|---:|---:|---:|
| Bundle hit (minimal) | 35.42 (35.42–35.62) | 14.32 (13.96–14.52) | 14.03 (13.72–14.07) | −0.29 | 2.52× faster |
| Component miss (developer) | 35.34 (35.13–35.68) | 36.68 (36.46–36.85) | 35.64 (35.60–35.78) | −1.04 | +0.8% (main +3.8%) |
| Lock miss (minimal) | 35.42 (35.42–35.62) | 34.67 (34.41–34.68) | 34.31 (34.28–34.33) | −0.36 | −3.1% (main −2.1%) |

`sail new` on the developer toolchain takes 0.08 s instead of 1.12 s (three
runs each, all equal to two decimals); it writes the same project and, as on
main, no lock file.

**Output.** After removing the run root, the project name and Cargo's
`Finished … in` time, every branch run's stdout equals main's in the same
round and arm, and its stderr equals main's as a set of lines. Against stock,
stdout is equal; stderr differs only by `note[SAIL3138]` on the miss arms and
the lock arm's `Compiling proc-macro2` version. All report `2060 verified` and
`1 verified`.

## What changed

- **Sysroot shape before the identity.** The shipped proof's identity hashes
  every library in the verifier's sysroot (`shipped::sysroot_identity`):
  about 640 MB on the minimal toolchain and 1.4 GB on the developer one,
  sequentially in `sail` before Cargo starts. The bundle manifest now records
  the name and size of each of those inputs (`sysroot_shape`). Sizes are part
  of what the identity hashes, so a different shape can never have the
  recorded identity; such a sysroot is refused before anything is hashed.
  A matching shape grants nothing, and a bundle without the field is decided
  by the identity alone. `sail new` uses the same check.
- **Sysroot hashing on several workers** when the identity is computed (hit
  and lock arms). The digest is unchanged: both stages above record the same
  identity.
- **One runtime read per run.** The shipped identity and instant revert's
  runtime admission (`retained::selection`, through `Installed::at`) each
  hashed all runtime files. A run now reads them once and shares the digests
  between the two (`source_build::RunDigests`); a new run, or another build
  directory, reads afresh.

## What did not change, and why

An `strace` read census of one component miss on main (in the evidence):

| Reader | Bytes | What |
|---|---:|---|
| `sail` (daemon) | 1,947 MB | The pinned Z3, read in full on every solver handshake (#213/#219) |
| `sail` | 1,428 MB | Sysroot libraries for the shipped identity, now skipped on this miss |
| Driver proxies | 721 MB | `verus` (4.7 MB) and `rust_verify` (26.5 MB), about 23 times |
| Driver proxies | 344 MB | Dependency artifacts and instant revert's graph copies |

The runtime is 130 MB on Linux, not 530 MB; the 530 MB the earlier record
counted is the drivers' repeated reads of the two executables. They come from
instant revert's native admission (`retained::Plan::compiler_matches`), which
hashes the actual launcher and compiler before and after each native compile.
The hit arm reads them too, through the shipped proof's native observation.
Passing the parent's digest, or checking stat stamps, instead would accept
compiler bytes that were not read at the point of use. ADR 0050 rules out
timestamps as content evidence for retention after mapped-write ABA
counterexamples, so this stays as it is; reducing it needs an owner decision
on that trust boundary. The daemon's per-handshake solver read is lever 1.3.

## Gates

On native c7i.8xlarge lane `hash1020g`, source before the rebase onto
`13ef5258` and `e45f9031` (which do not touch this code): workspace gate (832
tests, 37 skipped), all 35 solver tests, all E2E phases plus extra, and PR
conformance pass. FAST: 147 Same of 147 in all four modes. STANDARD: 159 Same
in pass-through and cache fill; cache warm and engine each record one
Unstable unit (Anvil, then `vest-generated/vest_tests`) whose two plain runs
disagreed on the order of `note: verifying module` lines, and zero Different
or infrastructure errors in every mode. macOS arm64 `ci/run.sh host`: 821
passed, 37 skipped.

## Raw evidence

Native evidence bucket, `experiments/2026-10-20/miss-hashing/miss-hashing-evidence.tgz`,
SHA-256 `90843d8d556dd66ce0dcc89acbc6896b37621d1b0242e2cc993c51175259cd1a`:
scripts, `results.tsv`, per-run stdout and stderr, output comparison, read
census summaries and host/stage identities.

## Reject known resolution misses first

The runtime/sysroot identity was still computed before checking whether vstd's
resolved versions and features could match the source policy. The same
resolution check now runs first. A match still requires the original complete
source, compiler, runtime, solver and adapter checks; this precheck only refuses.

A three-round native Mac control (mac-m4.metal, us-east-1a, macOS 27.0,
Cargo/Rust 1.99.1, runtime f8589ed4 on upstream 7d82994) records Vest cold at
37.57 seconds against stock's 37.59 seconds. The earlier combined baseline was
39.05 seconds against 37.26 seconds. The default-limit proof fails equally in
both arms (VI-0032); this is a valid failed-proof comparison, not a passing proof
claim. CLI SHA-256 `4824ed857071b26a2b998123e3ef9ca74cc53659b9de00a5d3a3091ff6dc6b49`.

That control contains an unselected lookup-isolation experiment, but this row
rejects the resolution before any native observer runs. It qualifies only this
refusal optimization. Fresh complete installed-product release measurements
remain separate. Native standalone source `1701eacb` passes workspace, solver, full e2e
including extra, PR/full-fast and standard conformance. Standard has 159 Same
and zero unstable, Different or infrastructure errors in all four arms; its
18-mutant HumanEval probe and both reverts also pass. Raw gate logs:
`experiments/2026-10-21/early-refusal-gates.tgz` in the S3 evidence bucket,
SHA-256 `778b7f167bf235a9d1cca8498d2d29a9e5081f6114001abe887edf75e5b1c899`.
### Unservable native lookup graphs

A native observation now checks whether lookup artifacts have current-run producer
records or running marks before reading compiler and source bytes. A refusal uses
ordinary compilation; passing this preliminary check grants no reuse and keeps the
complete graph capture and final provenance checks unchanged. No lookup entries are
filtered. The filesystem regression covers a foreign artifact refusal and verifies
that the ordinary capture still includes both artifacts.

Initial native Mac diagnostic (mac-m4.metal, us-east-1a, one paired round):
CapybaraKV cold took 30.78 s in Sail versus 31.50 s stock, with matching output and
a shipped-proof miss. This is diagnostic scope, not three-round release acceptance.
Full native gates and final combined measurements remain required.

### Fresh compiler families

Under the exclusive compiler-family lock, remember whether the owned target is
genuinely absent or an empty directory before metadata and store seeding. Such
a target has no old compiled artifacts to clean before its first invocation.
Existing targets, filesystem aliases, inaccessible paths and all retries retain
the ordinary cleanup. A filesystem regression covers the target lifecycle,
including files and both dangling and live directory symlinks.

Native Mac Anvil diagnostic: the input-preparation phase fell from 3.78 s to
2.18 s; a full cold run took 104.48 s, with matching successful verification.
This remains slower than the earlier stock baseline near 100 s; it is not
release performance acceptance. Repeated final measurements remain required.

The first successful public-profile Mac coexistence control exposed an omitted
cleanup side effect: retained admission canonicalizes the target before Cargo
runs, so the family must create its empty target explicitly. Without that,
the cold run verified correctly but retained no producer certificates and
subsequent reverts reverified. Fresh-target initialization now creates that directory and Cargo cache tag
before retained admission. Its filesystem regression requires a canonical,
tagged target and preserves existing artifact bytes. The composed Mac control
now passes all four steps: cold 39.17 → 33.74 s, edit 22.52 → 15.79 s, revert
22.54 → 1.97 s and unchanged 0.23 → 1.00 s (stock → Sail). The revert restores
the cold run's complete proof with zero verifier invocations. This is one
functional coexistence control, not the final three-round performance matrix.
The Mac host gate passes all 950 tests, Clippy clean. Native Linux workspace
and solver gates pass; full e2e and conformance are finishing before merge.
