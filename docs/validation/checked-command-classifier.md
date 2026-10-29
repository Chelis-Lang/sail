# Command classifier qualification

Status: the conditional Stock refinements pass at code checkpoint `67d1c84c`.
Fresh Linux workspace, solver, full E2E plus extra, PR-fast, full-fast and
standard conformance, including all 18 mutations and both reverts, pass at
`e63ac6aa`. The bounded component comparison completes all 100 samples with
zero refusals. The full Mac host gate passes at `f4d686fc`; lead review accepts
this bounded adoption and its measured cost tradeoff. Native-host/origin
refinement and the full service theorem remain open.

The qualified code checkpoint is based on main `4486c712`, after the parser and
Holding/filename deliveries merged. Its entire Git tree equals the audited
`2ac97a50` restack. All 278 complete proof programs and 13 native caller bindings
match the qualified `67d1c84c` code; inherited installer/release Python and
documentation changes have focused installer/link coverage. Historical proof
receipts retain their actual source and environment attribution.

The candidate is refreshed onto main `bdd5e050`. Since main `4486c712`,
only documentation, 11 assurance assertions in two daemon tests and the nine-line
child-Cargo-home fixture isolation from #610 change. Production/proof bodies,
all 278 complete programs and 13 native bindings, Cargo/profile/dependency
inputs, corpus, CI commands and verifier/runtime pins remain unchanged. Linux
corpus and component-cost receipts retain their actual `e63ac6aa` source and
binary scope. The exact feature patch from `487eb7ec` to `47390210` equals the
patch from `bdd5e050` to `f4d686fc`; the final qualification fold changes only
Markdown. No historical execution is relabeled as a final-head execution.

The new assurance assertions pass in actual `47390210` workspace CI and in the
full `f4d686fc` Mac gate. Both nonignored Unix tests,
`affected_version_one_history_is_refused_even_with_intact_content_seals` and
`a_new_worker_cannot_supply_even_an_unaffected_version_one_projection`, are
selected by the unchanged workspace gate. Their added assertions are not
credited to the earlier 1,225-test Linux execution.

| Scope | Result |
|---|---|
| Proof checkpoint base | Main `8797ccd4`, with the parser and Holding/filename slices |
| Production use | Native command facade and `Session::expects_output` call the shared operations; native callers remain hash-bound, not proved |
| Complete roots | Parser 135 verified / 213 function entries; command 108 / 163 |
| Fresh owning cases | All 55 parser and 30 command cases pass; the 54 parser and 29 command mutations each fail ordinarily with exactly one failing function |
| Combined proof registry | 95 fresh cases across parser/command/persistence, plus 183 historical cases reused only after complete-program, raw-result and runtime comparison; 278 distinct cases, not 278 fresh calls |
| Abstract models | All 35 complete programs and raw results match the actual `838706f0` executions; reused, not rerun here |
| Source guards | All 72 tests pass; all 278 generated programs remain byte-identical across the inventory lock; independent raw discovery/source audit passes |
| Fresh Linux workspace | Normal `ci/local.sh gate`: 1,225 passed, 54 skipped; 210.55 s at `e63ac6aa` |
| Fresh pinned solver | Normal `ci/local.sh solver`: 51 passed; 18.91 s at `e63ac6aa` |
| Mac compatibility | Full `ci/run.sh host` passes at `f4d686fc`: 1,214 passed, 54 skipped; no filtered or retried cases |
| Remaining scope | Final-head CI/review; native-host/origin refinement and the full service theorem remain open |

