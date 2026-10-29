# Shared stream parser qualification

The current Holding/filename union's parser root passes 133 verified checks
and all 208 function entries, including the four added filename functions. All
54 ordinary parser controls pass their required whole-root failure checks.
The combined matrix has 238 actual cases at the sources recorded in the
[combined qualification](compiler-filename-refinement.md), including 55 fresh
parser calls at `838706f0`. That record owns current union runtime/cost status.

The parent parser-only checkpoint below retains its original 129/204 inventory
and **55 fresh + 171 reused** coverage; it is not 226 fresh executions and is
not relabeled as the later union's proof. Its frozen-source full native gates
and narrow parser cost remain attributed to their actual source.
The restack onto `b56889f1` changes inherited installer helpers/tests and
documentation only; proof and runtime/cost inputs retain their executed source.
Scoped installer/link checks cover that base update, with current-head CI
required before merge.
[ADR 0086](../decisions/0086-checked-stream-framing.md) is accepted on
2026-10-29 for this bounded runtime adoption and its measured cost tradeoff. The [service-wide design](../design/evidence-acceptance-core.md)
remains open under #368.

## Parent parser checkpoint evidence

| Scope | Evidence and result |
|---|---|
| Source | Restacked base `b56889f1`; actual refreshed native source remains `7b1c2720` on `8797ccd4`, with older native/cost source `a84109cc` on `584fd14a`. No new runtime or proof execution is attributed to the restack |
| Fresh production proof | `d9da43c9`: whole parser positive 129 verified / 204 function entries; all 54 controls fail ordinarily with 128 verified / one failing function; no resource or VIR refusal |
| Inherited production proofs | 171 unique cases / 174 historical calls re-audited from closed `c0957b92` and `fefe3c73` receipts; complete generated programs, inventories, default commands, Stock runtime and Rust identity match |
| Abstract models | 35 executions at union `838706f0`: four whole positives and 31 ordinary controls; all current generated programs, all 3,873 Stock runtime files and pinned Rust match; actual raw results independently checked; no new execution at the parser head |
| Parent correspondence | All 226 production programs and native-parser bindings match from qualified `d9da43c9` through the restacked parent head; all 35 model programs also match their executed receipt. No new verifier calls |
| Frozen native Linux | `a84109cc`, c7i.4xlarge/us-east-1: workspace 1,203 passed / 54 skipped; solver 51 passed; full unchanged end-to-end + extra passed; full fast four arms each 147/147 units / 3,451 functions; standard four arms each 159/159 / 13,166 functions plus all 18 mutations and both revert controls passed |
| Current-base runtime refresh | `7b1c2720`: workspace 1,212 passed / 54 skipped; solver 51 passed; full end-to-end + extra passed (1,099.77 s); PR-fast four arms each 33/33 passed; all 18 mutations and cold/revert/forced-revert checks passed (21 rows, 1,050.02 s). No mutation evidence transferred |
| Installer/docs base refresh | Main `b56889f1`: focused installer tests (12/12) and documentation-link checks pass; all runtime crates, complete proof inputs, entry scripts, fixtures, image and pins are unchanged. Original native/corpus/cost evidence retains its actual sources; no new whole-binary identity is asserted |
| Before merge | Final head review and current-head CI. ADR 0086 and the bounded measured cost tradeoff are accepted; physical caller and service relations remain open |
| macOS | No fresh Stock proof suite or native host gate for this candidate. Portable code adds no platform-specific path; the design's two-platform qualification remains open |

## Meaning and limits

Default `CommandSplitter`, `ResponseSplitter` and `Lexer::next` delegate to the
actual shared operations. The checked relations cover original bytes, ordered
emissions, offsets, grammar, token spans and lexical failure. Checked
history/emission lemmas and explicit reveals make the unchanged recursive
predicates modular; no definition, contract, invariant, control, resource limit
or timeout is removed or weakened.

Three combined mutations cross proof intervals. Each is retained as a source
fixture and decomposed into two independent mutations. Source tests check
unchanged proof groups and reconstruct each original fault in both orders;
all six parts and the other 48 controls now fail ordinarily on the whole root.
The original combined descriptors themselves earn no canonical credit.

Native adapters, command classification, sexpr/Session consumers, representable
cursor/capacity and allocation premises, physical I/O and EOF, external byte
ownership and queue association, solver interpretation and truthful service
origins remain outside this proof. Reviewed/hash-bound callers do not prove
those relations. Model checks establish conditional model theorems, not the
complete production service. Native compatibility, corpus conformance and cost
are separate evidence.

## Native cost

