# Current negative compiler-generation controls

The source change is `cce420f` with refusal-fixture corrections `3eaff19` and
`76d08f1`. Independent review found no static authority blocker. The first
[native scope](m7-negative-compiler-generation-first-controls.md) passed all 15
affected tests, then failed strict Clippy on a test-only missing semicolon.
Correction `7d78ae2` retains every production predicate and assertion; corrected
strict controls and installed repair acceptance remain pending. The motivating
[recovered diagnostic](m7-repair-a960-recovered-setup.md) remains a complete
120-second fixture failure, with no performance credit.

Run the following affected scope in the existing exclusively returned private
Cargo target, using a committed source snapshot, one Cargo job and serial test
cases. Preserve each command's raw output, exit, elapsed time, source/lock hashes
and actual tested library image. Keep the common target, source-engine runtime,
old evidence and active foreign targets untouched. Check current container birth,
scratch write health, backing space and exact path holders before launching;
the interrupted Docker transition is not a completed control scope.

```sh
cargo test -p sail-verus --lib managed::runner::tests::negative_compiler_generation_requires_full_normal_current_and_complete_coverage -- --exact --test-threads=1
cargo test -p sail-verus --lib managed::runner::tests::current_negative_generation_reuses_real_dependency_without_a_proof_receipt -- --exact --test-threads=1
cargo test -p sail-verus --lib managed::build_inputs::family::tests:: -- --test-threads=1
cargo test -p sail-verus --lib managed::runner::tests::shared_fresh_artifact_cannot_replace_a_focus_context_proof -- --exact --test-threads=1
cargo test -p sail-verus --lib engine_events::tests:: -- --test-threads=1
cargo clippy -p sail-verus --all-targets -- -D warnings
```

Each process has a 300-second cleanup deadline for this component scope; this
does not change any production or original fixture deadline. Failure remains a
distinct terminal scope. The Cargo composite really builds a script-bearing
ordinary dependency offline, imports the positive generation, passes a captured
pinned negative full terminal through the production publication gate, imports
that generation into a different context, then rebuilds only the selected Rust
package and requires the ordinary dependency's actual Cargo artifact to be
fresh. It checks that the negative target remains incomplete/nonreused and no
successful proof receipt exists. This is a compiler-output admission control,
not a new real Verus proof, full input-owner conformance or a measured speed gain.

The same control requires VIR errors, partial full scope, unfinished/abnormal
termination and negative declaration establishment to leave the consumed export
absent. Restored-mtime external-input mutation must fail the existing post-watch
contract check, and an interrupted writer cannot revive an older local ledger.
The refusal matrix separately checks unknown/historical inputs, other Cargo
exits, missing requested targets, report errors/counts/exit/success mismatches,
malformed starts, missing completion and duplicate starts. Existing family and
receipt controls retain their original assertions.

After actual controls and strict checks close, the lead will rebuild the installed
product and run the unchanged original repair fixture at 120/30/5. Whole local
gates and benchmark requirements remain separate. No proof receipt, successful
target flag, Currentness predicate or verdict classification changes.
