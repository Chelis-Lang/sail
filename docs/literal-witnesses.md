# Find scalar literal-contract counterexamples

Use the separate opt-in witness family when a bounded literal-body mutant
violates a contract but its native Verus check remains UNKNOWN. Native mutation
classification and proof tiers stay unchanged. This path constructs a different,
explicit query for an exact admitted scalar body and independently checks the
concrete assignment against its unchanged contract.

Prepare queries through the existing job client, then execute them through the
existing SMT runtime. The job socket and runtime socket belong to the same
running `sail serve` instance.

```rust,ignore
let plan = jobs.literal_witnesses(&source_submission, 8)?;
plan.validate()?;
// Keep plan.unsupported and plan.omitted visible in the consumer report.
let response = runtime.execute(&sail_api::runtime::Request {
    version: sail_api::runtime::VERSION,
    input: sail_api::runtime::Input::Smt {
        queries: plan.prepared.iter().map(|p| p.query.clone()).collect(),
    },
})?;
```

Match each complete runtime result to its exact `Prepared` entry and call
`sail_api::literal_witness::observe`. `SatisfiableContractQuery` records SAT
for the separate contract-violation query. `NoWitness` records UNSAT for that
query. `Inconclusive` includes UNKNOWN, unavailable execution, partial output
and mismatched attribution. None is a source proof or native mutation kill.

For a SAT entry, call `sail_api::literal_witness::model_query(&prepared)` and
execute that query through the same runtime. Submit its complete returned
`QueryResult` to the source owner:

```rust,ignore
let concrete = jobs.validate_literal_witness(
    plan.clone(),
    prepared.candidate.id.clone(),
    &model_result,
)?;
// concrete.values contains original input names and exact Bool/Int values.
```

Validation regenerates the complete source-bound plan, decodes the full scalar
assignment and independently evaluates the original typed preconditions,
unchanged postcondition and eager `old` captures for the literal body. A
counterexample must satisfy every original precondition and make the defined
postcondition false. Overflow, an incomplete assignment or UNKNOWN establishes
no concrete counterexample. A model after a native UNKNOWN is never substituted.

The admitted slice is free functions with owned Boolean or fixed-width integer
inputs and scalar literal results. Checked addition, subtraction, multiplication,
comparisons, Boolean short-circuit evaluation, scalar conditionals and immutable
`old` expressions use the existing typed neutral specification semantics.
Predicate calls, references, nominal values and multiple requirement IDs remain
explicitly unsupported. The original literal inventory budget remains between
one and eight. All queries retain the existing raw runtime's default resource,
deadline and stream limits.

Retain the complete original source plan, query bytes, responses, validated
assignment and content hashes. On a later read, recheck those hashes and call
the source-owned validator again. The API's pure `Plan::validate` checks source
and query links; it does not repeat source admission or authenticate solver
execution. An older service rejects the new RPC explicitly and has no fallback.

The producing library offers deterministic `contracts::literal_witness::prepare`
and `validate` functions. For an actual installed-runtime diagnostic, use:

```sh
SAIL_BIN=path/to/sail cargo run -p sail-verus \
  --example literal_witness_acceptance -- path/to/fresh-evidence-directory
```

The runner retains original plans, requests, complete runtime output and
independently validated assignments for primitive Boolean/integer cases and
the order-service fixture. Tautological guarantees and impossible preconditions
are negative controls. This is bounded fixture acceptance, not whole-program
correctness, full realizability or a native positive-kill qualification.

Set `SAIL_WITNESS_SOCKET` to the cache socket of an already running instance to
exercise the actual source-owner preparation and validation RPCs and route all
queries through that instance's runtime endpoint. Without it, preparation and
validation call the owned library directly and queries use the selected binary's
existing raw runtime. Preserve this distinction in acceptance records.

A contract counterexample refutes that exact contract for the literal body.
Requirement IDs provide attribution. A stronger contract can fail while its
linked requirement still holds; semantic requirement rejection needs independent
checking of the corresponding requirement on the concrete observation.
