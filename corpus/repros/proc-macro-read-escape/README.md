# Procedural-macro input outside tracked compiler declarations

The procedural macro reads `external-value.txt` outside its Cargo workspace
with ordinary `std::fs::read_to_string`. It emits that integer as Rust tokens
without calling a tracked-path API by default. There are no third-party dependencies.

This is a concrete example of the documented undeclared-input boundary, not
an end-to-end Sail false-proof reproduction. It shows why SourceMap, dep-info
and compiler-provided tracked-path declarations cannot by themselves establish
complete physical compiler-input coverage ([#373](https://github.com/Chelis-Lang/sail/issues/373)).

Run from the repository root with a fresh output directory:

```sh
python3.12 corpus/repros/proc-macro-read-escape/repro.py --out "$SAIL_REPRO_OUT"
```

The runner copies the fixture, uses the pinned compiler and an isolated Cargo
home, and keeps all builds outside the repository. The ordinary Cargo control
prints **7, 7, 9**: editing only the data file retains the old expansion;
touching the requesting source forces the macro to read the new value.
The runner also checks `declared-input`, which emits an `include_str!` dependency
alongside the literal. That positive control prints **7, 9** after the same edit
without touching any Rust source.

An existing qualified engine build additionally exercises the real compiler:

```sh
python3.12 corpus/repros/proc-macro-read-escape/repro.py \
  --out "$SAIL_REPRO_OUT" --build "$SAIL_FORK_BUILD"
```

This mode checks the engine's pinned manifest and every runtime artifact,
explicitly selects its Z3, and keeps the exact same macro library and proof
source for both invocations. With input **7**, the genuine compiler verifies
one obligation and emits its completed verification/compilation observation.
With input **9**, the genuine compiler rejects the assertion. The runner records
whether either phase or final dep-info names the external file; it does not
assume that a completed observation covers every actual input. The existing
pinned fork's untracked observations omit this file from both phases and final
dep-info. The declared control records the exact external pathname in the
verification SourceMap and tracked-path fields. Erased compilation omits it;
this independently checks why both phases matter. Naming the path does not
prove stability between the macro's original read and the later include.

The private observation endpoint is an explicit, standalone research capability.
This fixture does not enable it in default managed execution, authenticate the
writer of a retained phase file, or test the default original-environment
transport. Raw commands, statuses, source/runtime hashes and stdout/stderr stay
in the requested output directory. A timeout or unexpected compiler result
fails the runner and preserves the evidence.

Restricting a compiler's filesystem access is a separate architectural question.
Denying this read can stop the macro before verification, but its new error
cannot replace the ordinary compiler's default diagnostics. A path allowlist
also leaves allowed files mutable and supplies no atomic observation point.
