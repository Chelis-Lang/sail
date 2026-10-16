# Unscheduled build-script cleaning

## Status

The joined #215/#174 candidate is implemented and unselected. Exact executed
`7411f458` contains output `354d2995` plus the scheduling memo on #225 executable
source `17464f16`. Accepted main `ee163bc3` differs from that baseline only in
three Markdown files. The delivery rebase preserves all non-Markdown bytes and
modes from `7411f458`; original `643ffd2e`, `f217785f` and PR #197 stay separate.

| Joined gate | Actual scope and result |
|---|---|
| Source and scheduling controls | Independent review CLEAN; all five schedule units pass before the native comparison |
| macOS host | 759 tests pass, 35 skips and all-target workspace Clippy on `7411f458` |
| Native workspace gate | PASS checkpoint, 83.09 s; enclosing full qualification running |
| Solver, all nine E2E phases plus extra, PR and full FAST | PASS checkpoints: 9.09 s / 640.94 s / 54.60 s / 491.74 s; enclosing capture pending |
| Full STANDARD and final guards | Current joined qualification running; no whole-pass claim |
| Current-pin Vest costs | Fresh 36-call comparison running; 12 A-establishment rows and 24 changed/unchanged rows required |
| Selection | Pending full correctness, costs, complete capture and current-head CI |

The [output record](managed-output-order.md) gives exact Mac and accepted-base
hashes, standalone output scope and the incomplete STANDARD attempt. Neither
baseline #225 qualification nor the prior tiny fixture qualifies this join.

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
- `7411f458`, first current-pin Vest comparison: 36 calls and final guards complete, but whole capture fails on an exported binary's managed hard links. Detailed rows are lost; no performance claim. The fresh capture-only successor keeps every workload and strict archive rule; it remains running.

#215's actual relay/control reproductions establish separate transport defects.
They do not prove the historical #174 failure's cause. PR #197 remains held
until the final joined change is qualified and explicitly supersedes it.
