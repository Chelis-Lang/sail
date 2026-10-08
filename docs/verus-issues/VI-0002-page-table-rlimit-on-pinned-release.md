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
- Include `allocate` in the fragility census: does it pass for some seeds?
