# Input-prefix and delivery qualification

Status: the complete `prefix` positive passes at `14d37ed8`: 182 verified,
zero errors and 264 function-inventory entries at unchanged default resources.
The manifest locks that observed inventory. All 37 negative controls and
Cargo/native/conformance/cost qualification remain pending. Historical
compatibility counts are not current results.

| Source | Date | Attempt |
|---|---|---|
| `369fb802` | 2026-10-29 | Whole-prefix positive FAIL: 181 verified, one failing function, 264 inventory entries; `command_head` lacks two lemma calls' slice-length bound. No timeout/resource/VIR error; final source/runtime/tool guards pass. |
| `14d37ed8` | 2026-10-29 | Whole-prefix positive PASS: 182 verified, zero errors, 264 inventory entries; all 17 original witness/factory/bridge functions remain present. Default 120 s, one thread, no cheating; final source/runtime/tool guards pass. |

The closed failed raw packet has SHA-256
`05bc1173a802854b94a70c451cec7ff996f9526749cab9d2320c23cfccc03e19`
(private key `experiments/2026-10-29/acceptance-prefix-369fb802-discovery.tgz`).
An independent audit checks the canonical source, all 316 generated programs
and the raw result; retained runtime/tool maps are reviewed rather than
independently rehashed from runtime payloads. The complete new positive packet
has SHA-256 `6801d00079b1a3683492594ff18989b3fea2575bfbc7587b8706382f6e336ef0`.
Its actual raw result supplies the 182/264 inventory; all 37 negative controls
remain unrun.

The candidate now adds proof-only assertions connecting the mathematical view
to the actual slice's `usize` length. Contracts, executable bodies, controls
and resource limits are unchanged. The complete positive checks these assertions;
ordinary negative controls remain the next proof gate.

The candidate extracts the original key/prefix/delivery cores from `e07c9f39`
onto the command/persistence stack originally at `2baa1336`, now restacked
on `2ac97a50`. The new raw-input module
reuses the current parser grammar and actual lexer. The original head contract
is retained, and the complete positive checks its proof-only completeness
lemmas at ordinary default resource limits.

The recovered historical mutation descriptors cover key fields/domains,
counter overflow, feed ordering, cloning, typed construction, original raw
intervals, same-owner operations, actual writes, permanent refusal, mode-bound
seeds and frozen delivery. Current parser/head controls remain registered on
their owning roots. All complete old programs must be compared before any
source-specific proof reuse is claimed. All selected new controls must fail
ordinary verification, without timeouts, resource failures or narrowed scope.

Lightweight source checks at `20c261d4` pass 76/76 tests. Canonical Git
reconstruction also finds all 278 inherited complete programs (six positives
and 272 negatives) byte-identical to `2baa1336`, including every compiled
dependency. This is source correspondence, not new Stock execution. The new
prefix control suite and changed native callers remain unqualified.

The native source review retains the existing parser, response shape checks,
held-response release/drain, complete output side, retained reply bytes and
replay-consistency checks. These source comparisons are review evidence, not
execution. Required future gates are direct Stock models/refinements, native
workspace/solver/full end-to-end checks, fast and standard differential
conformance, and an actual native measurement of added validation cost.

The proposed graph decision is [ADR 0089](../decisions/0089-checked-input-prefix-delivery.md).
No merge, runtime selection, universal origin proof or release readiness follows
from extracting these sources.
