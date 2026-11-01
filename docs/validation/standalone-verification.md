# Standalone Verus-file qualification

Status: implemented and native-qualified on Linux and macOS for the next
release. Protected PR checks and final Sail release composition are separate;
these candidate archives retain the existing version and are not a published
Sail release.

## Source, runtime and scope

Both final native builds use Sail `9ecd57daa9995ed8eb801b08278bba2518c1473b`,
qualified fork `0ad405ebf52f3f730544d8cffa5864e1c9d640cf`, stock Verus
`0.2026.10.18.7d82994` and its pinned Z3. Linux is native c7i.8xlarge,
us-east-1; Mac is native mac-m4.metal, us-east-1c, macOS 26.7. Subsequent
landing changes update documentation and the branch base; Rust, Cargo,
protocol and acceptance-test inputs retain their qualified contents.

| Gate | Result |
|---|---|
| Final Linux workspace gate | Clippy, 1,303 tests / 54 skipped, docs, layering and policy checks pass |
| Final Mac host gate | Clippy and 1,292 tests / 54 skipped pass |
| Final Linux inherited integration | 51 solver controls, complete E2E including extra, all four fast PR arms (33 Same / 311 function records) pass |
| Installed standalone workflow, both platforms | CLI, receipts, undo provenance, jobs, watch, MCP and actual blocked-solver cancellation/restart pass |
| Real original source files, both platforms | Five files / two pinned repositories, 45 calls and 371 function records per platform; verdicts and complete diagnostics agree |
| Candidate archives, both platforms | Fresh-PATH bundled-runtime, first-Serve, edit and repair smoke pass |
| Published runtime consumption | Anonymous validation and unchanged release-index composition pass; first use serves vstd with one project verifier, unchanged use starts none, both Current |

The earlier full integration run at feature `5575a925` also passes all four
fast arms (147 Same / 3,451 function records), all four standard arms
(159 Same / 13,166 function records), 18 mutations and two return controls.
It predates later standalone-only admission and watch corrections and is not
relabeled as final-source acceptance. The final source reruns the complete
local PR gate and the installed/real-file controls above. Unchanged fork
compiler/source-suite qualification retains its own scope in the
[core record](general-performance.md).

## Actual workflow controls

The installed suite requires human-output agreement with stock, complete
structured results, a real unchanged receipt hit, failure/fix/undo with the
original establishing proof, forced execution, changed modules/includes and
restored include mtime, partial scopes, conservative non-UTF-8 include refusal,
controlled environment changes and external source alias retargeting. Durable
jobs are Current on completion and Historical after an edit. Watch covers
startup, save/failure, deletion and atomic recreation. MCP exercises compact
and full replies. Cancellation reaches an actual blocked solver check, publishes
no successful seal, and the restarted verification matches stock. No Cargo
manifest or lockfile is generated.

Final review reproduced a genuine stale proof in the initial candidate
([#636](https://github.com/Chelis-Lang/sail/issues/636),
[minimal reproduction](../../corpus/repros/standalone-external-namespace/README.md)).
Adding an external competing module made stock report E0761 while Sail reused
the old successful proof. Version 2 binds intervening discovery directories,
followed entry kinds and directory-module candidates. Four real compiler
controls cover both module layouts, an unread symlinked directory, and a broken
source link becoming a file. Each requires an actual zero-verifier receipt hit
before the edit and a fresh verifier with exact stock diagnostics afterward.
All four pass on both platforms. Watch refuses source/input expansion beyond
its working directory before subscribing to those ancestor namespaces; CLI
and MCP retain their broader verification coverage.

## Bounded timing diagnostic

The five real files are the pinned HumanEval `tasks/lib.rs` and four original
Verus guide examples. Three paired warm calls per file retain matching verdicts,
per-function flags and complete diagnostics; successful Sail calls are Current
and start no verifier. These are direct-file workflow diagnostics without an
RSS observer, not general performance acceptance or a replacement for core
Cargo benchmarks.

| File | Linux stock → Sail median (s) | Mac stock → Sail median (s) |
|---|---:|---:|
| HumanEval | 18.861 → 0.046 | 10.623 → 0.055 |
| requires_ensures | 0.587 → 0.056 | 0.216 → 0.045 |
| recursion | 1.016 → 0.055 | 0.375 → 0.045 |
| integers | 0.612 → 0.055 | 0.210 → 0.045 |
| pervasive_example | 0.604 → 0.055 | 0.213 → 0.045 |

## Raw evidence

- Final Linux gates, installed/real-file controls, qualified factory and smoke:
  `standalone-linux-final-evidence.tgz`, SHA-256
  `6c8a360f7d14113b8337d94daa1cdd1d22cad931e08cf8aa28162c58aec7493d`.
- Final Mac host/factory/installed controls, including the separate core
  11 source-engine fixtures and cancellation: `standalone-mac-evidence.tgz`,
  SHA-256 `129c65a8509b8b539bf98200951ee34aecc9ba82a835633bb8a3424e35954fd1`.
- Final Mac real-file calls: `standalone-real-corpus-mac.tgz`, SHA-256
  `8b9e6705002b73db97076de197c604ebd862acf61011e6c75ae37a5c02a21f6a`.
- Mac anonymous public runtime composition and final archive smoke:
  `standalone-delivery-mac.tgz`, SHA-256
  `7cd71c1a663fa661b55f55979be4f7623e056e0ab8b2d727d3c6ef986adeadc2`.
- Original stale-proof reproduction: `standalone-namespace-repro.tgz`, SHA-256
  `fba07e339d5d3c68c978367c38274d2769ce6ffd1ff0dd96282e2dcdc2c978fb`.

## Attempt index

- 2026-11-01, initial working tree: missing capability fixture field,
  persistence parent directory and invalid partial test arguments corrected.
- 2026-11-01, intermediate feature: shared-clone Git objects unavailable in
  Docker; independent objects restored. Freshly dropped include observations,
  an invalid environment fixture and compact/full MCP fixture assumptions
  corrected without weakening verdict checks.
- 2026-11-01, `5575a925`: full inherited gates and five-file checks pass;
  subsequent external inline-module reproduction exposes #636, fixed above.
- 2026-11-01, `3c2a4960`: two isolated-runtime fixture failures caused by the
  private wrapper's runtime override; removed from the gate environment.
- 2026-11-01, `d689bf5b` / `587de650`: workspace gates pass; factory setup
  encounters a mounted output root, a nonminimal toolchain, Docker Rustup
  rename restrictions and an existing evidence directory. Fresh factory paths
  and the documented Rustup copy fallback resolve setup; policy is unchanged.
- 2026-11-01, `587de650`: all four namespace controls pass; a new fixture
  assumes an explicit optional `engine: null` instead of its documented omission.
  The fixture now checks the optional value and the unchanged no-side-effect
  and complete-proof assertions.
- 2026-11-01, `9ecd57da`: all final gates pass; archive smoke first stops on
  a checksum sidecar path before unpacking. Correct sidecar and fresh-PATH
  retry pass. Original logs remain in the final Linux capture.
- 2026-11-01, Mac setup: a prematurely queued helper stops before any verifier;
  capture interpreter and ordering corrected. Core smoke setup also stops
  before its marker on an existing prerequisites directory; fresh paths and
  an outer status wrapper complete smoke. No timed sample is discarded.