The same public API harness compares baseline `584fd14a` with `a84109cc` on a
native c7i.4xlarge in us-east-1. All 20 byte/kind/offset/error/sticky fingerprints
match, including incomplete and malformed fragments. Five alternating pairs
per case retain all 200 timing samples and zero refusals.

| Whole-input operation | Baseline median (µs) | Candidate median (µs) | Ratio |
|---|---:|---:|---:|
| 59 KB commands, 4 KiB chunks | 284.9 | 282.4 | 0.99× |
| 100 KB responses, 4 KiB chunks | 295.5 | 303.8 | 1.03× |
| Same commands, one-byte chunks | 468.5 | 1,416.8 | 3.02× |
| Same responses, one-byte chunks | 608.7 | 2,253.4 | 3.70× |
| Token lexing of the 59 KB input | 121.6 | 231.5 | 1.90× |

At 64-byte chunks, median command/response costs rise 5.0%/7.9%. Tiny incomplete
streams cost 2.6–4.3× more (still low microseconds); malformed-token lexer cases
cost 1.36–1.41× more (about 16 ns extra). The complete samples and ranges are
retained. This is parser/lexer microcost, not an aggregate percentage, a service
latency benchmark or a speedup claim. Neither worse fragmentation nor lexer
cost is hidden by the bulk-stream results.

These measurements retain their actual sources. Reuse on current main requires
unchanged complete crate/fixture/profile inputs and checked tool/runtime
identities. Both checks pass at the refreshed native source. Direct corpus
reuse uses the reviewed call-path closure; the changed managed mutation path
ran freshly in full.

## Retained evidence

The closed proof packet is privately retained at
`experiments/2026-10-28/acceptance-parser-d9da43c9-proof-closed.tgz`,
SHA256 `07e93fd884bc31d81b85dbbdebe9ff9455c126342fb57421c90eec97d4d9e670`
(34,832,279 bytes; PUT and fresh GET checked). It retains all 55 actual programs
and raw results, the 226-case preflight, inherited audits/transfers and every
failed/interrupted current-session prefix. The separate native/cost packet is
`experiments/2026-10-28/acceptance-parser-a84109cc-native-cost-closed.tgz`,
SHA256 `5bbf38c304b0711a2f4789a9c0f9b8907621ecabb4dc07c1fb87988dab314039`
(746,650,903 bytes; PUT and fresh full GET checked). Its 61,275 included payloads
cover actual Sail executable variants, raw reports/streams, current and refused
native prefixes, cost fingerprints/samples and independent model/audit
companions. Its 3,732 regenerable build/IR exclusions are explicitly indexed by
hash and reason; their raw bytes are not claimed retained.

The current-base refresh packet is
`experiments/2026-10-28/acceptance-parser-7b1c2720-current-native-closed.tgz`,
SHA256 `e0661fca7b1d1a484827c27e8a3bc4aa7de09ca764a1abcef0d8bf0837004254`
(238,957,451 bytes; PUT and fresh full GET checked). All 28,405 included payloads
pass readback; 1,854 regenerable exclusions retain digest/reason. The packet
also retains all 84 refreshed mutation stdout/stderr files and all 84 original
`a84109cc` mutation streams, with separate source attribution.

Actual CLI bytes differ by normal build scope. Original end-to-end uses
`59b662c4…`, original corpus/mutations `54a6c950…`; refreshed end-to-end uses
`b8644802…`, refreshed PR-fast/mutations `2aa805e9…`. Actual originals and Cargo
fingerprints are retained. No whole-binary equality is claimed.

| Companion | SHA256 |
|---|---|
| Independent final 55 + inherited raw audit | `529be360ebdfad9b2c982866480ef4d991e80cb3027d25952f98d9f95c9116b9` |
| `d9da43c9` to current-base `7a63045b` complete program/native-binding correspondence | `ef0a3b49e6495d14661374eb80463c2aee8472f210fb55c3241b79c7374c5e6f` |
| `a84109cc` to `7f2cd6b1` Markdown-only correspondence | `2b4961fd9a84a5f693ff379c46f825acd77ac8402df73976f69291080d05d1a0` |
| Actual union 35-model execution report | `4d673cb4d7d6a5063ada3ee58a8ee423bc05e8413c1144878b18e6d415175d64` |
| Current 35-model program/runtime correspondence and raw-result audit | `04d0c687425c3392e8479a13a594e31c372023cc047af7ace2a0f760c0a82686` |
| Lead 584 → 879 direct-conformance path/source audit | `cb31c91d4de64e3070f90c87c8dd0ca0f36007ade822dc9a39a5d814072751bb` |
| Candidate-to-candidate 211-entry crate/fixture/profile/direct-path audit | `db497c90efbaec3d25139ecddfc5202a790db71ebc0e7103f6b5cdd0c4bda61a` |
| `7a63045b` to actual refreshed `7b1c2720` Markdown-only correspondence | `30e96e4e3dc0429f5c3b33acaecdd7fc93748fdb2586a8333b1e3ebf62c65661` |

