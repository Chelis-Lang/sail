# Unscheduled build-script cleaning

## Status

The joined #215/#174 implementation is merged in PR #226 after macOS/native
Linux qualification, independent review and all required delivery CI. Exact executed
`7411f458` contains output `354d2995` plus the scheduling memo on #225 executable
source `17464f16`. Accepted main `d7b30b88` differs from that baseline only in
three Markdown files. The delivery rebase preserves all non-Markdown bytes and
modes from `7411f458`; original `643ffd2e`, `f217785f` and PR #197 stay separate.

| Joined gate | Actual scope and result |
|---|---|
| Source and scheduling controls | Independent review CLEAN; all five schedule units pass before the native comparison |
| macOS host | 759 tests pass, 35 skips and all-target workspace Clippy on `7411f458` |
| Native workspace gate | PASS, 83.09 s |
| Solver, all nine E2E phases plus extra, PR and full FAST | PASS: 9.09 s / 640.94 s / 54.60 s / 491.74 s; FAST has all four arms, 147 Same / 3,450 functions each |
| Full STANDARD and final guards | PASS: 5,108.27 s, all four arms 159 Same / 12,726 functions each; final source/runtime/products and complete strict archive pass |
| Current-source affected fixture | PASS: all nine measured calls; baseline 0.896 → joined 0.174 s (5.14×), with complete capture and final guards ([scope below](#current-source-affected-fixture)) |
| Current-pin Vest costs | Complete retained 36-row facts verified: no Vest speed or CPU gain; enclosing capture-deadline failure preserved ([scope below](#current-pin-vest-cost-facts)) |
| Delivery | PR #226 merged at `eea420bd`; reviewed delivery `4208273d` has the identical Git tree; required CI and main workflow `39141431318` pass |

The [output record](managed-output-order.md) gives exact Mac and accepted-base
hashes, standalone output scope and the incomplete historical STANDARD attempt.
The joined native archive is SHA-256
`6a1080b8b9d2483dfa44fe3e18c4b1213334f74f96ad369d9abf6989c757400f`
(51,138,055 compressed / 860,282,837 raw bytes). Independent audit
`ae211eadbb96aacd45a747ab23890a4aa0b93f66f281faa8619a84848dfafba9`
verifies every member, all four complete FAST/STANDARD reports, 649 source files
and final runtime/product inventories. FAST retains rlimit drift in 12 units
and STANDARD in 18 per arm, with zero verdict/diagnostic differences, unstable,
skipped or infrastructure-error units. An optional retained CPU-profile replay
failed independently; that diagnostic failure is not product qualification.
Neither baseline #225 gates nor the earlier tiny fixture qualified this join.

## Optional scheduling facts

The memo avoids cleaning an unobserved script only when a successful Current
Cargo run did not schedule it under the exact same input bytes, graph, selection,
compiler and environment. It supplies no proof receipt. Every observed script's
content/environment contract and late compiler-family checks still run. The
current invocation must independently confirm successful Cargo absence and
unchanged inputs before Current or publication. A contradiction refuses Current.
Missing, malformed, oversized, nonregular or changed facts retain cleaning.
Source edits and ambiguous selections, including `--workspace`, remain
conservative.

Facts are limited to scripts in the existing selected dependency closure.
Empty facts return before expensive binding or publication; an old empty memo
is a miss. Actual Vest metadata has 46 script packages, 41 outside that closure;
ordinary cleaning already exempts those 41 without a memo. This narrowing
removes useless optional work and grants no new scheduling or proof authority.
A real-file unit covers outside-only, empty and mixed relevant facts; the four
original strict memo tests remain.

A same-name dependency at another version can otherwise be cleaned even with an
exact Cargo package ID. The committed
[reproduction](../../corpus/repros/cargo-clean-package-versions/README.md) and
managed script-marker/source/generated-input/environment controls remain strict.

## Current-source affected fixture

Three rotated rounds on native c7i.4xlarge (16 vCPU), us-east-1, compare exact
baseline `17464f16`, joined `7411f458` and genuine stock `426d8b01`. All nine
measured calls and all 22 total CLI calls pass. The six Sail/shim bodies are
independent copies of the verified Vest cohort's exact products; no compiler
or Sail rebuild is part of this screen. Both Sail actors use normal `e5365c05`.

| Actor | All three measured seconds | Median |
|---|---|---:|
| Baseline | 0.896185 / 0.885139 / 0.898136 | 0.896185 |
| Joined | 0.179989 / 0.172063 / 0.174437 | 0.174437 |
| Stock | 0.085104 / 0.083266 / 0.086055 | 0.085104 |

The joined median is 80.54% lower than baseline (5.14×), still about 2.05×
stock. Every baseline sample rebuilds its native marker and invokes Verus
once. Every joined sample preserves its marker and invokes Verus zero times;
stock also preserves its marker. The joined memo contains the dev-only 1.0
script: the selected closure includes dev dependencies even when Cargo does
not schedule them. This confirms the narrowed implementation's benefit.

Full stock comparisons retain vstd's 2,060 proved checks / 1,871 functions and
the requested target's one function, with exact verdicts, multiplicities,
diagnostics and public stdout. Current/provenance, source, native marker,
runtime and all six product guards pass. The complete strict archive passes.
CPU was not measured; no CPU gain is claimed. Ending logical cache inventories
are 241,173,114 bytes for baseline and 238,621,117 for joined; the memo is
316 bytes. These are neither peak nor physical storage measurements.

This fixture establishes the reported bug's improvement, not a general Vest
speedup or whole-product acceptance. External evidence is under
`unscheduled-script-output-join-2026-10-18/current-7411-targeted-native-attempt01/`:

- Complete archive: SHA-256 `ed2d1c0781a88992cb1d14e7be839e60c9a8d25f1e803c9c69b9ed2feabb2928` (51,512,938 compressed / 194,851,423 raw bytes).
- All-sample summary: SHA-256 `72115e3bd0e99960346bf67d36f8bbeb4e6dbe2ce9d5de43e0c94307646e5e59`.

## Current-pin Vest cost facts

The second four-actor comparison retains all 36 rows: 12 fresh-A establishment
calls and 24 measured changed-B/unchanged-B calls, with zero unrun or refused
rows. All 27 Sail results are Current with their actual run provenance; each
changed B freshly verifies, and unchanged B reuses that B with zero verifier
invocations. All nine four-actor function/verdict/diagnostic comparisons for the requested
Vest target pass:
3,237 verified and 3,127 full function entries, preserving multiplicity. All
72 before/after source records, binary/runtime guards and nine cache captures
verify independently against the complete immutable archive.

This is scoped performance evidence, **not a whole-cohort pass**. The worker's
capture subprocess exceeded its fixed 120-second parent timeout, so worker
exit 1 and the enclosing failure remain. That child later completed its archive
and receipt; the receiver independently verified every byte, mode and index for
all 3,437 files. No deadline or acceptance threshold is changed.

Native host: c7i.4xlarge, us-east-1. Sources are exact `17464f16` (accepted-main
executable baseline), `354d2995` (#215) and `7411f458` (joined #174), with original
normal689 / fork `e5365c05` and genuine stock `426d8b01`. Every Sail/shim pair was
freshly built with identical locked release/jobs8/incremental0/debug0 flags.
Sail uses ordinary `verify --lib`; stock explicitly requests JSON and time data.
The fixed three rotations are stock/main/output/memo, main/output/memo/stock,
and output/memo/stock/main. Three rounds do not balance four rank positions.

Times are seconds; each cell gives median and full retained range. A is
recorded setup, excluded from the 24-row measured comparison.

| Actor | Fresh A setup | Changed B | B unchanged |
|---|---:|---:|---:|
| MAIN `17464f16` | 116.477914 (114.806145–117.891575) | 40.503571 (40.197466–41.529148) | 0.768136 (0.749438–0.775817) |
| Output `354d2995` | 116.303511 (115.671678–117.631057) | 40.923112 (40.634209–41.275760) | 0.756609 (0.753234–0.790719) |
| Joined `7411f458` | 116.166325 (114.631368–117.873243) | 40.904680 (40.470374–41.501371) | 0.766336 (0.755381–0.773187) |
| Genuine stock | 102.821635 (100.888469–103.750401) | 57.686277 (56.616702–58.310157) | 0.249297 (0.247762–0.282470) |

There is **no Vest speed gain**. Joined changed-B median is 0.99% slower than
MAIN; unchanged is 0.23% faster with overlapping ranges. Output alone is 1.04%
slower for B. Joined versus output is essentially unchanged for B and has a
1.29% higher unchanged median, also within spread. These observations do not
establish a general no-regression claim. The tiny `643ffd2e` gain below retains
its own affected-fixture scope.

Measured B plus unchanged CLI waited-tree user CPU totals are 455.33 s for MAIN,
457.34 s for output and 458.64 s for joined; system CPU totals are 25.00, 25.56
and 25.50 s. No CPU gain is claimed. Across all three states, CLI peak-RSS maxima
are 3,077,552 / 3,078,472 / 3,078,820 KiB. Separate daemon windows record
98.09 / 97.83 / 98.11 s self CPU across the three cycles. These scopes exclude
some outer validation/observer work and do not measure simultaneous whole-host
memory or whole-pipeline CPU.

Final per-cycle logical storage medians are 935.31 MB for MAIN (882.59–936.60),
935.44 MB for output (934.16–936.73), 936.82 MB for joined (935.53–936.82), and
316.88 MB for stock (316.87–316.88), using decimal MB. All 12 complete owned
project/cache/target trees total 9,320,621,858 logical and 10,430,398,464 physical
bytes, including aliases, sidecars, graphs and certificates. Common runtimes,
toolchains, fresh Sail build trees and the final archive are outside that tree.
The nine compact cache captures retain 2,483 metadata/stream bodies and index
regenerable payload identities. Original modes are indexed; evidence-copy modes
can differ. This remains performance evidence, not artifact-restoration
qualification.

The actual second performance phase takes 2,079.156 s; all 36 CLI waits sum to
1,903.135 s. The first failed cohort's 1,484.85 s phase is a different attempt
and supplies no timing acceptance because its detailed rows were lost.

Raw evidence stays outside git under
`unscheduled-script-output-join-2026-10-18/`:

| Evidence | SHA-256 |
|---|---|
| `combined-vest-independent-capture/local-operator/result.tgz` (527,453,814 compressed / 1,780,301,157 raw bytes) | `7d33128f868513b0b86c9aafcb7eb17868baf72320acce3ec1950954a6ea4f5f` |
| `complete-vest-read-only-audit/qualified-facts.json` | `54b9ed44295e37a807548088104efff477db981bf5c18a32c5cff382b00c74ed` |
| `complete-vest-read-only-audit/actual-summary.json` (all rows, rotations and cost ranges) | `8d1af2ff902557c517a7ebcf6ef7c317dd1d93260b80291159f10ead8c1bb4cc` |

## Prior targeted fixture: exact `643ffd2e`

A verified package has normal and dev-only dependencies with the same package
name at different versions. Cleaning the unscheduled version also cleans the
scheduled sibling. Each actual script execution changes a marker in that
sibling's OUT_DIR. Three interleaved rotations use independent warmed
workspaces, targets and caches with default `sail verify` / `cargo verus verify`
output. Every measured Sail result is Current and complete. Baseline invokes
one fresh verifier and changes the marker; the memo invokes zero and keeps it.
Stock also keeps its marker, and candidate stdout equals stock literally in
all three rounds. The baseline's unnecessary verification emits a passing
summary; its full semantic/diagnostic check remains mandatory rather than
pretending that summary equals unchanged stock's empty stdout.

| Actor | Median seconds | Full retained range, seconds | Fresh verifier invocations |
|---|---:|---:|---:|
| #215 baseline `9dff1ab4` | 1.098398816 | 1.093856905–1.102322632 | 1 |
| Memo `643ffd2e` | 0.217023968 | 0.215807603–0.222608779 | 0 |
| Genuine stock `426d8b01` | 0.109443302 | 0.106959086–0.111564038 | Cargo unchanged |

The fixture improvement is 5.06× (about 5.1×); the memo remains slower than
stock. This is not real-project, cold, edit, general CPU or default acceptance.
That source binds complete metadata/snapshot even for empty memo facts. The
new narrowed source requires its own four-actor Vest comparison; this fixture
cannot establish real-project or common-path overhead.

This run used a native c7i.4xlarge in us-east-1, original normal runtime archive
`689004c0` / fork `e5365c05`, genuine stock `426d8b01` and pinned Z3 4.16.0.
Both Sail/shim pairs were freshly built from the exact sources with identical
locked release flags, jobs8, incremental disabled and release debug0. Performance
copies received the existing `strip -S` treatment; original bodies were guarded
and retained. Timing uses monotonic blocking waits, with durable-result checks
after the measured CLI exits. Dependencies were fetched during recorded setup;
normal establishment, warmup and measured calls used Cargo offline.

All 22 actual commands pass: nine measured samples plus three creates, three
fetches, one separate stock authority call, three establishments and three
warmups. The untimed stock JSON/time call supplies two genuine full documents;
all complete Sail target function/verdict/diagnostic multisets must match those
documents, preserving multiplicity. Unknown output refuses. Every source map,
original binary/runtime/stock body guard and cache-capture check passes, and the
complete evidence archive passes strict byte/mode/index verification. Compact
cache capture retains reports, receipts, certificates, manifests and identities
for regenerable payloads; this is performance evidence, not independent cache
restoration qualification. All samples, including their raw streams, remain
available; none was refused or dropped.

Raw evidence is outside git under
`unscheduled-script-output-join-2026-10-18/performance-operator-successor/`:

| Evidence | SHA-256 |
|---|---|
| `local-operator/result.tgz` (43,861,919 compressed / 171,886,168 raw bytes) | `c4da766eaab4c200cb34d8172b9ac8e6880b12b980e4fd158a411d8fa5f20990` |
| `actual-nine-sample-summary.json` | `9cebd404ab44ca45acf104e94f484c289fc85832364d6da07a6d1acd43174c0f` |
| Archive member `evidence/performance/samples.json` | `79fe3242c34178cfc1a4203f8c56c2ecc42f1843d7925f2522cdbdddee6347f1` |
| Archive member `evidence/performance/summary.json` | `ecddf2e2f7edd5484a99e66374b46fa701ca350d9c1b7b0bd02e3ab4bdac6ed6` |
| Archive member `evidence/performance/stock-authority.stdout` (full stock authority) | `183e368967e19a347f74039e44721acb074a2ec625c8575b3c988e3aae15cc55` |

The source before this paired documentation amendment is preserved as
`archive/unscheduled-output-native-643ffd`; executable Rust/Cargo files remain
byte-identical. Earlier failures and historical evidence remain indexed below.

## Historical limits and indexed attempts

The earlier native memo `34bacd5c` on `682ddd98` used fork `73dea021`, stock
`426d8b01`, Z3 4.16.0 and a c7i.8xlarge in us-east-1. Its workspace gate
(642 tests, 35 skips), solver35, then-current seven E2E phases plus extra and
PR subset (four arms, 33 Same / 311 functions) pass. Its three-round unchanged
fixture falls 0.941 s to 0.198 s (4.76×), stock 0.100 s. Those historical
checks and older Mac gates do not qualify the new runtime or joined source;
full FAST/STANDARD never qualified that memo.

| Historical raw archive outside git | SHA-256 |
|---|---|
| `unscheduled-script-memo-native-evidence.tgz` | `45c135eaeef01bc600cc6b3d6584cecc65540bde276f09de5b2e2523773401b6` |
| `unscheduled-script-memo-linux-binaries.tgz` | `49957b3b79d5910e591be316af71b9c92ee397d5226d05cc7ec3e69e7ce5667d` |
| `unscheduled-script-perf-evidence.tgz` | `fd91499a480f5b6ff94523a114a7870f790a198c5eea29bdcf3a29e8c05f4b09` |
| Earlier `mac-evidence.tgz` | `6dbab408385487c00489d8742310a35ff28b3f89557c7e77c268ca2cc5d378e9` |

- `34bacd5c`, 2026-10-16: native benchmark stops before samples because offline setup lacks vstd's index; later setup fetches exact dependencies before offline calls.
- `6563a170`, 2026-10-16: the strict unchanged fixture rejects one fresh verifier call on the preceding binary.
- First Mac scripts attempt, 2026-10-16: companion `sail-z3` absent; no valid fixture result until the existing companion is built.
- `9341e827`, 2026-10-16: gate642/solver35 pass, then E2E rejects reversed summary/Cargo-error order after 129.836 s; original temporary streams were deleted, cause UNKNOWN. PR/full FAST/STANDARD unrun. E2E archive `30dc726c06842b9ef2fa7217672e104d438a8632949c6d522d48666f94c3aa52`; final `6ebd7fb448bf075e98c14e0719e894ec11158150f8c84eacd29f4f7c583b8fdf`.
- `9341e827` original bodies, later bounded investigation: one equal combined-output loop is non-reproduction evidence, not adoption acceptance; full stream SHA-256 `880e50e8192bbf2d5409a3030a6dfb9a0fb0f1d713192429b30bccafe8641cd2`.
- `7411f458`, first current-pin Vest comparison: 36 calls and final guards complete, but whole capture fails on an exported binary's managed hard links. Detailed rows are lost; no performance claim. The successor preserves every workload and archive rule; its complete retained facts above do not erase its own capture-parent deadline failure.

#215's actual relay/control reproductions establish separate transport defects.
They do not prove the historical #174 failure's cause. PR #197 is closed as
superseded by the qualified, merged PR #226.

- `7411f458`, second current-pin Vest comparison: all 36 rows and complete archive verify, but the fixed 120 s parent capture timeout causes worker exit 1. Complete late archive `7d33128f868513b0b86c9aafcb7eb17868baf72320acce3ec1950954a6ea4f5f`; enclosing failure unchanged.
