# sail-lsp fixtures

`failing.json`, `passing.json` and `compile-error.json` are
`sail verify --output json` project results from real runs of Sail's pinned
Verus (fork runtime 7d82994) on a two-function library created by `sail new
--lib`: `double` with a postcondition it does not meet, the same source
repaired, and a type error. They are unmodified except that:

- vstd's per-function list is emptied (2 MB of rows the mapping never reads);
- the workspace path is `@WORKSPACE@`, the Verus build is `@VERUS_BUILD@`
  and the rustup home is `@RUSTUP@`, which tests substitute.
