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

Use `sail-contracts = "=0.2.0"` for the published stable crate,
or use the Sail workspace or a pinned Git revision. The [version-one specification](spec/neutral-contracts.md)
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

## Select widened profile three

Set `Submission.profile` to `3` for the forms the owner authorized in
[ADR 0103](decisions/0103-scoped-neutral-widening.md). The service advertises
profile three in `neutral_contract_profiles`; clients refuse it on endpoints that
do not. Profile three admits everything in profile two, plus:

- **Division and remainder in specifications.** `/` and `%` follow Rust's
  checked integer semantics. Division truncates toward zero, and the remainder
  takes the sign of the dividend, so `-7 / 2 == -3` and `-7 % 2 == -1`. Each
  operation is defined only for a nonzero divisor and a representable result.
  The signed minimum divided by negative one is undefined, and so is the signed
  minimum's remainder by negative one, exactly as Rust's checked operations
  refuse them. An undefined operation never makes a guarantee true. Both
  operands share one fixed-width integer type.

- **Calls, casts and `match` in executable bodies.** A body can call an
  admitted contracted function by a plain path (`clamp(x, 9)`, `crate::a::f(x)`,
  `Self::g(self)` or `Meter::g(m)`) and call a method on `self`. Each call is
  checked against the callee's own `requires` and uses its `ensures`, exactly
  as Verus checks a call. A body can cast to a fixed-width integer type; the
  cast keeps Rust's truncating semantics, and Verus proves nothing about the
  result beyond them. `match` admits integer and Boolean literals, literal
  ranges, qualified unit enum variants, `|` alternatives, wildcards and guards.
  Shared borrows such as `&value` are admitted. Calls to requirement predicates,
  library functions, closures and generic functions are refused, and so is any
  recursive call cycle, because the neutral syntax carries no termination
  measure for functions. Method calls on values other than `self`, casts to
  `usize`, `isize` or `bool`, mutable borrows, binding and destructuring
  patterns, loops and macros stay refused.

- **`while` loops with invariants and a termination measure.** The first
  statements of a loop body declare its head: an optional
  `sail_contracts::invariant!(cond, ...)` and exactly one
  `sail_contracts::decreases!(measure)`. Invariants use the specification
  semantics of `requires`, including definedness. The measure is an integer
  expression whose mathematical value Verus proves nonnegative and decreasing.
  The head can read inputs and locals declared with an explicit scalar type,
  such as `let mut i: u32 = 0;`, and a function with loops needs unique local
  names. The runtime macros type check the head but never evaluate it.
  Sail adds nothing to the head: an insufficient invariant fails verification.
  `loop`, `for`, labels, `break`, `continue`, `old` in a loop head and a
  missing measure stay refused.

- **Multi-file source closures.** Set `Submission.modules` to the crate's
  out-of-line module files, each named by its crate-relative module path rather
  than a filesystem path. Every `mod name;` declaration must resolve to exactly
  one submitted file, and every submitted file must be declared; `#[path]`
  redirection is refused. Module files can hold annotated functions, requirement
  predicates, structs and enums. Imports that start with `crate`, `self` or
  `super` are admitted when they name an admitted module, type or free
  function, including renamed imports. Glob imports, external imports and
  aliases that shadow a scalar type or a local item stay refused. The manifest
  lists each module file's digest, its `original_sha256` is the closure
  identity, and each binding names its module file. Unit enums must stay at
  the crate root, because the pinned Verus aborts on their structural-equality
  derive inside a module
  ([VI-0042](verus-issues/VI-0042-structural-derive-in-module.md)).

Profile three lowers with version four and contract version three. Its manifest
names the truncating rule in its arithmetic qualifier. Profiles one and two still
refuse specification division and remainder and the widened body forms, and
their emitted bytes are unchanged. Executable-body division was already admitted in every profile, and
Verus proves its divisor and overflow obligations.
