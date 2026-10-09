# VI-0002: page-table fails one check on the pinned release

- **Status:** open (tracked here; behaviour difference between Verus releases)
- **Verus:** `0.2026.10.04.426d8b0` (pinned)
- **Found:** 2026-10-08, while pinning the standard tier (PR #18)
- **Project:** `page-table` (verita's `parno/verified-nrkernel` fork, commit
  `be213895`, archived locally)
- **Impact on sail:** none. The failure is identical in every plain and sail
  run on macOS and Linux, and it is recorded as the corpus entry's expected
  outcome.

## Description

With the pinned release, `page-table/src/lib.rs` verifies 607 functions with
1 error: `rlimit exceeded` in `allocate` (`impl_u/wrapped_token.rs:901`), under
the project's own `--rlimit 100`. verita's nightly passes the same branch with
a Verus `main` from shortly before this commit, so the proof's resource use
appears to differ between Verus versions. This is the kind of proof
brittleness the charter's fragility census measures.

## Follow-up

- Re-check when the pinned Verus is upgraded.
- ~~Include `allocate` in the fragility census: does it pass for some
  seeds?~~ Yes (below).

## Fragility census (2026-10-09)

`allocate` was measured alone, with this command (ADR 0014):

```sh
sail-bench fragility --tier standard --project page-table \
  --verus-arg --verify-only-module --verus-arg impl_u::wrapped_token \
  --verus-arg --verify-function --verus-arg WrappedMapToken::allocate --seeds 8
```

Results (`bench/results/fragility-census-page-table-allocate-arm64-macos-devload-2026-10-08.json`):
- **One check.** The function's whole proof is a single check, with limit
  3,000,000,000 (`--rlimit 100`).
- **The unmodified session** (Z3's default seed, as plain Verus runs it)
  exhausts the limit (`canceled`). This reproduces the recorded failure.
- **Seeds 1, 2, 3, 5, 6 and 7** also exhaust it.
- **Seed 4 proves it** (`unsat`) using 448,058,976 units, 15% of the limit.
- **Reproducible.** A second run gave the same answers and deltas.

So the proof exists, and Z3 finds it easily under one search order and not
at all within the limit under seven others. This is seed fragility (the
Mariposa reseeding axis), not a proof that needs more resources. That
fits verita's nightly passing with a slightly different Verus: a
different query text perturbs the search much as a different seed does.
An opt-in "accept a proof from any configuration" mode (charter rule 8)
would verify `allocate`; default mode must keep reporting the failure.
