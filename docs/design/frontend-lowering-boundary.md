# AST-to-SST reuse boundary audit

**Status:** source audit of fork `47fb3be6fc73dd9eec77ffec31866b3157f430df`;
no complete reusable operation, restoration implementation or performance
acceptance. This is the bounded operation review required by
[ADR 0052](../decisions/0052-warm-frontend-reuse.md), not a new cache policy.

## The actual operation

`source/rust_verify/src/verifier.rs::verify_bucket_outer` freshly prunes the
crate, constructs `Ctx`, calls `ast_to_sst_krate`, then lowers SST through
`poly_krate_for_module` and verifies it. The proposed reuse boundary therefore
cannot be described as a function of an AST hash alone.

`source/vir/src/ast_to_sst_crate.rs::ast_to_sst_krate` accepts mutable `Ctx`,
a diagnostic reporter, the bucket's function set and the pruned crate. It
lowers visible functions, elaborates them in the global call graph's strongly
connected component order, rewrites recursion, elaborates bit vectors and
populates `ctx.func_sst_map`. It can fail before producing a crate. A cache of
only its returned `KrateSst` omits observable effects and downstream context.

## Input and effect inventory

This table identifies concrete dependencies and missing coverage. It is not
an exhaustive transitive read/write certificate. Fields of `Ctx` and
`GlobalCtx` that have not been proved irrelevant must remain inputs or refuse
reuse; this review does not authorize dropping them.

| Material | Concrete consumer or effect | Requirement for a future shadow boundary |
|---|---|---|
| Complete typed crate and imported declarations | `ast_to_sst_crate`, `ast_to_sst_func`, `ast_to_sst::get_function`; visibility, contracts, attributes, trait methods and bodies affect lowering | Preserve all ordered semantic data, identifiers, flags and span relationships; a source text hash alone is insufficient |
| Bucket function set and module | `function_to_sst` chooses owning-bucket checks; visibility controls which bodies and axioms lower | Bind exact set, module, crate identity and selection; test changes to focused verification |
| Function/dependency order | `ast_to_sst_krate` uses global SCC representatives and SCC node order; elaboration receives earlier spec definitions | Bind graph and ordering or prove their irrelevance; do not sort an observation to create hits |
| Context assembled during pruning | `Ctx::new` receives monomorphic types, spec-function types, reached dynamic traits, builtins, function-definition types and resolved types | Record all constructor arguments and derived maps, not only the pruned crate |
| Context semantic maps | Function, datatype, opaque type, trait/implementation, reveal-group and invariant maps in `context.rs` | Establish transitive reads in lowering and called helpers; incomplete instrumentation refuses reuse |
| Architecture and interpreter policy | `ast_to_sst::sst_bitwidth`; `sst_elaborate::elaborate_one_stm` reads `arch`, `rlimit` and `no_bv_simplify` | Include exact policy; compute-only assertions, bit vectors and interpreter exhaustion are mandatory controls |
| Function checking modes | `function_to_sst` switches `ctx.fun` for ordinary, decreases, recommends and safe-API checks | Account for entry/exit state, including error paths; restore no partially executed context |
| Produced function map | `ast_to_sst_krate` inserts every elaborated result into `ctx.func_sst_map` | Compare and restore the complete ordered effect on downstream context, not just the returned crate |
| Diagnostics and warnings | `ast_to_sst` and `sst_elaborate` emit through the reporter; interpreter evaluation can emit diagnostics and log output | Capture ordered messages, levels, labels and current source locations; unsupported logging/reporting modes refuse reuse |
| Trigger and identifier state | `triggers::build_triggers` appends chosen triggers; `def::new_internal_qid` may populate the global quantifier map; global contexts later merge diagnostic state | Trace which effects occur in this particular operation and preserve their pre/post state; a whole-context Debug hash is not a certificate |
| Naming and later AIR state | `NameCtxt` makes shared crate-name disambiguation decisions; later SST-to-AIR code updates quantifier counters and string collision tables | Prove that restoring the lowering artifact leaves precisely the fresh entry state for subsequent passes; do not accidentally include later effects in a purported AST-to-SST output |
| Span and source-map objects | `vir/messages.rs::Span` skips `RawSpan` in serde; its Debug prints `ANY`. `Spanned` Debug prints a location string. `rust_verify/spans.rs::SpanContextX` owns local/imported source positions and span allocation state | Neither current serde nor Debug captures a complete diagnostic artifact. Retain opaque/current bindings or refuse; packed positions and stable crate IDs are not file-content hashes |
| Failure result | Every lowering stage returns `Result<_, VirErr>` and can emit before returning an error | Compare the error, all prior effects and current diagnostics; matching successful SST is insufficient |

## Decision for this implementation

Persistent frontend restoration remains disabled. The existing raw AST/SST
capture cannot qualify this operation: it lacks a complete context/effect
boundary and opaque/current span binding. Increasing its record cap or
repeating its two-project timing run would not fix those omissions.

A future measurement implementation must first define a versioned envelope
containing complete inputs, returned value or error, context effects and
ordered diagnostic effects. It must execute the ordinary operation on every
attempt and compare that entire envelope on proposed hits. Any unrepresented
read, callback, output or source mapping makes the attempt unsupported.
Only a successful complete shadow result can justify restoration work.
Its key construction and restoration costs count against ADR 0052's 25%
median edit-cycle gate.

The expanded corpus supplies controls for this work: temporal-logic contracts,
imported arithmetic proofs, generated dependency graphs and passing/failing
edits, alongside the ordinary fast/standard corpus. These are available
coverage, not evidence that a future compiler cache has passed them.
