# Neutral Rust contracts

The `sail-contracts` library checks scalar contracts when ordinary Rust
code runs. The checks also run in release builds. A runtime check establishes
the behavior exercised by that execution; it does not establish an SMT proof.
The ordinary `sail verify` Verus input route is unchanged.

The [Verus lowering library](spec/neutral-verus-lowering.md) admits a complete
supported scalar source file, emits a content-bound shadow and can run the
actual managed backend. Its native oracle acceptance is separate from runtime
checks. Matching requirement IDs do not establish contract faithfulness.
[Neutral source jobs](neutral-contract-jobs.md) expose that actual helper through
the existing local Sail job service without caller-created shadows or tools.

Use the crate from the Sail workspace or a pinned Git revision of it. It is
not published to crates.io. The [version-one specification](spec/neutral-contracts.md)
defines the accepted source subset and the separate backend boundary.

```rust
use sail_contracts::{requires, ensures, requirement, satisfies};

#[requirement("COUNTER-NEXT")]
fn next_value(before: u64, after: u64) -> bool {
    after == before + 1
}

#[requires(value < 100)]
#[ensures(|result| next_value(value, result))]
#[satisfies("COUNTER-NEXT")]
fn increment(value: u64) -> u64 {
    if value == 0 { return 1; }
    value + 1
}
```

`increment(4)` returns `5`. `increment(100)` stops with `SAIL0702` before
executing the body. An implementation returning another value stops at the
postcondition. Every normal return checks its actual result, including an
early `return`. Unwinding is not a successful return.

There is one `satisfies` identity and one guarantee per implementation in this
slice. Multiple preconditions run in source order. The identity is explicit
owner metadata; matching IDs do not prove the contract faithfully represents
the requirement. That entailment remains Oyster's responsibility.

## Entry values and arithmetic

An inherent method can compare its final scalar fields with copies made at
entry:

```rust
#[derive(Clone, Copy)]
struct Counter { value: u64 }

impl Counter {
    #[sail_contracts::requires(self.value < 100)]
    #[sail_contracts::ensures(|result| result == self.value && self.value == old(self.value) + 1)]
    #[sail_contracts::satisfies("COUNTER-NEXT")]
    fn increment(&mut self) -> u64 {
        self.value += 1;
        self.value
    }
}
```

Each `old` occurrence copies a scalar after the preconditions and before the
body. It never retains an alias into mutable state or calls a user `Clone`
implementation. Its argument cannot call code or use the eventual result.
Results must be owned and `Copy`, such as a scalar or a `Copy` struct with
scalar fields.

Arithmetic and comparisons in contracts and predicates dispatch only on sealed
scalar types. Arithmetic stops with `SAIL0703` on overflow or invalid division,
even when Rust's ordinary release overflow checks are disabled.
`sail_contracts::Int::new(value)` constructs a checked runtime `i128`; its
operators never wrap. A later verifier interprets neutral `Int` as mathematical
integers and must retain that distinction in its evidence qualifiers.

## Admission limits

`SAIL0701` identifies unsupported contract syntax. Predicates are call-free,
pure scalar expressions. Contract calls target their generated checked entries;
an unmarked ordinary function is not accepted as a predicate. Quantifiers,
generics, traits, unsafe, async, opaque macros, loops and ordinary executable
function calls are outside this first slice. They are rejected instead of
being erased or approximated.

Attribute expansion checks syntax and uses scalar trait bounds, but it does
not replace Rust name resolution and type checking. In particular, arbitrary
named types can have user-defined field autodereferencing. Whole-source proof
admission must resolve all fields and predicate calls, admit only the stated
scalar closure, and reject unresolved types, effectful trait implementations
and hidden source dependencies before dispatch. A generated symbol name or
successful runtime compilation cannot establish typed purity or a proof.

`sail-contracts-ir::parse_source` rechecks the actual predicate definitions and
their requirement IDs. It rejects unresolved calls and forged entry names.
It performs no solver call and constructs no verification verdict.

The runtime and source-admission fixtures run with:

```sh
cargo test -p sail-contracts -p sail-contracts-ir -p sail-contracts-macros
cargo test -p sail-contracts -p sail-contracts-ir -p sail-contracts-macros --release
```

[ADR 0066](decisions/0066-neutral-contract-runtime.md) records the crate boundary
and the separate acceptance required for actual Verus lowering.

The [runtime qualification record](validation/neutral-contract-runtime.md)
records native gates and their exact evidence hashes. Runtime checks remain
distinct from proof establishment.

## Select flat Rust profile two

For the bounded order-service forms, set `sail_api::contracts::Submission.profile`
to `2`. The Rust client negotiates explicit profile support before sending the
source. Legacy submissions select profile one. The shared syntax reader offers
`parse_source_with_profile(source, 2)` and retains nominal enum metadata.

Profile two supports unit enums, flat records without adding Copy or Clone,
shared nominal input references and mutable scalar input references. Owning
nominal inputs and conditional/block specification results of nominal types are
unsupported. Direct flat-field checks remain supported. Several distinct
`satisfies` attributes can share a single conjunctive guarantee. Assurance keeps
one row per requirement identity; requirement entailment remains independent.
The [lowering specification](spec/neutral-verus-lowering.md) and
[decision](decisions/0076-neutral-source-profile-two.md) define the exact subset.
Runtime checks evaluate the actual borrowed result and original state. They do
not establish source proof, conclusive mutant rejection or whole-application
correctness. Backend UNKNOWN remains explicit.
