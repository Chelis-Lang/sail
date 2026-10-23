# VI-0034: Stable spec formals change a later proof's solver context

Status: fork defect, release blocker [#337](https://github.com/Chelis-Lang/sail/issues/337).
The signature-size correction passes the bounded compiler control and the
full installed Mac three-round HumanEval cohort (165 Same). Its rebuilt platform stages
pass ordinary public Vest undo and factory checks. Full qualification remains
in progress; see [the runtime record](../validation/spec-signature-runtime.md).

The three-round native macOS HumanEval mutation cohort finds a functional
stock/fork disagreement for `human-eval-constant-004`: stock verifies
`exists_even_index`; fork f8589ed4 rejects its postcondition. Fresh independent
Cargo arms reproduce it without Sail, its shim or cache. The pinned Z3 binary
is identical in both arms. The
[reproducer](../../corpus/repros/verus-spec-formal-context/README.md) records
source, flags, toolchain, dependency lock and the limits of smaller extractions.

SMT replay returns `unknown`, reason `incomplete quantifiers`, in the fork's
context where stock returns `unsat`. The failing proof query itself matches;
the earlier spec termination contexts use different formal names. Restoring
the preceding `count_even` termination scope recovers `unsat` while keeping the
fork's definition axioms. Restoring definition-axiom binder names alone does
not. Changing the shared canonical prefix alone also does not. Distinguishing
complete signature sizes recovers the proof in the captured script. These
controls identify an input-context sensitivity; they do not establish an
internal Z3 cause or an aliasing/capture defect.

Candidate fork `7d9d5410159a7baa2c74868ce1f591678788eb23` lowers uniquely named
spec formals using complete signature size and position, counting unused
parameters. Recursive copies retain signature size and depth. Original names
and spans remain available for diagnostics; ordinary proof/exec formals,
nested binders and function identities retain their lowering. Legacy serialized
variants are retained and new ones appended. Matching compiler/vstd/runtime
artifacts must be rebuilt together. No solver option, Sail cache key or
conformance expectation changes.

On native macOS 27.0 / EC2 mac-m4.metal / Rust 1.99.1, the actual candidate
compiler and stock both exit zero on the original full-crate mutation, with
all 331 function statuses equal. The candidate passes 13 VIR library tests
and all five spec-parameter regression tests, including counterexamples,
recursive copies, generic trait-default callers, shadowing and unused slots.
This is bounded compiler acceptance, not a full mutation or release result.
Native Linux build, all fork regressions, full fast corpus and consumer
workspace/solver/E2E checks also pass. The full standard, deep and installed
mutation checks remain in progress.

Raw component controls, literal SMT and commands:
`experiments/2026-10-23/human004-compiler-controls.tgz`, SHA-256
`2e7ead547b5991e2bb526bf43afa84722dd2f686c37f83d090f6b44f3dd6b5b0`.
The original failed cohort remains recorded in the reproduction and release
matrix. No upstream issue has been filed.
