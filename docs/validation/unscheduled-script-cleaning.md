# Unscheduled build-script cleaning

**Status: joined source candidate, unbuilt and unselected.** The memo is now
composed on exact #215 source `9dff1ab4`, with all new acknowledged-output tests
and the existing memo/regression checks preserved. Original `f217785f` and
PR #197 remain untouched. This join has no new Mac/native, timing, conformance
or full-gate qualification. It cannot be selected before #215 is accepted.

| Current join gate | Status |
|---|---|
| Source composition | Clean conflict-free port of the existing memo onto `9dff1ab4`; no scheduling, proof or output-policy redesign |
| Mac host, native gate/solver, all nine E2E phases plus extra, PR conformance | Pending on joined source |
| Full FAST then STANDARD | Pending on joined source |
| Native matched fixture | Planned: accepted #215 baseline, joined candidate and genuine stock, same default flags and nine measured samples; no new result yet |
| Selection | Unselected; historical 4.76× fixture result is not qualification of this join |

Historical source `d9ab117e` passes independent
source review and the complete macOS host gate (658 tests, 35 solver/platform
skips, workspace clippy); native, CI and performance qualification remain pending. Historical native gate/solver/E2E/PR
passes and the targeted fixture gain below remain scoped to their old sources.
The later `9341e827` native E2E failure remains unresolved, and full FAST then
STANDARD have never qualified this memo. A passing investigation does not erase
that failure.
Native source `34bacd5c` adds only the memo to baseline `682ddd98`; the latter
is the qualified code of the metadata/cache correction (#166/#189). The
subsequent rebase onto merged main `35c089b4` changes only parent documentation;
the memo's product code remains byte-identical. Runtime: fork `73dea021`,
stock oracle `426d8b01`, pinned Z3 4.16.0. Native checks used a dedicated EC2
c7i.8xlarge in us-east-1 and CI image `sail-ci:3c02e445e25df766`.

The memo withholds original cleaning only for unobserved scripts a successful
Current Cargo run did not schedule. Its binding includes the complete input
bytes, graph, selection, compiler and environment context. Existing observed
script contracts are checked unconditionally. Actual successful scheduling
and unchanged inputs are checked again before Current/publication. Missing,
malformed, nonregular, oversized or changed observations retain cleaning.
The memo supplies no proof receipt.

| Check | Actual scope and result |
|---|---|
| Native workspace gate | 642 tests pass, 35 solver-dependent skips; fmt, strict clippy, doctests, rustdoc, layering, deny and docs checks pass |
| Native solver | 35 tests pass |
| Native end-to-end | All seven phases plus local extra pass; strict same-name script marker, changed-source, generated-input and declared-environment controls retained |
| PR conformance | All four modes: 33/33 Same, 311 functions; zero Different, Unstable, infrastructure errors, counter drift or skipped projects |
| Source provenance | All 511 tracked hashes verified except excluded local agent settings; internal packages cleaned and Rust/Cargo mtimes refreshed before compile |
| Earlier macOS source `35b02338` | Full host check: strict clippy and 618 tests pass, 35 solver-dependent skips; actual managed script controls pass; no new joined Mac host claim |
| Full fast/standard | Pending for this memo; parent-source results do not qualify the new memo |

## Historical source and output hold

The PR's remote head is `63062e94`. The retained successor `9341e827`
rebases the same memo onto the later MCP/image parent; its three production
file edits and managed fixture edits are identical to that PR implementation.
The isolated delivery rebase applies those same edits on `3892881a`.
Only a documentation link conflict needed resolution; both links were retained.
The scheduling module and all added runner lines remain byte-identical to the
old implementation. The compiler-family selection change (#162), selected cc7
runtime, fresh solver digest memo and `no_reuse` result field remain intact.
The earlier `2936ead2` composition's macOS host check compiles and passes.
This does not qualify the new composition, Linux, native timing or the output
hold below.

The exact unresolved gate is a combined-output comparison: plain prints
`verification results:: 1 verified, 1 errors` before Cargo's final compilation
error, while Sail prints the same summary after it. The old trap removed the
complete original streams; only the actual unified diff remains. The later
single original-binary investigation exits zero and retains equal 347-byte
combined streams (SHA-256
`880e50e8192bbf2d5409a3030a6dfb9a0fb0f1d713192429b30bccafe8641cd2`).
It is non-reproduction evidence, not merge acceptance or a retry exemption.

[#215](https://github.com/Chelis-Lang/sail/issues/215) separately demonstrates
44 reordered outputs in 96 controlled complete-line producer attempts on the
shared Mac. Its actual production `runner::relay` and `adapter::cargo` sections
are byte-identical in `9341e827`, witness-parent `72acf29b` and main `2936ead2`.
The outer relay's stdout/stderr readers queue independently; the inner adapter
also pipes stdout while inheriting stderr. The memo changes neither path.
This is a concrete unresolved output-transparency defect and a matching possible
mechanism, **not proof of the historical real-Verus failure's cause**. No output
normalization, check relaxation, transport redesign or backend change is part
of this memo. The hold requires lead review of this unresolved gate, followed
by applicable current-source gates and full FAST/STANDARD before merge. No new
host or repeated output investigation is authorized by this source audit.

Current-source host stderr SHA-256:
`6a0b14e7ad72ddc2b23b2f2764aa881416a8053d2b7903815fb596dae67043cc`.
Independent source-review JSON SHA-256:
`8325c6243abf510fecbbb39bd67b7411ba3242b509d2f8bd978d004ce12d9e32`.
The review checks complete memo binding, unconditional observed-contract and
late family checks, actual Cargo scheduling and the separate Current/receipt
conditions. It does not authorize merging around the unresolved output gate.

## Historical targeted native measurement

A small verified package has normal and dev-only dependencies with the same
package name at different versions. Before the fix, cleaning the unscheduled
version also cleans its scheduled sibling. A file in that sibling's OUT_DIR
changes on every actual script execution.

Three interleaved unchanged rounds use independent warmed targets/caches and
default CLI output. Every Sail sample is Current and complete. Baseline has
one fresh verifier invocation and a new script marker each time; the candidate
has zero and keeps its marker. Candidate stdout is byte-identical to plain
stock in every measured round. No sample was dropped or refused.

| Mode | Median | Retained range | Fresh verifier calls per unchanged run |
|---|---:|---:|---:|
| Baseline `682ddd98` | 0.941 s | 0.939–0.954 s | 1 |
| Candidate `34bacd5c` | 0.198 s | 0.196–0.214 s | 0 |
| Plain stock | 0.100 s | 0.097–0.101 s | Cargo's unchanged run |

This is a 4.76× gain over the preceding Sail in this targeted fixture; it
remains slower than stock. No real-project, cold-start or edit gain is claimed.
Cargo dependencies were prefetched inside the container before the program;
all fixture setup and samples then ran with Cargo offline. Monotonic timing
uses blocking process waits, with durable-result validation after the clock
stops. Both Sail binaries and shims were copied and treated with `strip -S`,
as release packaging does; originals and stock were unchanged. Raw metadata
records executable/runtime hashes and all nine measured samples.

Source-body edits intentionally miss the memo. Unknown selection closures,
including `--workspace`, also retain conservative cleaning. No package-ID
clean optimization is claimed: Cargo 1.99's version-specific and full-ID
cleans still rebuild both same-name versions, as the committed
[reproduction](../../corpus/repros/cargo-clean-package-versions/README.md)
and actual macOS runs show.

| Raw archive (outside git) | SHA-256 |
|---|---|
| `unscheduled-script-memo-native-evidence.tgz` | `45c135eaeef01bc600cc6b3d6584cecc65540bde276f09de5b2e2523773401b6` |
| `unscheduled-script-memo-linux-binaries.tgz` | `49957b3b79d5910e591be316af71b9c92ee397d5226d05cc7ec3e69e7ce5667d` |
| `unscheduled-script-perf-evidence.tgz` | `fd91499a480f5b6ff94523a114a7870f790a198c5eea29bdcf3a29e8c05f4b09` |
| Earlier Mac `mac-evidence.tgz` | `6dbab408385487c00489d8742310a35ff28b3f89557c7e77c268ca2cc5d378e9` |

All native archives were downloaded and checksummed; report/source manifests,
all nine samples and candidate binary identity were verified separately.

Retained attempts:

- `34bacd5c`, 2026-10-16: initial isolated native benchmark stopped before any measured sample because the fresh image lacked vstd's registry index in offline mode (SAIL3103); the separately retained retry prefetched exact dependencies before offline setup/measurements and passed, without a product change.
- `6563a170`, 2026-10-16: preceding binary with the new Mac fixture fails the required unchanged-zero-invocation assertion (one fresh call); positive controls pass with the memo.
- First Mac scripts attempt, 2026-10-16: fresh worktree lacked companion `sail-z3`; building the existing companion and rerunning passed, without a product or assertion change.

- `9341e827`, 2026-10-16: native gate 642/35 skips plus solver35 pass, then
  required E2E FAIL after129.836s at the combined summary/Cargo-error ordering
  comparison. Original streams were deleted by the old trap; cause UNKNOWN.
  PR subset/full FAST/STANDARD were not started. E2E archive SHA-256
  `30dc726c06842b9ef2fa7217672e104d438a8632949c6d522d48666f94c3aa52`;
  final archive
  `6ebd7fb448bf075e98c14e0719e894ec11158150f8c84eacd29f4f7c583b8fdf`.
- `9341e827` original binaries, later bounded investigation: one original
  loop exits zero with identical retained combined bytes; no memo adoption
  qualification. Its separate observer limits and historical failure remain.
- Current-main source audit, 2026-10-17: routine rebase and source identity
  comparison only. No new compile, native run, benchmark or gate result.
