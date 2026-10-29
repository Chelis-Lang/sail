# Compiler filename token refinement

Status: production uses the shared filename predicate. The combined
Holding/filename extraction passes its complete proof matrix and required native
Linux runtime checks. Fresh current-main checks execute at `00382159`; exact
compiled inputs and reviewed direct call paths retain the earlier proof/corpus
receipts with their original sources. No complete service theorem is claimed.

## Source and scope

The extraction retains compiler-read schema2, receipt version 10 and the
write-ahead origin rules from #572. It takes the filename kernel and six
executable guard mutations from input checkpoint `e50ddd6e`; it does not import
that checkpoint's decoder, DOM, process or provider changes.

`source_inputs::dependencies` checks every newly decoded file rule's proposal
with the shared predicate. The complete document-layout check and original
decoding remain. An unsupported token invalidates the entire inventory;
`update` persists `covered = false`, later loads remain incomplete and managed
currentness is Unknown. Native regressions start with a valid prior inventory,
then mix valid and unsupported rows: neither the previous covered state nor a
valid subset can authorize reuse. Per-package maps cannot bypass the separate
currentness guard. The invocation and freshly produced verdict remain
independent of this conservative refusal.

Empty, NUL, carriage-return and newline names are outside the token contract.
Carriage return conservatively disables reuse even on Unix where it is a valid
filename byte. Spaces, escaped backslashes and trailing backslashes retain
literal identity; no filename bytes are normalized. Existing version-2 source
records and receipts retain their existing literal-path/currentness checks and
are not retroactively certified by this theorem. No record version changes.

The checker captures and digest-binds the native decoder/admission source as an
unproved native caller. Changing proposal identity or refusal propagation needs
fresh review; a digest is not a proof. Direct stock Verus checks the shared
predicate, not the filesystem, compiler encoder or document parser.

## Complete qualification

The current union has 238 refinement cases. Its query root checks 295 verified
checks and all 480 functions; inputs check 59/96; authority checks 188/318;
the parser checks 133/208. Parent #543's earlier parser context was 129/204.
Every prior function and ordinary negative control remains required.

| Gate | Best completed evidence |
| --- | --- |
| Refinements | All 183 query/input/authority cases execute at `58edf644`; all 55 parser cases execute at `838706f0`. Actual generated driver/dependency hashes establish exact correspondence to the combined source. |
| Abstract models | All 35 cases execute at `838706f0`: four complete positives and 31 ordinary broken controls. |
| Native Linux | `838706f0`: complete workspace gate, 1,207 tests; 51 solver/fault tests; full E2E including extra; PR conformance all pass. |
| Full fast conformance | Four arms each match 147 units and 3,451 functions; zero different, unstable or infrastructure cases; 11 recorded rlimit-drift units, no skipped projects. |
| Standard conformance | Four arms each match 159 units and 13,166 functions; zero different, unstable or infrastructure cases; 17 recorded rlimit-drift units, no skipped projects. All 18 mutants and ordinary/forced reverts match. |
| Current-main refresh | `00382159`: exact 238 refinement and 35 model compiled-input correspondence; fresh workspace 1,216 tests / 54 skipped, solver 51, full E2E + extra, four PR arms each 33 units / 311 functions and all 18 mutants plus both reverts pass. Direct-arm source audit checks 193 unchanged entries; all 6,107 tracked corpus files in nine pinned repositories match their Git blobs. Toolchain/runtime inputs and CI images remain unchanged. |
| Added validation cost | Unmodified native demo3 completes all 60 samples with expected outcomes. Original auxiliary postcheck fails on valid Cargo-no-op output; a separate read-only audit passes without rerunning or changing any sample. See the bounded results below. |

Completed native scope uses a c7i.4xlarge in us-east-1, the normal CI images,
Sail Rust 1.99.0, pinned stock Verus `0.2026.10.18.7d82994` with Rust 1.99.1,
and selected fork variant `d6096d5bd495`. No fresh macOS proof or native
qualification is claimed here; the central design retains its two-platform
requirement. Stock calls retain whole-root scope,
one thread, default resources and the 120-second deadline. Signals, syntax
errors and resource failures never count as successful broken controls.

The owning 183-case refinement and 35-case model runners retain actual
compiled-program hashes and canonical Git source, allowing independently checked
reconstruction; they delete their original temporary files. The 55 parser calls retain their actual compiled
files before execution and verify unchanged hashes afterward. E2E and
conformance select different normal build graphs and have distinct executable
hashes; their receipts identify the actual binaries used in each scope.
Successful output-loop fixtures delete their temporary per-command payloads.
The E2E receipt retains complete phase stdout/stderr; separately indexed native
managed/repair records are not a claim that those deleted payloads survive.

Closed current-main packet:
`experiments/2026-10-28/acceptance-proof-union-00382159-native.tgz`,
1,211,791,619 bytes, SHA-256
`5c870d7d664edb9ba3f63b106fa8f01610c51be14398068f77562b21244401e8`.
Private PUT and complete fresh GET/hash comparison pass. A full owner byte
audit also checks every retained archive payload. Its inventory preserves the
actual passing commands and both failed auxiliary controllers, separately from
the read-only cost audit. Regenerable build/VCS trees and ephemeral endpoints
have explicit exclusion records; no recorded timing sample is excluded.

The prior whole-proof/direct-corpus packet is
`experiments/2026-10-28/acceptance-proof-union-8387-native.tgz`,
188,395,930 bytes, SHA-256
`c9afa50d31c5e6b10673f20c394843a6bcccf8c08e9d1ca4f527991afea58563`.
Its private PUT and complete fresh GET/hash comparison pass. It retains the
actual 183 + 55 proof calls, 35 model calls, full direct corpus runs, failed
prefixes, source bundles and independent bounded/Holding audits.

