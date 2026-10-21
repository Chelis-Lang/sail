# Neutral Verus lowering qualification

The explicit source lowerer was checked against both the pinned Sail fork and
stock Verus on native Linux on 2026-10-21. Both verifiers received the exact same
generated source for each control, with the pinned Z3. No neutral contract is
established by this record alone: caller-source currentness, actual obligation
mapping and contract-to-requirement entailment remain separate checks.

The native oracle required the expected actual success or failure, complete
crate verification and matching verifier result fields. Every control reported
`encountered-vir-error = false`; compile failures cannot satisfy the negative
controls.

| Exact source control | Expected outcome | Verified / errors, on each verifier |
| --- | --- | --- |
| Add one under bounded input | Verified | 1 / 0 |
| Short-circuit arithmetic guard | Verified | 1 / 0 |
| Signed minimum | Verified | 1 / 0 |
| Mutable wallet and old scalar state | Verified | 3 / 0 |
| Negated overflowing predicate | Not proved | 0 / 1 |
| Overflowing eager old capture | Not proved | 0 / 1 |

These are whole-crate backend counts, not requirement establishment counts.
The runtime fixtures compile with the real neutral macros and check the same
bounded input and state transitions independently.

Two failed attempts remain part of the evidence. The initial attempt could not
find the CI image's engine through the ordinary mutable cache; fixture lookup
now validates the explicit `SAIL_ENGINE_BUILD` runtime, while preserving strict
missing-input failures. The next attempt exposed a real mutable receiver
postcondition error. Lowering now uses `final(self)` for current poststate and
`old(self)` for entry state, including all arithmetic definedness conditions.
Entry preconditions and eager capture assertions still refer to entry state.
Mutable by-value inputs and nonreceiver reference parameters remain unsupported.

The actual oracle log SHA-256 is
`bdafc1d26b0830f3a9eb4eb7737c9bd87178c0ba42bfdf4d8280e87159c10edc`.
Generated sources, source mappings and original verifier stdout/stderr are
retained with both failed attempts. See the
[lowering specification](../spec/neutral-verus-lowering.md) for the supported
subset, source-policy admission and the explicit native command.

The corrected lowering source passed native `ci/local.sh gate`, the complete
solver lane (including this real oracle), full E2E and `conformance fast pr`.
Conformance found no differing verdicts, unstable units or infrastructure errors;
resource-limit drift retained the existing comparison policy. This adds no
conformance corpus entries or default-route behavior changes.

| Native log | SHA-256 |
| --- | --- |
| `sail-neutral-lowering-gate.log` | `27a63ef9d0053f1993ee4700aed20708b74c94d123e76597539f1b14695ba054` |
| `sail-neutral-lowering-solver.log` | `c3a5ad58ba3e4704b20fe77366b00ac1f793d651ad91a5027034ec35589ddd27` |
| `sail-neutral-lowering-e2e.log` | `bcbe6b21ece37a17a2f3b75b2f8aa33bf0121e983552ccd0d696862e3bc61bda` |
| `sail-neutral-lowering-conformance.log` | `46695dca3369023d6435b84f2695eeba0b3802225c0a82061c0070bc3188d89c` |

The retained oracle archive, including the original failed attempts, has SHA-256
`e84e7245bb7ea743c6d265184e215f19f004b2fabeb3b57e02f38648bf2eba99`.
The explicit `neutral_contracts_acceptance` example checks the separate public
managed helper. Its library invocation is independent of the direct verifier
oracle; a matching direct-verifier result does not establish that helper path.

## Actual managed helper

The native library acceptance example called `contracts::verify` on the original
add-one source and then on a body edit that returns the unchanged input. Both
completed through the real managed runner and pinned runtime. The original
source files remained byte-identical. Post-run original, emitted-source and
lockfile digests agreed with the returned manifests and translation trust.

| Case | Exit | Currentness | Requested target | Verified / errors | Compiler error |
| --- | --- | --- | --- | --- | --- |
| Original | 0 | Current | Complete | 1 / 0 | False |
| Body edit | 101 | Current | Incomplete | 0 / 1 | False |

The failed body edit contains one underlying `unknown` answer with reason
`(incomplete quantifiers)`, followed by `unsat`. Its legacy function label is
`failed` with an SMT tier, and a model is retained for the unknown check. That
model is not a SAT result or a concrete SMT refutation. The new neutral
projection reports `NotProved`, not a mutation kill. The original contains
no unknown checks and meets the complete attributed proof conditions.
Requested-target projections of these original records are retained as
regression fixtures; the complete raw archive remains unchanged.

