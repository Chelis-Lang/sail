# Original installed reader workflow: 730a321 PASS

The ordinary installed product from frozen source
`730a3210151ca51b7e011121c4795553b0cfa3e5` passes the complete original reader
workflow. Both unchanged collectors use four concurrent clients and retain
200 samples each, split into 100 Result and 100 Status reads. Every sample
passes the original Current, input-revision and continuing full-verifier
predicates during actual HumanEval verification. The complete retained CLI/MCP
scenarios also pass with original 120/30/5 workflow, CLI and RPC limits.

| Collector / operation | Median (ms) | p95 (ms) | Maximum (ms) |
|---|---:|---:|---:|
| Python Result | 68.484 | 94.197 | 99.411 |
| Python Status | 42.016 | 59.896 | 68.730 |
| Rust Result | 45.637 | 67.483 | 470.549 |
| Rust Status | 35.333 | 50.273 | 57.884 |

All four p95 values are strictly below the original 200 ms bound. Rust's
isolated 470.549 ms Result maximum remains below the unchanged five-second RPC
limit; no maximum or p95 check was waived. Python reads retain the entire
**4,311,179-byte** Current Result, including dependencies and full repair reports.
Rust decodes the complete typed graph. This is a different complete payload
from the historical 4,313,743-byte response, rather than a requirement to match
that earlier literal size. No graph trimming or collector change supplies the pass.

The baseline and independently prepared active repair reports are both accepted
Current results: vstd 1,871 verified and HumanEval 331 verified, with zero failed,
unknown, provisional or unstable functions. Their exact contexts and input
revisions remain distinct. The explicit unmeasured `--prepare-contexts` phase
precedes the original ordinary/repair-context setup checks, which retain their
600-second invocation bound. No copied receipt supplies proof authority.
The 855.920-second inclusive workflow duration includes preparation and is not a
reader-latency measurement. Measurements use Linux amd64 Docker emulation on
arm64 macOS with concurrent development work; they are not native reference
performance figures.

The [bounded machine-readable record](m7-reader-730a321-terminal.json) binds the
unchanged collectors, original lifecycle scripts, wrapper, exact product bytes,
selected thirteen-artifact engine manifest, proof contexts and raw evidence.
Both products are the same bytes as the
[ordinary build and original repair validation](m7-core-730a321-validation.md).
Input seals match before and after. The outer controller exits zero; cleanup,
archival and outer-finalization error arrays are empty. The retained daemon exits
normally without forced termination.

The exported archive has SHA-256
`93e526f0b0391837dddfa353a57b5fdbdd7d8299c79c7c2102f0f1ce5331ec6f`,
11,533,206 compressed bytes and 171 regular members. All member bytes, SHA-256
and modes were independently checked against the retained inventory, covering
184,761,483 expanded bytes. Original native workspaces remain unchanged; private
environment, source/product copies and proof caches are excluded from the export.
A separate bounded holder audit finds no live candidate or unavailable row
among twelve processes and retains ten stable complete all-dead thread groups.
Those unreaped groups are terminated evidence, not a general orphan-absence
certificate. The acceptance container's recorded birth remains unchanged.

Earlier reader failures, including the original 343.142 ms Result p95, remain
failed. Component gains and timing subsets do not explain this pass or any
historical failure. The earlier instrumented `730a321` publication scope remains
UNCERTAIN. This closes the original responsive full-read and retained-workflow
scope for this product; it does not complete M7. Three-round HumanEval/Vest repair
improvement of at least 2× each, with spread and full oracles, and complete current
standard/cache-fill/warm coverage remain outstanding. PR #74 remains draft and
unaccepted.
