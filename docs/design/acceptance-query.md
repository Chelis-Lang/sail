# Query association proof component

**Status: independently checked specification model; no production acceptance
path uses this model.** This is the first query-association component of the
[evidence-acceptance design](evidence-acceptance-core.md), tracked by
[#346](https://github.com/Chelis-Lang/sail/issues/346). It changes no runtime
behavior, crate dependencies, solver inputs or corpus entries.

## Checked statement

[`proofs/acceptance/query_association.rs`](../../proofs/acceptance/query_association.rs)
proves `executed_reply_has_exact_binding` by induction over an arbitrary finite
sequence of modeled session actions. Each delivered reply's binding equals
the requested command's binding at that position: solver identity, mode, all
raw input bytes through the command endpoint, and that endpoint. The proof
assumes neither the final invariant nor mathematical truth of an SMT answer.

Supporting proofs establish these properties:

- Input fragments preserve the exact bytes and extend a live solver's input
  with those bytes. Fallback starts with the full raw prefix, including bytes
  already received beyond the latest parsed command.
- Independent command/reply arrivals cannot shift pairing. A pair consumes
  the same position on both sides; an absent side consumes nothing. Response
  buffering preserves every received byte across supplied framing boundaries,
  including whitespace.
- Publication retains the ordered command/reply association. A non-clean,
  stopped, incompletely paired or unequal-length session publishes no entry.
- Cache serving requires an admitted entry with the complete binding of the
  next request. A same-shaped reply for another binding cannot satisfy this.
- Fallback suppresses exactly the replies already served from cache, then
  forwards later complete responses once. There is no return to cache-serving
  mode. Late replies from a failed connection cannot be served from cache.

`positive_pairer_arrival_and_publication` exercises reply-before-expect
arrival and clean/unclean publication. `positive_cache_then_fallback`
establishes a cached first response, exact two-byte replay, suppression of
that first replayed response, delivery of the second response, and rejection
of later cache events. These witnesses exercise acceptance branches instead
of satisfying the theorem only through empty or failed executions.

The model is unbounded in action count and byte-sequence lengths. It is an
inductive specification proof, not bounded enumeration or an executable shim
replacement. It does not prove termination, stock diagnostic parity,
resource-limit policy, performance or solver soundness.

## Concrete refinement still required

The code inspection starts at main source `1b8bfd31` and includes the streamed
transcript-digest change from #358. The production boundaries below are actual
code, while their correspondence to this model remains to be proved.

| Model element | Production boundary | Remaining obligation |
|---|---|---|
| `Binding` | `sail-key::PrefixHasher::{new,update,key}` | Solver/mode domain separation, framing, every raw byte and endpoint agree with the exact-byte model. Digest equality needs the declared cryptographic assumption; no hash-injectivity axiom appears here. |
| `input_fragment`, `observe_command` | `cached::run`, `observe`, `CommandSplitter`, `Session::expects_output` | Actual output-producing commands appear once, in order, at their real endpoints. Unsupported protocol changes leave cache serving/recording safely. |
| `ReplyBuffer` | `cached::output_side`, `ResponseSplitter` | Supplied boundaries are whole actual responses, preserve whitespace and correctly handle a final incomplete reply. Byte accounting does not prove delimiter recognition. |
| `Pairer` | `cached::Pairer::{reply,expect,drain,finish}` | Both queues represent the ordered process stream. Held checks, effort/statistics annotations, errors and `Expect::Stop` preserve the key/reply pair. |
| `published` | `spawn_storer`, stderr monitoring and process termination | `clean` follows actual normal completion, complete output, successful forwarding and the existing quiet-session policy. |
| Cache/fallback transitions | `cached::run`, `start_solver`, `output_side`, client error handling | Launch and writes preserve identity and bytes. Suppression offsets, threading, channel failure, partial writes and integer bounds refine modeled counters. Failed clients are never reused. |
| Admitted cache | Daemon protocol, local persistence, bundled/shared admission | Every initial/recovered entry has an ordered trusted origin for its complete binding. Connect pairing/publication to all store writers and readers. |

The session model takes an **admitted** store as input; initialization does
not validate arbitrary disk entries. The pairing model proves an ordered
origin for entries from its queues. These models do not yet prove persistent
publication, recovery or store admission. Thus the checked statement does not
establish truth of an arbitrary cache entry with a plausible reply shape.

## Assumptions and trust inventory

This proof root contains no `assume`, `admit`, `external_body`, assumed lemmas
or unchecked external specifications. Stock Verus, ordinary vstd trusted
definitions, pinned Z3 and the verifier's Rust toolchain remain trusted.

Before this component supports a production acceptance theorem, its interfaces
must establish or explicitly retain all of the following conditions:

1. Solver identity/mode describe the actual launched process and immutable
   runtime. A path/hash observation followed by launching a changed executable
   is not covered by a datatype.
2. Command classification and response framing preserve actual stream order
   and completeness. Same-looking replies provide no independent association
   evidence; queue position supplies the association.
3. Actual process/output observations justify `clean`; an unsupported boolean
   or successful parse is insufficient.
4. The local store starts valid or is validated into the admitted-store
   relation. Every writer/recovery path preserves it. Shared and bundled
   writers need an explicit trusted-writer contract or independent
   certificates; signatures and checksums do not prove SMT answers.
5. Concrete counters, queues and I/O failures implement the transitions.
   Unsupported observations cannot mint successful evidence. Abstract proofs
   do not verify their parser or I/O adapters merely by referring to them.

These conditions are outstanding interfaces, not axioms inserted into the
proof to make it pass. Integration must discharge or name them in the
composed theorem's inventory.

## Direct check and controls

The direct check uses upstream stock Verus `0.2026.10.18.7d82994`, revision
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust `1.99.1`, and pinned Z3
`4.16.0` from `corpus/toolchain.toml`. The macOS arm64 archive and bundled
Z3 hashes were validated against that manifest. Proof source SHA-256:
`0d2525d0137e70d80435bfdaeb0234550c84e93f3a1ff9d7022b53fe51fc07ec`.

After clearing inherited Sail/Verus settings, injected Rust flags and compiler
wrappers, invoke the extracted validated upstream runtime directly:

```sh
RUSTUP_TOOLCHAIN=1.99.1 VERUS_Z3_PATH="$stock_runtime/z3" \
  "$stock_runtime/verus" proofs/acceptance/query_association.rs \
  --no-cheating --output-json
```

This invokes neither Sail nor its fork, shim, daemon or managed receipts.
Local result: normal exit 0, `success: true`, `verified: 24`, `errors: 0`,
`is-verifying-entire-crate: true`. The coordinated runner and complete proof
inventory remain a separate integration deliverable. Native Linux checking
and the repository gate remain required before publishing a component PR;
this local model check does not replace them.

Negative controls replace unique text in a temporary copy and run the same
complete invocation:

| Control | Exact needle | Replacement | Required failure |
|---|---|---|---|
| Pair a later reply with the first command | `binding: s.commands[s.paired as int],` | `binding: s.commands[0],` | `pairing_preserves_order` fails. |
| Suppress one reply beyond those served | `delivered: if s.seen < s.skip {` | `delivered: if s.seen <= s.skip {` | `replay_suppresses_exactly_served_replies` fails. |

Both return nonzero with failed obligations. They alter actual transitions
and add no contradictory assumptions. The coordinated runner must separately
reject omitted modules, replaced runtimes and narrowed verification scope.
