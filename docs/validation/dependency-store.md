# Dependency store

Validation of [ADR 0045](../decisions/0045-dependency-store.md): new projects
and worktrees start from a verified vstd that an earlier managed run on the
same machine stored.

| Field | Value |
|---|---|
| Status | **Pass** on the measured scope: seeded first runs match plain stock Verus in exit code, stderr (without Cargo progress lines) and `verification results::` lines; mismatched and corrupt entries fall back to a normal verification |
| Source | Sail `feat/prebuilt-vstd` (lane L47), code at `93a55a0d` (later commits change only documentation and tests); before = `main` at `d4ac51af` |
| Runtime | Sail's Verus, fork `73dea021` (`engine-runtime-73dea021-linux` in the CI image; `-macos` from the Sail cache); oracle: stock release `0.2026.10.04.426d8b0`; Z3 4.16.0 |
| Environment | Linux: native EC2 c7i.4xlarge (16 vCPU, Xeon 8488C) for timings and c7i.8xlarge for the checks, us-east-1, Ubuntu 24.04, inside the CI image, one timed command at a time, load average 2–5. macOS: Apple M3 (macOS 26.5) under other work, load average 15–23 |
| Scope | A one-function project made as README's First verification says (crates.io `vstd = "=0.0.0-2026-10-04-0306"`), and vest at `11af01dd` (`-p vest_lib`, crates.io `vstd 0.0.0-2026-09-06-0133`, unpatched) |
| Limits | Managed runs only (the ordinary non-managed path is not seeded). MCP measured through a Python client with `PYTHONCOERCECLOCALE=0` (see below). Two platforms, one Cargo home each. No idle macOS timing |

## Checks against plain stock Verus

`scripts/dependency-store-check.py` (the method is in its docstring) creates
new projects whose source fails a vstd precondition and a postcondition, so
diagnostics point into vstd's registry source. For each case it compares
Sail's first managed run with plain stock `cargo verus verify` on the same
project in a new target directory.

| Case | Linux | macOS |
|---|---|---|
| donor: empty store, vstd verified and stored | identical; 33.8 s (plain 33.7 s) | identical; 83.9 s (plain 85.5 s) |
| seeded: new project, vstd from the store, one verifier run | identical; **1.29 s** (plain 33.5 s) | identical; **3.24 s** (plain 67.4 s) |
| twin1 and twin2: two new projects at once | both seeded, 1.36 s and 1.38 s | both seeded, 3.20 s and 3.37 s |
| mismatch: vstd with `default-features = false, features = ["alloc"]` | another key, no warning, vstd verified; identical | same |
| corrupt: one stored `.rmeta` damaged | `warning[SAIL3131]`, vstd verified; identical | same |

"Identical" means equal exit codes, byte-equal stderr once Cargo's progress
lines (`Compiling`, `Checking`, `Finished`, ...) and Sail's one SAIL3131
warning are removed, and equal `verification results::` lines: plain's
(vstd's `2060 verified, 0 errors`, then the crate's `1 verified, 2 errors`)
against the same lines rendered from Sail's reports. Every case read Current.
Raw outputs: `l47-store-check-linux.tgz` (sha256
`5ea4a4e8205a1508cdb089e58bc46d9df792802cd828ccf4a3c33bc9dd631aa3`) and
`l47-store-check-macos.tgz` (sha256
`24205c367ac322dab2c4a3b1f497385038a308ef1ddfb33f6578987107c8226e`), kept
outside the repository.

CI's `End to end (managed)` phase checks seeding after a prune and for two
new projects at once (`ci/test-managed.py`); the unit tests are listed in the
ADR.

## First-call timings (Linux)

Medians of three rounds, seconds; each round used a new Sail cache. "First
on the machine" is a project's first managed run with an empty store; "new
project" and "new worktree" are first runs after it, with the same
`Cargo.lock`.

| Run | Before | After | Plain `cargo verus verify` |
|---|---:|---:|---:|
| demo, first on the machine (`--incremental`) | 27.6 | 28.2 | 29.5 |
| demo, new project, first `--incremental` | 27.5 | **1.30** | 29.5 |
| demo, new project, first MCP `verify` | 27.7 | **1.51** | 29.5 |
| demo, new project, second `--incremental` | 0.55 | 0.61 | — |
| vest, first on the machine | 68.9 | 67.9 | 66.1 |
| vest, new worktree, first `--incremental` | 52.3 | **25.7** | 66.1 |
| vest, new worktree, second `--incremental` | 1.63 | 1.68 | — |

Ordinary `sail verify` (not managed) on the first demo: 29.7 s before and
30.1 s after, unchanged. Ranges were within ±0.6 s of each median. The vest
worktree's 25.7 s is vest_lib's own verification. Before the change a new
vest worktree also took less than the first vest run because the solver cache
already held vstd's answers; the Rust and Verus frontend for vstd remained.

The MCP row uses a Python MCP client with `PYTHONCOERCECLOCALE=0`. Without
it, Python 3 adds `LC_CTYPE=C.UTF-8` to its environment when the locale is
`C`, and the MCP server it starts inherits it; `LC_*` is part of a proof
context ([freshness](../freshness.md#which-environment-variables-are-proof-inputs)),
so that server's runs are another compiler family and another store key. It
then verified vstd itself (27.7–28.2 s) and stored it again, as designed.

Raw logs: `l47-timings-linux.tgz` (sha256
`c48fa5edbfbbdb08fcc6ee759af150155a98d962131704c3c6baeb0381bfe954`).

## Attempts

- `39ccf16c`, 2026-10-15: first timing round, MCP row not seeded; cause: the
  timing script started the daemon with `CARGO_TARGET_DIR` set and launched
  the MCP client from Python (locale coercion), two different proof contexts.
  Replaced by the three rounds above.
- Review of PR #154, 2026-10-15: seeding now goes through a hidden sibling
  renamed into place, ordinary misses are silent, stale leftovers are
  reclaimed, stored directories are checked and a failed adoption no longer
  fails the run. The first version of this change never seeded (its seed
  required an empty target, but the run's `cargo clean` had already written
  Cargo's bookkeeping there; the Linux check and `e2e managed` failed). Fixed
  by moving a bookkeeping-only target aside; then gate 600/600, solver 35/35,
  `e2e` (all phases), `conformance fast pr` 33/33 and this record's check
  passed on Linux (seeded 1.18 s against plain 28.8 s), and the check on the
  Mac.
- `d6f5612b`, 2026-10-15: CI-image `e2e managed` failed once: the new
  worktree checks ran before the prune checks, and copies of the project share
  its proof context hash. Moved after the prune checks; passed.
