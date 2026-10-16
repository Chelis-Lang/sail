# Shipped dependency build

Validation of [ADR 0048](../decisions/0048-shipped-dependency-build.md): a
release archive ships vstd and its registry closure compiled and verified, so
the first managed run on a fresh machine replays them.

| Field | Value |
|---|---|
| Status | **Not yet qualified or shippable.** The hardened macOS matrix has one store-copy output failure (#184); eight other cases pass. The fork commit's runtimes are not published or pinned, re-pinned conformance is pending, and CI does not yet exercise serving. Prior candidate results below are historical |
| Source | Sail `feat/shipped-vstd` (lane M11-3); Sail's Verus from fork commit `b4450da9` (branch `m11-3/shipped-vstd`, fork PR #4), built locally with `cargo xtask build-verus-engine` on each platform |
| Oracle | stock release `0.2026.10.04.426d8b0`; Z3 4.16.0 |
| Environment | Linux: native EC2 c7i.4xlarge (16 vCPU, Xeon 8488C), us-east-1, Ubuntu 24.04, outside the CI image, load average 2–4; default toolchain 1.99.1 (Cargo 1.99.1). macOS: Apple M3 (macOS 26.5) under other work, load average 15–31; default toolchain stable |
| Scope | Staged release trees (`scripts/release.sh stage`) whose shipped build was recorded from a `sail new --lib` project with crates.io `vstd = "=0.0.0-2026-10-04-0306"`; checked projects made with `sail new` |
| Limits | One vstd version and resolution. No idle macOS timing. The CI image still runs the pinned `73dea021` runtime, which cannot record a shipped build, so CI does not exercise serving yet |

## Candidate hardening after takeover

The current source retains every compiler argument in its key, including
lint and rendering options: `--force-warn` bypasses `--cap-lints allow`,
and invalid `--color`, `--diagnostic-width` and `--check-cfg` options emit
errors. SDKROOT and MACOSX_DEPLOYMENT_TARGET are now bound, undecodable
environment input misses, both --extern spellings bind library and VIR
bytes, Cargo.toml is a recorded source, and redirected source filenames
miss even when the bytes match. Every destination is checked before placing
any output from an entry. Regression tests cover these boundaries.
Public proof origin uses `shipped:<identity>/<key>` rather than the local
replay run; it survives receipt, local-store and adopted-receipt reuse.
The build identity now additionally binds actual installed compiler/sysroot
bytes under relative paths, including the standard library and `proc_macro`;
matching a pinned toolchain's version alone is insufficient. Unit tests
require identical copies at different roots to match and altered, missing,
non-file or dangling inputs to miss. Fresh-machine serving with this final
guard remains pending. No sysroot hashing is added to unchanged runs.
The local checker now also relocates the actual Verus toolchain into a
private Rustup home, requiring a hit with identical bytes. Appending inert
bytes to that copy's Cargo executable must then miss despite its unchanged
version; the system installation is left intact.

The old broad lint equivalence and Apple link-variable exclusions below
were removed; the cross-Cargo serving result is historical. The checker
still requires serving at another Cargo home with matched compiler inputs,
and compares exact output under another Cargo version whether it hits or
misses. Current staged results will replace the historical table after the
candidate build is checked. The hardened `c98b3828` matrix completed on
macOS with the same candidate fork: eight cases passed, while store-copy
omitted vstd's stdout result line. The exact defect independently reproduces
on rc.5 and is tracked as [#184](https://github.com/Chelis-Lang/sail/issues/184);
the new assertion remains strict. Raw matrix and host-check evidence:
`shipped-macos-c98b3828-failed.tgz`, SHA-256
`40a14821146831c6811302947ac8295960199553ceacec8e14100d3725a10fa0`.

## Checks against plain stock Verus (prior candidate)

`scripts/shipped-deps-check.py` (the method is in its docstring) gives every
case an empty Sail cache and a new Cargo home at another path than the
release's, and compares Sail's first `sail verify` with plain stock
`cargo verus verify` under the same Cargo home in a new target. Every project
fails a vstd precondition (`v[0]`) and a postcondition, so stderr holds
`::: <Cargo home>/registry/src/index.crates.io-…/vstd-…/std_specs/vec.rs:55:9`
and the quoted vstd line.

| Case | Linux | macOS |
|---|---|---|
| new: `sail new` project, vstd replayed | identical; **1.27 s** (plain 29.0 s) | identical; **5.44 s** (plain 34.1 s) |
| other-home: longer Cargo home path, default toolchain 1.98.1 (Cargo 1.98.1) | identical; 1.67 s (plain 29.6 s) | identical; 2.94 s (plain 28.9 s) |
| package-dir: workspace member, run from its directory | identical; 1.58 s (plain 29.4 s) | identical; 4.50 s (plain 52.5 s) |
| rerun: case 1 again, vstd reused, no verifier for it | identical; 0.60 s | identical; 0.92 s |
| mismatch: vstd with `default-features = false, features = ["alloc"]` | another key, no warning, vstd verified; identical | same |
| corrupt: the stored vstd `.rmeta` damaged | `warning[SAIL3133]`, vstd verified; identical | same |

"Identical" means equal exit codes, byte-equal stdout (both
`verification results::` lines), and byte-equal stderr once Cargo's progress
lines and Sail's one SAIL3133 warning are removed. The rerun's one verifier
is the failing crate's, which keeps no receipt. Raw outputs:
`m113-shipped-check-linux.tgz` (sha256
`ae2bf4911cf1013e266fbe3f700b4e8059d2ac0a5b8992c6d5cd546a9122a97d`, with the
timing log) and `m113-shipped-check-macos.tgz` (sha256
`8d730c4380b3bc27abfd9bc15d743d065910b9abda4423a6c1bcb74013fd2d9c`), kept
outside the repository.

## First-run timings

`scripts/shipped-deps-timing.py`, medians of three rounds, seconds. Each
round: a new `sail new` project (one verified function) under a new Cargo
home and an empty Sail cache, dependencies fetched beforehand (untimed).

| Run | Linux | macOS (loaded) |
|---|---:|---:|
| Sail's first `sail verify`, shipped build | **1.29** | **3.50** |
| the same without the shipped build (before) | 30.66 | 54.52 |
| plain stock `cargo verus verify` | 29.71 | 42.57 |
| Sail's second `sail verify` | 0.26 | 0.47 |

The macOS rounds ran before the last two key changes below (the build and
the checks were then redone; serving costs the same).

## Archive size

| | Linux x86-64 | macOS arm64 |
|---|---:|---:|
| shipped build, unpacked | 151 MB | 117 MB |
| shipped build, gzip | 31.9 MB | 24.6 MB |
| whole archive, gzip (with / without the build) | 74.3 / 42.4 MB | 59.9 / 35.3 MB |

## Status of the gates

| Gate | Status |
|---|---|
| Dependency-store output correction (#184) | Open; separate fix reproduces rc.5's defect and passes the live macOS reproduction; native qualification pending |
| Fork change reviewed and merged into the fork's `sail` branch | Open (fork PR #4; the fork's CI passes) |
| Runtimes for the fork commit published (`engine-runtime-<sha>-linux`, `-macos`) and pinned in `fork.toml` and `artifact-*.json` | Open (maintainer step); the PR keeps the `73dea021` pin, so CI stays on the published runtime |
| `ci/local.sh conformance fast` and `standard` with the re-pinned runtime | Open, after the re-pin (ADR 0043) |
| A CI check that records and serves a shipped build | Open, after the re-pin |

## Attempts

- 2026-10-16, Linux and macOS, first staging: `release.sh stage` refused to
  ship (vstd not recorded): rustc's working directory for a registry package
  is the canonical registry path (`/private/var/…` on macOS), which the
  remapping missed. Fixed by remapping both forms.
- 2026-10-16, macOS check: no case served under Python, which sets
  `SDKROOT` on macOS; Apple's link variables were removed from the key.
- 2026-10-16, both: `RUSTUP_TOOLCHAIN` (the user's default toolchain, set by
  rustup's proxies) was in the key; removed (the Verus launcher runs its
  driver with its pinned toolchain).
- 2026-10-16, Linux check: other-home (Cargo 1.98.1 against a build recorded
  with Cargo 1.99.1) missed from vstd's macro crates on: Cargo 1.98.1 passes
  a package's `[lints]` table as `--allow=…` and `--check-cfg` flags, 1.99.1
  does not. Capped lint settings were removed from the key; both platforms
  then passed.

- 2026-10-16, takeover, inherited Linux gate, solver, end-to-end and fast PR
  conformance passed on the original candidate with runtime `73dea021`
  (serving was not exercised by these gates); archive sha256
  `b36db856a8148f4a77051ca7598d79223078f5b46c89693b7b4704606e45f2ea`.
- 2026-10-16, `c98b3828`, hardened macOS matrix: eight cases passed;
  store-copy failed stdout identity (#184), despite matching proof origin,
  exit code and stderr. Exact rc.5 reproduction established that the omission
  predates the shipped-build change. The separate correction must qualify
  before this feature can pass its full acceptance matrix.
