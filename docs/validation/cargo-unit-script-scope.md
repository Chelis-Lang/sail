# Conditional build scripts and failed requested proofs

**Status:** the bounded native correction passes the workspace gate and full
managed acceptance. Remaining local gates, composed installed-product mutation
qualification and Mac checks are pending.

Native Linux c7i.8xlarge, us-east-1; pinned Cargo/Rust 1.99.1, runtime f8589ed4
on upstream 7d82994. The reproduction adds a Windows-only path dependency with
a build script that must never execute to the existing managed acceptance
project. Its original failing-first, independently verified vstd, repeated
failure, source-edit and reuse assertions remain intact.

- Baseline `791b897d` (fixture on main `4636ff20`): first failed proof exits 101
  with Currentness Unknown, violating the existing acceptance assertion.
- The correction passes the full managed acceptance with this same fixture.
  The failing requested target remains incomplete, and later calls reuse its
  independently established vstd receipt. The unused script never executes.
- Unit witnesses refuse unknown schemas, different roots, duplicate roots,
  invalid dependency indices, unexpected actual units, missing/duplicate
  completion messages and malformed artifact records. Planned scripts are
  never exempted by this absence authority.

The graph is read from the current pinned Cargo request. It is not a cross-run
absence memo. Ordinary receipt, byte, environment, mutation and before/after
input checks remain in force. No release performance claim follows from these
correctness checks; the full CapybaraKV mutation loop is still required.