The source policy SHA-256 in both runs is
`77cfe09e6aec9d1934662198c343800bcde4c53b6cda2ed1011fe43f180fc12a`.
The exact lockfile SHA-256 in both runs is
`132a4290419ad4e598e1adc1227b3a4e473d29545d4c6a237ecd0b67fce511ee`.
Actual metadata digests remain separate per shadow and are preserved in the
complete returned records:

| Original evidence file | SHA-256 |
| --- | --- |
| `positive.run.json` | `eefbcde69b645d739c808f380958e36627bc3e14106fc3fb6ae8fa7bbad15c08` |
| `body-edit.run.json` | `dea68029c895bc4b295b21543ea2ee0b3a939e94739b9dfa8f2021b2b9da7fa7` |
| `sail-neutral-managed-acceptance.log` | `03988b22c4e88dad2f2b4f7f96eae19d73d888bd21f8d0c8e0ef7f45a0a8d8ea` |

An initial real helper call correctly refused overlapping source and cache
namespaces with `SAIL3112`. The helper now creates separate owned sibling shadow
and backend-cache namespaces. No existing managed disjointness check changed.
A separate harness setup attempt passed a bare Cargo name where the existing
managed interface requires its actual executable path; the harness now resolves
the absolute rustup-proxy path without changing its dispatch filename. Both
failed attempts and their original diagnostics remain retained. Raw identifier
spelling is also checked against reserved translation and scalar names before
a backend runs.

To reproduce the library acceptance, build the example and the actual Sail/shim
binaries in the native CI image, then invoke the example directly with a fresh
evidence directory and the image's validated engine. Clear Cargo's test-only
loader environment; the helper deliberately refuses such overrides:

```sh
cargo build --locked -p sail-verus --example neutral_contracts_acceptance
cargo build --locked --release -p sail-cli -p sail-shim
export SAIL_VERUS_BUILD="$SAIL_ENGINE_BUILD"
unset RUSTUP_TOOLCHAIN LD_LIBRARY_PATH DYLD_LIBRARY_PATH DYLD_FRAMEWORK_PATH
./target/debug/examples/neutral_contracts_acceptance /fresh/evidence/directory
```


## Conservative proof projection and runtime witness

A fresh actual managed acceptance run uses the new neutral projection without
changing legacy reports. Its original is `Proved`, with 2061 underlying unsat
checks and no unknown, SAT, missing-answer or missing-source checks. Its actual
body edit is `NotProved`, with 2062 unsat and one unknown check; the verbatim
reason remains `(:reason-unknown "(incomplete quantifiers)")`. Every count includes
advisory and dependency checks. Exact requested-function attribution is checked
for both runs, and the original, shadow and lockfile hashes are checked after
execution. These outcomes establish contract proof under requires, not
requirement entailment, feasibility or a mutation-gate result.

The separately compiled original returns 2 at input 1. Reexecuting the actual
edited fixture with that same precondition-satisfying input triggers exactly
`SAIL0702: postcondition failed for requirement ADD-ONE`. A compiler error,
precondition failure or arithmetic panic does not satisfy this acceptance.
The separate runtime-sample record retains the edited source digest, typed
input and digest, observed diagnostic and digest, and reference output. This is
one runtime counterexample sample, independently of the unknown SMT answer.
The [minimal reproduction](../../corpus/repros/neutral-contract-unknown/README.md)
retains the exact emitted source and links the original requested-target record.

| Original final evidence file | SHA-256 |
| --- | --- |
| `positive.run.json` | `554200023dd60e212c4b222ac7a419c265c39bf961658195ad21a5cf08399f87` |
| `body-edit.run.json` | `ef55fb414e6ed55adddda0b34a02e84666512b11659e276fd19bd239e8fafb99` |
| `body-edit.runtime-witness.json` | `47549b644688198355826b34467085af7e9c5d11b2f881192789c9ededadf9a7` |
| `sail-neutral-proof-managed-final.log` | `3543fa894c0f832cf9186ca2eae422f6c910d64922f07ea40d0c103c42552dd5` |

The conservative projection and regression passed the native full gate and
solver lane, including the actual fork/stock oracle. The corresponding original
logs are `sail-neutral-proof-gate.log` (SHA-256
`0db79d18bb2237e889fc366ac4b9557fff455f207dc20ef341c11dcfaf5797d8`)
and `sail-neutral-proof-solver.log` (SHA-256
`62fae9ec52d0061572059cac2f7d93b9f1def62596de7b69692bf494774b934c`).
Final PR validation additionally covers the exact final harness and reproduction.
