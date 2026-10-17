# Module-candidate absence

Use rustc 1.99.0. Copy this directory to a disposable directory; compile
`lib.rs` with `rustc --crate-type=lib --emit=metadata lib.rs`. Both candidates
produce an ambiguity error. Move `foo/mod.rs` outside the source tree and repeat:
compilation succeeds. Restore it and repeat: ambiguity returns. Keep all three
stdout/stderr streams and statuses. No Verus or wrong-verdict claim is made.
See `docs/verus-issues/VI-0025-module-candidate-absence.md`.