These correspondence checks execute no verifier. The model receipt retains its
actual union source; it is not relabelled as fresh parser-head execution.

## Earlier attempts

Every row retains its actual source and scope. Refused or interrupted calls
receive no positive or ordinary-control qualification credit. Current proof
attempts are included in the closed proof packet above; older archive hashes
below identify their original immutable receipts.

| Source / date | Result and limit |
|---|---|
| `38ce57a0`, 2026-10-27 | Whole parser discovery refused at the chunk loop's default rlimit: 126 verified / one resource error, 201 function entries; archive `aafa67f2c4e61d3d19d34a1529594d513d82451eb0f27c2e586d4175e33e2b07` (317,343 bytes) |
| `5f310427`, 2026-10-27 | Nested `hide` rejected as function-header placement before verification; retained with the following positive packet |
| `96c2c4f0`, 2026-10-27 | Positive-only four-root checkpoint passed; parser 127/201 in 7.211 s, not a cost measurement; packet `0c5c7832878ad853398c59e8db8f613ed0c10690818af713677a79ee90e3a15e` (949,418 bytes) |
| `55f4402b`, 2026-10-27, setup | Exit 125 before Stock because the ignored mountpoint was absent; zero proof calls |
| `55f4402b`, 2026-10-27, retry | Four positives and 25 controls passed, then original-byte assertions plus unrelated scan/loop rlimit refused; 50 later calls unrun; packet `5e464ed609c664b3a9254d8f3a8d2a8f80e15cc7f16c76090b885255c6a22387` (2,263,073 bytes) |
| `502a1968`, 2026-10-27 | Positive 127/201 passed; original-byte intended assertions plus unrelated loop rlimit refused in 58.805 s, not a wall timeout; packet `ff259847e58347a1a40924533e0874fb57042d10fc23c39f1a3c78826609cfe9` (6,032,505 bytes) |
| `993bfe7e`, 2026-10-28 | Snapshot matcher rejected `appended@` before verification; equivalent erased `appended.view()` used next |
| `72edd307`, 2026-10-28 | Both new helper lemmas verified; positive loop rlimit remained, 128 verified / one error |
| `1b614970`, 2026-10-28 | Bounded positive 129/204 and ordinary original-byte control 128/1 passed; only these two calls credited; bounded packet `26d36a793429d1c3fa1f4fcba4ceeef0a0fe8acc0e00ffb2ad23389be967a6b6` (6,775,331 bytes) |
| `6aeb50b1`, 2026-10-28 | Duplicate inherited rerun intentionally interrupted after independent exact-input reuse audit; raw prefix/current program and terminal guards retained, zero new canonical credit |
| `74245adb`, 2026-10-28, setup | Missing audit mount refused before Stock; zero calls |
| `74245adb`, 2026-10-28, retry | Positive and first seven ordinary controls passed, then chunk-original-order intended failures plus unrelated outer-loop rlimit refused |
| `e894a107`, 2026-10-28 | Positive passed, original-byte again hit unrelated loop rlimit; no complete bounded qualification |
| `1250a538`, 2026-10-28 | Nested `hide` header-placement refusal before verification; zero proof credit |
| `dedf7c09`, 2026-10-28 | Original-item opacity introduced no other errors, but whole positive loop rlimit remained, 128 verified / one error |
| `d9da43c9`, 2026-10-28 | Bounded positive/original-byte/chunk-order passed; subsequent full 55 passed at unchanged resources; bounded packet `f3daacaf75857af0f028acf3cd51ade6d1868557b286f55097b86c7ee381004f` (12,876,785 bytes) |
| `d9da43c9` native v1, 2026-10-28 | Workspace/solver passed; end-to-end setup refused SAIL3140 because neutral projects were placed under repository Cargo configuration; no end-to-end credit |
| `d9da43c9` native v2, 2026-10-28 | Corrected unchanged full end-to-end passed with source/runtime guards; follow-on corpus held at a natural boundary for main update |
| `a84109cc` cost setup v1, 2026-10-28 | Read-only source cloning refused Git ownership before builds or samples; isolated clone setup corrected; no cost credit |

Earlier source corrections preserve the original `Token::text` must-use
annotation (#533), full-transition/chunk control groups (#537) and internal
Cargo lock edge (#544). Their historical filings do not substitute for native
or proof acceptance. Main's worktree-sync tooling defect is tracked separately
as #589; canonical standalone source bundles avoid it here.
