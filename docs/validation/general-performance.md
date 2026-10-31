# General performance qualification

**Current status:** implementation candidate; broader native qualification in
progress. No new release performance acceptance or frontend reuse is claimed.

The [review](../design/general-performance.md) defines the unchanged authority
boundaries. The [evaluation guide](../performance-evaluation.md) defines the
expanded corpus, explicit runtime selection and separate diagnostic controls.
The existing [v0.2.0 matrix](v020-performance.md), observer-overhead failures and
strict stock source-suite failure retain their original scope and requirements.

| Gate | Current evidence | Limit |
|---|---|---|
| Broader selection | 18 reviewable workloads; four new repository pins; Verus-TLA baseline passes (141 functions compared); hekate-math passes all 18 units (1,848 records); semi-persistent passes all three selected packages (2,430 / 909 / 29 records) | Baselines compare stock with bare stock/shim; selected-fork, managed and targeted-edit coverage remain separate |
| Run-scoped observations | Fresh-byte decode sharing, borrowed source projection basis, indexed dependency traversal and exact streamed JSON hashing implemented | Two-round broader whole-call screen completed; previous-Sail control and final closing-barrier candidate still require selection measurements |
| Linux functional controls | Complete workspace-preparation composition passes strict Clippy, 1,283 ordinary tests (56 skipped), doctests, documentation/layering/dependency gates and six evaluation controls on c7i.8xlarge / us-east-1 | Closing-barrier cut and seven-control revision require final gate |
| Algorithmic scaling | Native single-chain lookup including index construction: 4,096 nodes, 83.888 ms legacy / 1.574 ms indexed; identical closure | One component diagnostic, not repeated whole-program acceptance. JSON timing benefit is small and variable; bounded intermediate allocation is the distinct change |
| macOS | No result for this candidate | Native host gate, paired performance and release memory gates remain |
| Compiler reuse | No persistent frontend reuse implemented or selected | ADR 0052 complete-input/output shadow qualification remains open |

Raw records stay outside git. Add their source/runtime/environment identities,
scope and SHA-256 here after completed captures; do not describe a build or
matching failed baseline as a passing real-project workload.

## Compatibility findings

The initial successful baselines use stock `0.2026.10.18.7d82994`, its pinned
Z3 and Rust 1.99.1 on c7i.8xlarge / us-east-1. Verus-TLA uses scratch source
dependency adjustment; hekate-math runs its original 18 standalone proof roots.
Both arms of every passing unit exit zero with zero verification errors;
hekate-math reports 3,050 verified occurrences per arm across its roots. These
are compatibility/comparator controls, not timing acceptance.

Initial intake/component archive SHA-256:
`1b28f575e3317810594778665e628f017ab8267ad0a6fcd94eb426ff7aeecc67`.
It retains both intake attempts, the original comparator reports, selected
binary digests and all component samples, including slower JSON samples.
Source is the recorded working tree based on `715c0f5e`, before the complete
workspace preparation fix; it is not a clean-head release acceptance record.

Semi-persistent exposes a harness preparation defect: patching only the
selected package leaves conflicting exact vstd pins in other Cargo workspace
members. The candidate now discovers the complete workspace with Cargo's
offline, dependency-free metadata and adjusts each scratch member consistently
on both sides. The clean `b9466ece` rerun passes all three selected packages: containers,
abstract domains and AU, with 2,430 / 909 / 29 function records compared.
Fetched sources and proof bodies are unchanged. Selected-fork held-out
timing and proof edits remain separate gates.

The TLSF pin does not compile with the pinned vstd: `Set::new` returns an
`Option<Set<_>>`, and `Map::new` expects a set rather than its predicate argument.
Both arms report the same two E0308 errors and compare zero functions. This
is an incompatible candidate, not a successful allocator baseline. No proof
port or runtime substitution is included.

## Evaluation controls

The subsequent seven-control harness passes natively on c7i.4xlarge /
us-east-1. It additionally refuses changed or missing measurement inputs.
Two actual syscall-observer executions preserve an unset child `LC_ALL` and
an explicitly supplied `C.UTF-8` value. Their native commands exit zero and
produce complete syscall records. Archive SHA-256:
`9d3feb9245421e9cdd6e0db9fbef44ebe50cc3c54078737e275e41f83e6842f9`.
This is harness correctness, not a product speedup. The compiler operation
[audit](../design/frontend-lowering-boundary.md) is static source review;
it supplies no complete-key or restoration acceptance.

## Failed attempts

- 2026-10-31, `b9466ece` benchmark with first external loop catalogue: the four
  arithmetic edit expectations and first five temporal-logic expectations
  pass; the final refinement matches stock's compiler failure and refuses
  its expected-positive outcome. Stock's minimal reproduction
  reports E0308 (`nat` addition assigned to `int`), zero verified/functions
  and zero verification errors. The matching nat local passes a native minimal control; the real loop rerun remains pending. Do not
  count the matching compiler failure as passing edit qualification. See
  [VI-0038](../verus-issues/VI-0038-literal-addition-infers-nat.md).

- 2026-10-31, `b9466ece` diagnostic controller: perf discovery did not follow
  the installed kernel-tools directory symlink; the retry then lacked an
  explicit shipped vstd lock beside its wrapper. Both refused before a timed
  invocation. Corrected the diagnostic controller paths.
- 2026-10-31, `b9466ece` first syscall diagnostic: setting the tracer's locale
  also changed the child's `LC_ALL`, causing receipt invalidation and verifier
  work. Interrupted that capture and retained it as an invalid unchanged-call
  diagnostic. The helper now restores the original child's locale; the next
  profile sets one consistent locale for cold and all subsequent calls.

- 2026-10-31, `8f298701`: workspace preparation composition failed to compile
  because registry-adjustment logging lost its repository-root parameter
  during extraction. Restored that explicit parameter before native rerun.

- 2026-10-31, working tree based on `715c0f5e`: first native candidate gate
  failed Clippy's needless-borrow check in the benchmark CLI. Corrected in the
  subsequent native subset gate; no check or baseline changed.
- 2026-10-31, working tree based on `715c0f5e`: first intake refused all six
  selections because independently installed tools looked for a sibling shim.
  The runner now passes explicit executable paths; the rerun retains all units.
- 2026-10-31, working tree based on `715c0f5e`: semi-persistent's three package
  attempts refused Cargo resolution due to incomplete scratch workspace pin
  adjustment. TLSF reports matching compiler errors. Both remain explicit.