Original proof/checkpoint environment: native x86-64 EC2 `c7i.4xlarge`, `us-east-1`, Ubuntu 24.04;
CI core image `587662778c7e8f88`
(`sha256:738574001e55a7367ab9f89f0fa09196caf94b33b04b91352a580532bcc33c68`).
Sail uses Rust 1.99.0. Direct proofs use Stock `0.2026.10.18.7d82994`,
Rust 1.99.1 (`58cf8f9c35b355199727627d07ae34a4e2846d67`) and Z3 4.16.0;
the Stock archive SHA256 is
`e9af03732d4bdd32b93a8fcf471706739c24c1473fa5086618d96c47237845dd`.
Native solver tests also use fork `47fb3be6` with manifest
`d6096d5bd4952df54f8fdd0fdc157a1bb610298318166495604ccb7201dbe44b`.
Complete actual tool/runtime maps are unchanged before and after the checks.

## Mac host compatibility

The full native arm64 `ci/run.sh host` passes at `f4d686fc` on macOS,
Rust/Cargo 1.99.0, with one Cargo job, serial nextest and nice 19. Clippy,
1,214 tests pass; 54 configured ignored tests remain skipped.
The test stage takes 142.90 s; the full gate takes 231.47 s. Actual source
blobs, tool binaries and caller Cargo configuration agree before and after.
The existing global Kache build-wrapper configuration remains active. The
new persistence transaction/symlink tests and the fixed CLI/no-reuse branches
have actual passing entries. No test filter, retry or deadline changes apply.
This closes host compatibility, not Mac Stock/corpus/performance qualification
or a theorem about native filesystem effects.

The earlier `47390210` full host attempt failed two inherited CLI fixtures.
Both reproduce on unchanged main `487eb7ec`: the fixtures inherit the caller's
Cargo wrapper configuration and take an unsupported-configuration fallback.
Separate #610 fixes #608 by isolating only each child Cargo home, preserving
HOME/Rustup, parent build configuration and every assertion. It is merged in
the current base. The failed whole run remains failed despite its other passing
tests.

Private Mac host evidence is
`experiments/2026-10-29/acceptance-command-cache-f4d-macos.tgz`, SHA256
`156d6b73dee601f317dc94b60ccacdf98875dbaf137d2e9f001ddffbf3f1a5c3`,
28,326,463 bytes, with 1,378 indexed payloads and complete payload readback.
It retains canonical source, complete gate streams, actual CLI/cache-test
executables and Cargo fingerprints. The separate #608 reproduction/fix packet,
`experiments/2026-10-29/cli-fixture-cargo-home-5ec-macos.tgz`, has SHA256
`62acde47721b2e44f9807f97f047a4774b56c96ea5ad9b94e2b3e0a601341ac0`
(66,872,558 bytes), including the original failed full gate and main control.
Passing test-child outputs not printed by the normal harness are not claimed
as retained per-command payloads.

## Fresh Linux runtime qualification

The `e63ac6aa` run uses native x86-64 EC2 `c7i.4xlarge` in `us-east-1`,
Ubuntu 24.04, Sail Rust/Cargo 1.99.0 and the same pinned Stock, proof-Rust,
Z3 and fork runtime files identified above. Its actual core image is
`sha256:ab8104b1eab2a37ad33d2e61b62057552f520e3e65e50fde0f492fe1d9ca573e`;
MCP image is
`sha256:86695f7208fec84723598ed09820a67abcf9b175fdce336fc4f188611ed4e4ac`.
Both images' complete verifier/tool maps agree before and after. These are
fresh native runs, not new formal refinement or model executions.

| Normal command | Result | Seconds |
|---|---|---:|
| `ci/local.sh e2e` | Every phase plus extra passes | 1,013.98 |
| `ci/local.sh conformance fast pr` | Four arms, each 33 units / 311 functions, all same | 105.86 |
| `ci/local.sh conformance fast` | Four arms, each 147 units / 3,451 functions, all same | 476.61 |
| `ci/local.sh conformance standard` | Four arms, each 159 units / 13,166 functions, all same; all 18 mutations plus cold/revert/forced-revert agree | 5,386.21 |