The final restack uses main `b56889f1` and parent parser checkpoint
`c04fcee3`. Main adds installer directory-mode handling, a release-layout
diagnostic and documentation. All runtime Rust, Cargo/lock/toolchain, proof
program/checker/native-binding, fixture, CI-image, corpus and benchmark inputs
remain unchanged. The changed installer suite passes 12/12 on exact current
source; documentation links pass. Actual native binaries and all runtime/cost
results retain their recorded `00382159` / `8797ccd4` identities. No new
unaffected native run is attributed to this restack; current-head CI is required.

## Bounded native cost

The unmodified `scripts/perf-loops.sh demo 3` rotates plain Verus, main
`8797ccd4` and candidate `00382159`, with fresh projects/targets and Sail caches
for each mode/round. Installed tools, dependency downloads and OS page caches
remain shared. Both Sail arms use the actual normal minimal release recipe
`cargo build --release --locked -p sail-cli -p sail-shim` on the same idle native
c7i.4xlarge/us-east-1f. No `CARGO_TARGET_DIR` override or emulation is used.


Wall seconds, median (min–max), three samples per cell:

| Phase | Plain Verus | Main | Candidate |
| --- | ---: | ---: | ---: |
| cold | 36.644 (35.260–37.599) | 34.571 (33.867–34.792) | 34.346 (33.854–34.381) |
| warm | 0.122 (0.119–0.124) | 0.296 (0.295–0.298) | 0.296 (0.294–0.302) |
| edit | 0.855 (0.851–0.885) | 1.119 (1.116–1.134) | 1.107 (1.102–1.117) |
| fail | 0.854 (0.834–0.870) | 0.875 (0.874–0.890) | 0.861 (0.860–0.868) |
| fix | 0.885 (0.885–0.888) | 1.146 (1.137–1.175) | 1.138 (1.132–1.152) |
| warm2 | 0.123 (0.119–0.124) | 0.285 (0.281–0.288) | 0.288 (0.287–0.292) |

All 54 CLI samples have matching expected exits; only the nine intentional
failed-proof calls exit 101. Non-warm proof summaries and postcondition errors
match all three arms. Warm phases correctly emit no prover summary and complete
as Cargo no-ops; they do not claim a fresh proof or settled replay latency.
All six retained candidate MCP replies report Current, six verified functions
and zero failed/unknown/provisional functions. Full source/toolchain/pinned
Stock/engine/binary inventories match before and after the run.

The auxiliary controller's nonempty-summary assertion was wrong for warm Cargo
no-ops. Its FAILED result remains unchanged. The corrected read-only audit
requires matched empty summaries plus successful/no-error Cargo output on those
phases, retains every original sample and checks all other expected proof
outcomes. The unmodified benchmark command itself exits zero. These three-round
toy results combine parser, Holding and filename changes; they cannot isolate
individual validation cost or establish a service/corpus performance gain.

| Date | Source | Attempt | Result |
| --- | --- | --- | --- |
| 2026-10-28 | `a804c7b1` | Bounded setup attempts 1/2 | Output-directory and source-capture/symlink setup refusals; zero verifier calls. |
| 2026-10-28 | `a804c7b1` | Initial whole-root discovery | Query positive passes 286/468; provisional expected count is wrong. Preserved as a failed inventory attempt, not proof qualification. |
| 2026-10-28 | `9eb65c5f` | Combined bounded matrix | Query passes 295/480; two input witnesses exhaust default resources. Checked erased guidance fixes them without changing contracts or resources. |
| 2026-10-28 | `7b3f7597` | Bounded matrix | All 19 proof cases and nine native filename tests pass; complete actual input bytes and final guards retained. Independent audit passes. |
| 2026-10-28 | `58edf644` | Final setup attempt 1 | Supplementary comparison names the wrong generated filename; zero verifier calls. |
| 2026-10-28 | `58edf644` | Full refinement suite | All 183 cases pass. Outer controller stops after this successful phase when main advances; no unrun native stage receives credit. |
| 2026-10-28 | `938606fb` | Union preflight | Strict library digest refuses a declaration-order change before any verifier or native gate. The reviewed digest is updated to the actual sorted library without weakening the check. |
| 2026-10-28 | `838706f0` | Union native prefix | Proof transfer, parser55, models35, workspace, solver, full E2E and PR conformance pass. Outer output move fails on ownership; original and copied evidence remain byte-identical. |
| 2026-10-28 | `838706f0` | Corpus continuation | Only previously unrun full fast/standard run; both pass with terminal source/image guards. |
| 2026-10-28 | `00382159` | Current-main native prefix | Proof input transfer, workspace, solver and full E2E + extra pass. Auxiliary retention wrongly expects a failure-only directory on success; its failed result stays indexed. Actual persistent managed/repair records are copied unchanged. |
| 2026-10-28 | `00382159` | Current-main continuation | Only unrun PR conformance and the exact standard mutation tail run; all pass, with source/image guards and no omitted mutation. |
| 2026-10-28 | `00382159` / `8797ccd4` | Native demo3 | All 60 original measurements complete with expected outcomes and unchanged source/tools/runtimes. Auxiliary postcheck fails on legitimate empty warm prover output; corrected read-only audit passes on the same complete data. |

The theorem concerns exact encoding of supplied bytes. Complete compiler input
discovery, physical filesystem/process observation, native caller semantics and
the service theorem remain open obligations in the
[central design](../design/evidence-acceptance-core.md). Matching corpus outcomes
include expected failures; they do not mean every compared function is proved.
