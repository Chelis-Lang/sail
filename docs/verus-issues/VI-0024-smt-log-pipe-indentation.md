# VI-0024: SMT file logging changes literal solver-pipe whitespace

**Status:** locally reproduced in the actual AIR emitter; unfixed. No verdict
disagreement or native end-to-end Verus result is claimed.

Normal fork `73dea02112d730e7ab52f58158012d440fb89512` makes
`Emitter::indent` and `unindent` depend on whether a file log is attached.
`Emitter::log_node` uses that same indentation when serializing to the solver
pipe. Consequently a nested assertion under a push has different literal
solver input with file logging enabled. One-line commands need not differ.
See the [minimal reproduction](../../corpus/repros/verus-smt-log-indentation/README.md).

On macOS, Rust 1.98.1 compiles the actual AIR library and selects exactly one
test: 1 passed, 146 filtered out. This does not build a new runtime or establish
Linux/project qualification. The original single-line fixture's failed
assumption and all actual logs stay in the external edit-provenance evidence
set. No SMT bytes, cache keys or replies are normalized or rewritten.

External `live-prefix-edit-provenance-2026-10-17` raw stdout SHA-256
`e25b5ba31017b93b95cb6c68e0852b68ed65b14b910d27a5eaa32ef6951da451`;
build/test stderr
`accd930677f2917171cac88afa4e6cadf791b38698578fd502f7b29477efa1fd`.

For the source-only continuation research, `--log smt` therefore cannot be
assumed to preserve the earlier normal workload. The proposed alternative is
a disabled marker in the separate SMT transcript writer, with a compiler-owned
request-batch counter; it still needs review and runtime validation.
