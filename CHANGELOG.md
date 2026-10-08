# Changelog

All notable user-visible changes are recorded here. The project follows
semantic versioning once the API is published.

## Unreleased

- `sail verify`: runs `cargo verus verify` with the same arguments, through
  `sail-z3` and the cache daemon (started on demand).
- `sail serve`: the daemon, serving a local disk cache over a Unix socket.
  `sail-z3` with `SAIL_DAEMON_SOCKET` set serves replies from the cache and
  starts the pinned Z3 only on a miss, replaying the whole session prefix
  (ADR 0011).
- `SAIL_DAEMON_TIMEOUT_MS`: the shim's daemon client timeout (default
  10,000 ms).
- `sail-z3` in cached mode exits when its solver dies, as pass-through does,
  instead of waiting for stdin to close. A reply cut short by the solver's
  death is never stored. Fault-injection tests: `docs/fault-injection.md`.
- Workspace skeleton, CI fast tier and AI-authorship guard.
- `sail-z3`: a pass-through Z3 shim for Verus (`VERUS_Z3_PATH`), with an
  optional trace tap (`SAIL_TRACE_DIR`).
- `cargo xtask fetch-verus`: installs the pinned Verus release.
- `sail-bench profile|seeds|history`: query-time profile, seed stability and
  history-dependence analyses over shim traces.
- `sail-conformance`: `cargo-verus` corpus units (with Verus dependencies
  patched to the pinned source in a scratch copy), project requirements
  (skipped and reported when unavailable), and the standard-tier corpus.
