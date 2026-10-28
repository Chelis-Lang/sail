# Shared stream parser qualification

Status: complete positive Stock discovery passes, but the full control suite
is refused at the default resource limit. Native compatibility, cost and
conformance are unrun. The candidate adopts the new native path when built; owner acceptance
and completion of #371 are not claimed. See the [design](../design/checked-stream-parser.md).

| Scope | State |
|---|---|
| Base | Main source `67b8b95d` / canonical `9dc2038c`, bugfix base `ef655f82`, reviewed model cut `03665cd9` |
| Source operations | Actual splitter/token lexer and exact grammar extracted from the frozen source lineage; command/head/session/cache features excluded |
| Existing proof programs | Source-only comparison verifies all 25 captured programs against model baseline `03665cd9`: query 134/263, inputs 54/91, observed authority 174/303; no parser qualification inferred |
| New parser root | Actual complete positive at `96c2c4f0`: 127 verified / 201 function entries; all 54 controls preserving the 51 original fault behaviors remain pending |
| Qualification admission | Locks the actual 127/201 inventory; the pending-discovery refusal remains tested |
| Native callers | Candidate default splitter/lexer paths now delegate to shared implementations; adapters and command/sexpr consumers are bound/unverified, not Stock-compiled |
| Remaining gates | All 54 parser and 22 baseline controls, strict native/Linux/Mac compatibility, solver/scoped standard conformance and narrow measured parser cost |

