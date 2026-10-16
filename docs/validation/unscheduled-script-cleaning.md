# Unscheduled build-script cleaning

Status: implemented; native required gates and a three-round targeted
benchmark pass. Full fast and standard conformance for this memo are pending,
so this is not yet acceptance for merge. Required PR checks are enforced
separately.

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

## Targeted native measurement

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
