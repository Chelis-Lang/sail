# VI-0011: Cargo no-vstd metadata does not reach launcher macro mode

- Status: open, tracked locally; no upstream report filed
- Found: 2026-10-10
- Verus source: `426d8b01e7ffb910a36c15e1f870ff985bb61eef`
- Reproduction: `corpus/repros/verus-cargo-no-vstd/`

A Cargo package can declare `no-vstd = true` with `verify = true` in
`[package.metadata.verus]`. Cargo-Verus reads that supported field and forwards
`--no-vstd` through its per-package driver-arguments environment variable.
The Verus launcher chooses the macro mode before rust_verify decodes that
variable. Its `get_vstd_kind` reads literal process arguments only and then
sets the child `VSTD_KIND` to `Imported` when it finds no literal `--no-vstd`.
The builtin macros consequently expand assertions and Boolean postconditions
using vstd facilities, although the later verifier configuration says no-vstd.

Relevant pinned source locations are `source/cargo-verus/src/metadata.rs`
(`VerusMetadata::no_vstd`), `source/cargo-verus/src/subcommands.rs`
(`make_cargo_plan` forwarding per-package `--no-vstd`),
`source/verus/src/main.rs` (`get_vstd_kind` and the child environment assignment),
and `source/rust_verify/src/cargo_verus.rs`
(`extend_args_and_check_is_direct_rustc_call`, which decodes driver arguments).

An ordinary Cargo-Verus invocation on the minimal Boolean fixture returned
101 with E0433 and an entire-crate result showing zero verified functions.
The recorded run used a validated Sail source variant with interactive mode
disabled; the implicated launcher implementation is unchanged pinned source.
The evidence includes exact binary, patch, source and output hashes and records
Linux x86-64 emulation. A body-only `assert(true)` probe reproduced the same
macro failure. Setting `VSTD_KIND=NoVstd` outside the launcher does not solve
this ordering problem because the launcher replaces that environment value.

The explicitly selected optional engine relocates an already effective exact
`--no-vstd` Cargo driver token into the launcher arguments before `--`. It reads
only the common options and the current package's options using the pinned
separator decoding; it does not scan another package's flags or match a
substring. It removes only that delimited token from the child's two selected
packed environment values, preserving every other token and byte, and inserts
the literal option once. Simply duplicating the option was experimentally
rejected by rust_verify with `Option 'no-vstd' given more than once`. Duplicate
effective requests and contradictory macro modes fail closed with `SAIL3121`.
Its stock fallback preserves the same relocated arguments and environment. The ordinary stock/default adapter is unchanged.
Each adaptation records `launcher-options.json` alongside the target's report,
with the run, context, exact Cargo package, target, kind and forwarded option.
The source patch and full focused/independent verification criteria are unchanged.
Project results also name the adaptation in a run-bound provenance note.
`corpus/repros/verus-cargo-no-vstd/forwarding-oracle.json` records actual
Cargo verification against the direct same-option baseline: two complete
functions, identical diagnostics and 24,013 identical full-target SMT bytes.
These timings come from Linux x86-64 emulation under development load.

A potential upstream fix would determine macro mode from the same effective
Cargo driver arguments as rust_verify, while preserving contradictory-mode
rejection. No upstream issue has been filed.