Earlier larger-root receipts apply only to their named frozen sources and
scope. They do not qualify this module relocation, new root or main-based
adapter. Native source/target closure, source erasure and private-constructor
checks remain required. The source extraction restores the inherited missing
Token::text must-use annotation (#533); no native diagnostic reproduction is
claimed for that metadata correction. Full-transition/chunk negative fixtures
retain all original proof groups while preserving their runtime fault effects
(#537). There is no timeout or proof-scope relaxation.

The exact-interval guard refuses three combined descriptors whose runtime
changes cross proof boundaries. Each now has two independent canonical parts,
including the second-Stray/outer-arm effects. Source tests check every part's
unchanged contracts, loops and proof groups, and raw composition in both orders
exactly reconstructs each original combined `after` body. The original three
descriptors remain control-source fixtures, not qualified controls. All six
parts and the other 48 controls still need ordinary whole-root Stock execution;
no kernel operation, return join or native adapter changes for this decomposition.

Before the intentional authority-model update, the feature's 25 captured
programs compared byte-identically to original main `67b8b95d`. The current
feature likewise leaves all 25 programs byte-identical to reviewed baseline
`03665cd9`, whose actual authority inventory is 174 verified / 303 function
entries. Its model suite has 35 cases. These are independently observed base
results, not a new parser proof or a claim that the model cut has merged.

Explicit cursor/output-capacity and native lifetime obligations remain open,
as do physical EOF/origin, original-owner/queue, Session and solver relations.

The owner-approved validation policy belongs to the prerequisite; this
feature changes no CI job placement or mathematical check. Draft publication is authorized before the pending gates, not merge or
owner acceptance of the candidate native adoption.

Source review of the first feature draft finds a missing internal lock edge
(#544): smtlib Cargo adds acceptance but the package lock initially lists only
proptest. The source correction records that same internal edge in Cargo.lock
and adds a drift guard, without any version or third-party dependency change.
This is source-confirmed, not a reproduced native compiler failure; all pending
native/proof/solver/conformance/cost gates remain.

The first standalone discovery at `38ce57a0` retains the three complete base
positives (134/263, 54/91, 174/303), then refuses the parser root at its
`StreamSplitter::push_items` loop under the unchanged default resource limit.
The raw result is 126 verified / one resource error with 201 function entries;
these are failed-attempt diagnostics, not a discovered positive inventory.
Final source/runtime guards pass. The closed failed archive has SHA-256
`aafa67f2c4e61d3d19d34a1529594d513d82451eb0f27c2e586d4175e33e2b07`
(317,343 bytes). A first guidance attempt at `5f310427` is refused during Verus translation:
`hide` is a function header and cannot sit inside the existing proof envelope.
Its raw diagnostic and final source/runtime checks are retained, with no parser
proof credit. Replacement erased guidance exposes the empty scan case and
records each exact emission-capacity/inventory cut; the existing scan lemmas,
all runtime operations/contracts/loops and all 54 fault effects remain. The
replacement guidance passes the complete root at the original limits.

At `96c2c4f0`, all four complete roots pass with pinned Stock
`0.2026.10.18.7d82994`, Rust `1.99.1`, Z3 `4.16.0`, `--no-cheating`,
one thread and the unchanged 120-second timeout: query 134/263, inputs 54/91,
authority 174/303 and parser 127/201. Actual parser verification takes 7.211 s
on a native c7i.4xlarge in us-east-1. Complete source/runtime/image checks pass;
this is a positive proof result, not a parser cost measurement or native/control
qualification. The manifest records only the actual observed inventory. The
late-build-script guard fixture now uses that real inventory while retaining
its synthetic verifier-I/O attack and every refusal assertion.

The closed positive-only archive is
`experiments/2026-10-27/acceptance-parser-main-positive-closed-20261027-v1.tgz`,
SHA-256 `0c5c7832878ad853398c59e8db8f613ed0c10690818af713677a79ee90e3a15e`
(949,418 bytes). PUT/GET is byte-identical. Local audit checks all three nested
hashes/sizes, 301 canonical Git blobs, all four actual drivers/file sets and
full function inventories/default resources; independent lead audit is pending.
The archive retains both preceding failed attempts.

The first full-matrix launcher at `55f4402b` exits 125 before any Stock call:
its fresh read-only checkout lacks the `/work/target` mountpoint. All 80 cases
are unrun in that attempt. The separate normal retry creates only the ignored
mountpoint, keeping the frozen source, assertions and default resources.

That retry at `55f4402b` terminates before completing the suite: all four
positive roots and 25 ordinary negative controls pass, then
`split-list-original-byte` correctly fails its original-byte assertions but
also reaches the default resource limit in the unrelated `model_scan_join`
lemma and `push_items` loop. Its result is 124 verified / three errors;
it earns no ordinary-control credit. Fifty later calls are unrun. Native
compatibility, cost and conformance remain unrun. The closed archive
`acceptance-parser-main-closed-20261027-v2.tgz`, 2,263,073 bytes, SHA-256
`5e464ed609c664b3a9254d8f3a8d2a8f80e15cc7f16c76090b885255c6a22387`,
retains the original raw results, both launch attempts and the earlier
positive-only archive. Regenerated canonical programs are explicitly
identified as reconstruction; the normal runner deleted its temporary inputs.

The delivery retry adds only checked intermediate sequence/scan equalities
inside the existing erased `model_scan_join` proof. Runtime operations,
contracts, loop obligations and every control effect are unchanged. No proof
resource, timeout, inventory or acceptance condition is relaxed.

That bounded retry at `502a1968` is also refused. The complete parser positive
passes with 127 verified / 201 function entries. The original-byte control
correctly fails its two assertions, with 125 verified / two reported errors,
but the `push_items` loop still reaches the default resource limit. The added
checked scan-join cuts eliminate that lemma's separate resource refusal;
they do not qualify the control or the full suite. The canonical 80-case
suite is deliberately unrun after this refusal. No further proof expansion,
native benchmark or corpus acceptance is inferred. The PR remains draft.

The native c7i.4xlarge/us-east-1 run uses core image
`sha256:eb9f76a640e4faafea55d1089f83372c190e27651432209a64660b78591e4244`,
the same pinned Stock/Rust/Z3 as the positive checkpoint, one thread and a
120-second per-case deadline. Pre/post source, runtime and image checks pass;
the Rust compiler version is checked before execution. The fresh complete
archive `acceptance-parser-delivery-502a1968-proof.tgz`, 6,032,505 bytes,
SHA-256 `ff259847e58347a1a40924533e0874fb57042d10fc23c39f1a3c78826609cfe9`,
retains both original complete programs, raw outputs, frozen source bundle,
controller and explicit incomplete scope. Upload/readback checks pass.

Source comparison preserves all 80 drivers and all 25 baseline programs
byte for byte; only the erased splitter proof text changes in the 55 parser
programs. The entire Cargo-erased splitter source is byte-identical to
`97ed0dd3`. All 48 source tests pass. These source facts do not replace the
missing complete control, native, cost or conformance gates.