Every arm has zero skipped projects, unstable units, verdict/diagnostic
differences and infrastructure errors. Recorded rlimit-count drift remains in
the reports. The closer checks every unit outcome and all 21 mutation rows,
including no false verified result and both reverts' own-answer checks. All 84
mutation stdout/stderr files are retained under their exact row/arm paths.
Final source/image checks pass and complete runtime/tool maps are unchanged.

The normal E2E and conformance builds produce distinct release variants. Their
actual executable bytes and Cargo fingerprints are retained per stage; source
identity does not imply binary equality across these feature graphs. Passing
output-loop temporary fixtures are deleted by the unchanged E2E script: the
packet retains its complete wrapper streams and persistent job evidence,
without claiming all per-command temporary payloads.

## Bounded component cost

The public API harness compares main `4486c712` against `e63ac6aa` on the same
idle native host, after every correctness gate completes. Five alternating
pairs per workload retain all 100 samples. Full classifier values/errors/spans,
Session state/key traces, record metadata/replies and actual cache record bytes
agree; external clock/path/inode fields are separately retained and checked.
Pinned-Z3 SAT/model replies are captured once outside timing and shared exactly.

| API workload | Baseline median | Candidate median | Candidate / baseline |
|---|---:|---:|---:|
| Short commands | 66.02 ns | 66.26 ns | 1.004× |
| Invalid commands | 59.54 ns | 57.58 ns | 0.967× |
| Long commands | 3.084 µs | 3.179 µs | 1.031× |
| Session output prediction | 29.11 ns | 32.24 ns | 1.107× |
| SAT record parsing | 1.950 µs | 2.027 µs | 1.040× |
| Model record parsing | 4.080 µs | 4.100 µs | 1.005× |
| SAT record publication | 24.158 µs | 25.158 µs | 1.041× |
| Model record publication | 26.003 µs | 24.747 µs | 0.952× |
| SAT warm hit | 7.899 µs | 8.026 µs | 1.016× |
| Model warm hit | 10.321 µs | 10.116 µs | 0.980× |

The Session and SAT parsing increases are measured costs of this candidate.
All ranges and outliers remain in the sealed JSON; for example, SAT publication
ranges are 23.65–73.76 µs baseline and 23.32–83.19 µs candidate. These small
component batches include retaining returned values in preallocated vectors.
Preparation, validation, serialization, cleanup and solver calls are outside
timing. Publications use prepared buckets and buffered writes; hits use
prewarmed entries within the checked fresh-atime window. This does not measure
service/corpus latency, fsync/durability, stale-touch/GC or isolated proof cost.

Both arms use normal locked offline release builds after one locked workspace
fetch fills an isolated registry cache. Their external lockfiles match exactly;
selected third-party versions/checksums match the repository lock, with 193
repository packages unselected. Actual feature/dependency graphs, binary and
source/tool/runtime/config fingerprints are recorded; no whole-CLI build
equivalence is inferred. All final identity guards and raw sample checks pass.

The complete parser, command and persistence discoveries ran at `2baa1336`
with unchanged default Stock resources, one verifier thread and a 120-second
per-call deadline. The locked inventories come from actual whole-crate output.
At `67d1c84c`, all 95 owning cases were freshly executed with those same flags
and limits: three positive roots and 92 ordinary negative controls, with zero
resource or VIR refusals. Full source/runtime/Rust/image terminal guards pass.
No control, contract or executable body changed in the inventory lock.

The 183 inherited calls executed at `58edf644` (three positives and 180 ordinary
controls). Their captured complete-program hashes, all raw results, Stock
archive/runtime and Rust identity match this candidate. The 35 abstract model
calls retain their original `838706f0` attribution. Historical temporary driver
files were not retained by the normal runner; their captured hashes match the
canonical reconstructed programs. Actual drivers and dependencies for all 95
fresh calls are retained in the new packet.

