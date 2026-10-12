# Frozen 244d51d validation and subsequent strict failures

Source `244d51ddad9d509c8256a9c43481b9358cc5d8e5` passes the complete local
workspace gate and original pinned-solver suite. Subsequent bootstrap candidates
have separate failed strict-compilation scopes; neither failure is reclassified.

| Frozen source | Scope | Result |
|---|---|---|
| `244d51d` | Complete workspace gate | 641/641 tests and every remaining gate step PASS; 428.620 seconds including compilation/checks, 166.848 seconds in nextest. |
| `244d51d` | Original pinned-solver suite | 41/41 PASS; 110.745 seconds enclosing phase, 105.733 seconds in nextest. |
| `e8345f3` | Bootstrap candidate gate | FAIL, exit 101 after 25.972 seconds: runner `similar_names` and moved context hash. Rust workspace/bootstrap controls and solver unattempted. |
| `378a12c` | Corrected bootstrap candidate gate | FAIL, exit 101 after 29.703 seconds: `items_after_statements` and a 64 KiB stack array. Rust workspace/bootstrap controls and solver unattempted. |

Both failed gates passed their initial Python helper controls before strict Rust
compilation stopped them. Source `b4a8f0d` corrects the first compilation failures
by renaming bootstrap locals and borrowing the unchanged report-owned context
hash. Source `602ec67` corrects the later import placement and uses an identical
64 KiB heap buffer. This record supplies no result for the subsequent gate.
There is no lint allowance, oracle change or deadline extension.

The [machine-readable record](m7-core-244d51d-validation.json) binds every host raw
file by length and SHA, including source archives/inventories, gate/solver logs,
terminal results and controller records. It preserves exact source revisions,
container birth/image, commands, limits, and the separate failure outcomes.
All three use two compiler jobs and one independent nextest case at a time;
internal fixture concurrency and original test deadlines are unchanged. These
Linux amd64 Docker measurements under emulation on arm64 macOS are not native
reference performance measurements.

The original controllers report equal source/protected bytes and PID1 across
each closed scope. Container identity, image and birth match before/after.
Known-group closure records have no live survivors or unavailable rows. The
244 gate and solver each retain nine terminated unreaped processes; the e834
failed gate retains one and the 378 failed gate none. Their complete task evidence contains only dead `Z`
tasks. This file-only review verifies those recorded task rows and host raw
hashes; it does not perform a new native census or certify arbitrary orphan
absence.

The three retained images are operator-selected copies, not inferred execution
identities. Their byte/mode/SHA records remain available, but all Cargo artifacts
were not archived. No installed-product, corpus, reader-latency or benchmark
credit follows from these local gates. The independent
[bootstrap source review](m7-ordinary-script-bootstrap-review.md) remains a static
ACK; corrected native controls, sensitivity and whole-product viability remain
pending. The earlier complete product-730 reader PASS and original failed
benchmark are separate scopes.
