# 0076: Explicit flat Rust source profile two

Status: implemented candidate; native backend qualification remains separate.

## Decision

Keep neutral source profile one and its lowering versions one and two unchanged.
Add an explicit profile-two submission selected by `Submission.profile = 2`.
Omitted profile fields continue to mean one and serialize without a new field.
The service advertises supported profiles separately from source-job protocol
version one. Clients refuse profile two before submission unless advertised.

Profile two admits nongeneric unit enums deriving exactly `Debug`, `PartialEq`
and `Eq`; flat named structs deriving exactly `Debug` with direct Boolean,
fixed-width integer or admitted unit-enum fields; shared references to these
values; and mutable scalar references. Results remain owned values. File-level
`forbid(unsafe_code)` and existing literal passive attributes are admitted.
Payload enums, explicit discriminants, arbitrary derives, trait implementations,
aliases, generics, nested records and executable calls remain unsupported.

Repeated distinct `satisfies` identities share one conjunctive guarantee.
Each function/requirement pair receives its own binding to the same actual
function obligation and guarantee digest. A proof establishes the conjunction;
it does not identify which requirement an aggregate failure refutes. Oyster
continues to establish each contract-to-requirement relationship independently.

Lowering version three records contract profile two. Retained version-one and
version-two manifests use their original profile-one projection and bytes.
Entry and final mutable-reference projections remain distinct; `old` captures
are eager scalar computations. Enum state can be captured as Boolean equality.

Runtime postconditions borrow the actual return value, so checking a flat
result does not add `Copy` or `Clone` to the application. Equality borrows both
operands. Nominal equality relies on the exact admitted derived enum equality;
the complete source admission excludes user-written trait dispatch. A runtime
check alone does not establish that source admission or a universal proof.

## Consequences

The API and shared parser own source selection and nominal metadata. Callers
provide no compiler, solver, proof code or configuration. Unsupported scope and
unknown backend outcomes remain explicit. The existing literal mutation policy
supports only single-requirement scalar-return targets; broader shapes remain
unsupported rather than receiving invented aggregate classifications.

The bounded fork/stock diagnostic preserves original source, emitted shadow,
mapping, stdout/stderr and literal solver logs. UNKNOWN remains inconclusive,
including when a verifier failure or a model follows it. This decision changes
no default verifier mode and does not qualify native mutation kills.