| Private evidence | SHA256 | Bytes |
|---|---|---|
| `experiments/2026-10-28/acceptance-command-persistence-67d1c84c-proof-closed.tgz` | `5c966593ada68cdf28e90cd4ab74932a341bc52068e122459a384161b333a676` | 14,574,125 |
| `experiments/2026-10-28/acceptance-command-persistence-67d1c84c-native-checkpoint.tgz` | `35d18a850d3ffaf8974cd3c15ced3a69891591e6c64a6d0fd14d396f396a1557` | 7,496,885 |
| `experiments/2026-10-29/acceptance-command-cache-e63-native-cost-closed.tgz` | `2a06157d7d2597d31ef4672507ea01a398ed7c12adb2beaf459210cad3607c37` | 653,007,092 |

All three packets pass complete payload readback and private PUT/fresh GET with
matching bytes and SHA256. The proof report has SHA256
`2cf98e3d45d111545260fa78fb5be8adec6a35791979680acabbf8b5827388ca`.
The independent discovery/count-lock audit has SHA256
`1aac6117196d33618c183d751d6ec81263a687a5134802d42f0d5986c3ee9d96`.
The independent full selected-case audit rechecks every actual driver, dependency,
function inventory and ordinary result; its report SHA256 is
`f258562b601efb5b64e0046c9d803ecd76dffeaf69b17933d5ab7c331bdbcf2c`.
The original `67d1c84c` native checkpoint records debug Cargo fingerprints and
commands only. The current native/cost packet retains 40,902 indexed payloads
and explicitly identifies 134 excluded regenerable build/IR files by digest;
it makes no independent raw-IR audit claim. Its actual gate audit SHA256 is
`9aa2398dff27956624b9a69e96ccdb1828b25bf3e0283b04e39484173c7ab4be`;
component-statistics SHA256 is
`18a7ab94a6b3a34187f65fbf6b6acc3d630cde6394f1d9113e8dc2a9914d4dda`.
The independently reviewed method, closer, source bundle and both failed setup
attempts are retained. A separate fresh-GET packet audit checks every indexed
payload, source maps, all corpus/mutation outcomes, all 100 samples and actual
returned values/record associations. Its report SHA256 is
`95eb152a7c3b2d348c7c376e906bf969d443b74c63063e90fb4ea9b794fd5674`.
This audit reads recorded evidence; it does not rerun verification or rehash a
live host, and does not independently reimplement the full diagnostics comparator. Actual native report SHA256 is
`789f4a1e25ac4d90e3759cfeb77e1e1c837ae97fe835cadfb4f04adf8a73ec99`;
actual cost report SHA256 is
`379b691d527c92907d84b92216a8d192d5e6765f67a543b0158237e99bf5aee3`.

Normal reproduction runs every registered case:

```sh
ci/local.sh proofs
ci/local.sh refinements
ci/local.sh gate
ci/local.sh solver
```

Resource refusals, malformed output, changed inventories and narrowed scope
refuse qualification. Simulated-verifier guard fixtures supply no Stock credit.
The [design](../design/checked-command-classifier.md) describes the existing
shallow classifier and the unproved native/Session/solver boundaries.

| Date | Source | Attempt | Result |
|---|---|---|---|
| 2026-10-28 | Uncommitted extraction | Source suite | 65/66 pass; the unchanged final-source fixture refuses an uncommitted root before simulated verifier calls. Zero Stock credit. |
| 2026-10-28 | `d1299f69` | Committed source suite | 66/66 pass; historical source-only checkpoint, not native acceptance. |
| 2026-10-29 | `e63ac6aa` | Cost setup v1 | Offline lock generation exits 101: the image has no downloaded registry. Zero measurements; preserved. |
| 2026-10-29 | `e63ac6aa` | Cost setup v2 | Locked prefetch and offline lock pass; external harness compilation exits 101 because Arc byte fields lack serde support. Zero measurements; preserved. |
| 2026-10-29 | `47390210` | Mac host | 1,212 pass, two inherited CLI-fixture failures, 54 skipped; reproduced on main and fixed separately by #610/#608. Raw failed run retained in the #608 packet. |
