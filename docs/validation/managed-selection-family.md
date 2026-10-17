# Managed Cargo selection and dependency artifacts (#162)

Status: implemented candidate with a completed native transition measurement.
The native source `e2600b44` passes the full workspace gate, solver tests, all local
E2E phases, PR conformance, the Mac host gate, full FAST and full STANDARD;
final source/runtime/binary guards pass. Current-composition GitHub checks
remain required before merge. This does not select M8
or establish a recurring warm-repair improvement. See the
[freshness guide](../freshness.md#cargo-selection-and-compiled-artifacts).

An interactive package verification followed by a repair with explicit `--lib`
previously allocated another Cargo artifact family and compiled `vstd` again.
The correction shares compiled artifacts across package/target selection,
using the existing selection filter. Exact arguments still bind proof contexts.
Features, profile, target triple, compiler configuration, manifests, runtime,
engine identity and environment remain compiler-family inputs. Cargo freshness
and complete dependency receipt/artifact checks remain mandatory. Requested
targets cannot borrow sibling proof receipts. The v1 store remains quarantined.

## Matched native measurement

On 2026-10-17, one quiet native c7i.4xlarge in us-east-1 ran three paired rounds
in rotated arm order, retaining all 18 calls. Both research arms used the same
normal `cc7bbce4` runtime and literal Vest recipes at project commit
`11af01ddaef76cceded03517cd48764adb9d35b1`. Baseline Sail `0272afe8` is
`a6c7a4b8` with only the research fork selector; candidate `5a52f8a9` is product
`285f7fa3` with that same selector. A single physical project/source-cache path
was held across contexts, with separate Cargo, proof and daemon caches.

| Call | Baseline median | Candidate median | Observation |
|---|---:|---:|---|
| Package index, without `--lib` | 83.715 s | 83.500 s | No useful change |
| First body repair, with `--lib` | 67.868 s | 32.732 s | 51.77% lower; 2.073x |
| Later warm specification repair | 33.128 s | 32.937 s | No useful change |

Every call exited 0 with a Current requested-target outcome and restored
sources; each repair has Current full/focus outcomes.
The baseline first repair actually compiled `vstd` and `vest_lib`; the candidate
compiled only `vest_lib`, retaining the original Current dependency receipt
and checking its exact artifacts. Both index arms compiled two targets; both
later warm repairs compiled only the requested target. These observations
isolate a first-selection transition, not a recurring frontend-floor reduction.

All nine pairs have equal canonical outcomes, equal captured requested-target
literal query multiplicities and unchanged compiler comparisons: 3,016 functions per
pair, no resource-limit drift. The original enclosing screen failed because
its external caller fed raw rustc artifact notifications to a comparator that
expects a diagnostic stream. It remains a failure. A separate native replay
passes the same retained pairs through the unchanged conformance API after
separating only exact typed artifact metadata; all diagnostic and unknown bytes
remain unchanged. Six caller controls include diagnostic, malformed metadata
and unknown-output refusals. No timing was rerun. Metadata and original streams
remain retained separately; this is not path normalization.

The measurement evidence index identifies all six full context archives:
SHA256 `495481013abde5dfbb91e71225c0d6d149e520b03fe3d1592c095e75944bcdd8`. The corrected native replay,
including all original compiler inputs and caller controls, is in build archive
SHA256 `32b44aa58eb23eab57dcc60b180aad0b93e35e1dc4f6901c5972cf0dbf4ad816`.
The runtime archive is SHA256
`717680b4cb126f48397b02f32dcbf54e0f2a320fa0c16c86a97237a8c3eee8b1`;
all runtime/source/binary guards are retained. Empty transcript placeholders do
not establish complete solver work; all-hit status remains UNKNOWN.

## Current-source qualification

The qualified native source `e2600b44` is based on `12e82e37`; its two changed
Rust files are byte-identical to measured product `285f7fa3`. That qualification
uses the then-accepted `73dea021` runtime; the timing screen separately binds
its genuine cc7 runtime above. Absolute timings belong to those exact sources
and runtimes.

The implementation PR composes the same selection Rust onto current main
`df1c3630`, including independently qualified #219 fresh-byte digest reuse and
#221 full closure-identity runtime selection. Both selection files remain
byte-identical to `e2600b44`; Cargo manifests and lockfiles are unchanged.
The added non-documentation differences are exactly those merged daemon and
runtime-pin changes. The native table below retains its `e2600b44` scope;
current-composition GitHub checks are separately required before merge. No
additional whole-composite timing or native tier run is inferred from rebasing.

| Gate | Actual result |
|---|---|
| Full native workspace gate | PASS: 660 tests, 35 skipped, 2 doctests |
| Native solver tests | PASS: 35 tests |
| Restored-mtime solver identity regression | PASS |
| Retained compiler-pair replay and caller controls | PASS: 9 pairs and 6 controls |
| Every local E2E phase plus extra | PASS |
| PR conformance, all four arms | PASS: 33 Same / 311 functions each; no differences or drift |
| Full FAST, all four arms | PASS: 147 Same / 3,450 functions each; no differences or drift |
| Full STANDARD, all four arms | PASS: 159 Same / 12,726 functions each; no differences, instability, infrastructure errors or skips |
| Final source/runtime/binary guards | PASS: 591 source files, 3,886 runtime hashes, real MCP client and all five compiled bodies unchanged |

STANDARD records one resource-limit counter drift in pass-through, cache-warm
and engine, and none in cache-fill: the same
`lib::spec::cyclicbuffer::log_entry_alive_value_wrap_around` counter changes
51,948 to 51,945. These reports qualify under the unchanged harness; no verdict
or diagnostic difference is reported. The complete STANDARD command exits 0
in 4,550.32 seconds. All 25 produced archives are strictly verified.
Final archive SHA256
`eed696ac307d2ebaaff8f1d1efc556aefd450de0afe319932d86a544f08f4733`;
STANDARD checkpoint SHA256
`eda5b6827214acf39531e40aac9cddf23eb04bcb14fd6767542b19fd6c3011e2`.

The real-Cargo feature/profile/configuration and dependency-mutation negatives
pass in the native gate. The exact `e2600b44` shared-Mac host gate passes Clippy
and all 649 tests (35 skipped), with every source byte and mode unchanged.
Log SHA256
`4e6348e8b0b378941b8d968482e6ebc5c0b36301fed22365d4dc7b41a056d8fe`.
The native and applicable Mac gates are complete for their exact source
scopes above. Required current-composition GitHub checks must pass before
merge. The recurring warm floor, M8 acceptance and #162 remain open.

## Indexed failed attempts

- `a17941f3`, 2026-10-17: source-cache/receipt-cache overlap correctly refused;
  later aggregate projection and source restoration also failed.
- `a17941f3`, 2026-10-17: selected raw capture exceeded its former 1 GiB bound.
- `a17941f3`, 2026-10-17: baseline calls passed, but the observer assumed one
  total compiler invocation and saw dependency plus requested target; archive
  relocation later crossed mounts. Candidate profiling was unrun. Final
  SHA256 `aecc4c46b32e853456e147d3a44b4fabee04b9d29c4114773401bd8b17648ab1`.
- `8dc02803`, 2026-10-17: external preparer required an optional field; no
  builds or timings ran.
- `8dc02803`, 2026-10-17: native Clippy rejected similar test variable names;
  no timings ran. Corrected by a variable rename. Final SHA256
  `bb37b2a4ed3711ae00958580c0c53c707503fdf74de102a750793b22927950ea`.
- `285f7fa3`, 2026-10-17: full gate passed; external cumulative inventory
  rejected the tracked `CLAUDE.md` symlink. No timings ran. Final SHA256
  `7de76b0e016d45e28b413ac65b0ba9f27c94e56d0ca96bc8a487fa416dee5479`.
- `285f7fa3`, 2026-10-17: full gate and solver passed; external PR target-path
  mismatch stopped before cases and timings. Final SHA256
  `28961e04743783ad36c7aaf9cc9034dff94e77147d8c8f19c6c970d142db1a81`.
- `285f7fa3`, 2026-10-17: all 18 measurements passed their call-level checks;
  external raw-artifact/diagnostic API mismatch refused the enclosing screen,
  so PR conformance was unrun. Corrected retained-data replay above. Final
  SHA256 `504544a898bf369e679e94362222b1cc4c98ec21fb62fffd1b1e86171556e478`.
- `e2600b44`, 2026-10-17: Mac host gate failed three target-isolation tests
  under an externally added `CARGO_BUILD_BUILD_DIR` override. Removing that
  supervisor input passes the unchanged full gate above; no product or test
  assertion changed. Failed log SHA256
  `1450c74e1ae133a414c847bf8f38fa5bbffe243302602aa08f9e21584332984f`.
- `cad0675b`, 2026-10-17: the first GitHub output job exceeded its unchanged
  10-minute job limit and was cancelled. The exact output subcommand remains
  unknown; failure-only test artifact upload was skipped by cancellation.
  Native full E2E above passes the same implementation. No assertion, timeout
  or image change was made. Job log SHA256
  `b606df6a6f55cced5276f98d9b2097a1d48f6a0f1d1a259bece1e263b2e0384d`.
