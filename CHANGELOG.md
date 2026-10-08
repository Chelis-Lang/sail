# Changelog

All notable user-visible changes are recorded here. The project follows
semantic versioning once the API is published.

## Unreleased

- Workspace skeleton, CI fast tier and AI-authorship guard.
- `sail-z3`: a pass-through Z3 shim for Verus (`VERUS_Z3_PATH`), with an
  optional trace tap (`SAIL_TRACE_DIR`).
- `cargo xtask fetch-verus`: installs the pinned Verus release.
- `sail-bench profile|seeds|history`: query-time profile, seed stability and
  history-dependence analyses over shim traces.
- `sail-conformance`: `cargo-verus` corpus units (with Verus dependencies
  patched to the pinned source in a scratch copy), project requirements
  (skipped and reported when unavailable), and the standard-tier corpus.
