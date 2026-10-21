# Temporal jobs, version 1

Status: specification for implementation review. This interface uses the native
D.I.V.E. library at revision `1025cd6a0510183bc16abc1e6e3eea4f9305c5a9`.
It never executes a D.I.V.E. command, TLC, or Java and has no alternate backend.

## Inputs and execution

A request names one operation: exhaustive model checking, finite seeded
simulation, or validation of a complete finite or periodic trace. Source files
are a sorted map of relative UTF-8 paths to exact source bytes, with an explicit
root module and configuration. Absolute paths, traversal, symbolic links,
duplicate paths, missing imports, external search paths, host operators and
ambient module resolution are not admitted. The worker materializes private
files from this immutable closure and supplies only that closure to D.I.V.E.
Built-in standard module identity is bound by the backend revision.
The first slice admits only the pure embedded `Naturals`, `Integers`,
`Sequences`, `FiniteSets` and `Bags` modules. Semantic analysis checks the
actual loaded origins before binding; host-capable modules such as `TLC`,
`Json` and `Randomization` are unsupported even on an untaken branch.
The guard also inspects actual lowered native overrides and effects. Native
overrides match module names even for user-provided files, so a substituted
`TLC.tla` cannot grant printing or other host capabilities.

The fixed semantic profile is native D.I.V.E.: mathematical integers, exact
states, seed zero, fingerprint zero and one worker. Temporal checking selects
final analysis with independently validated short lassos. These choices are
part of the backend identity, not claims of TLC-equivalent arithmetic or timing.
Requested state/depth limits are explicit; reaching a limit establishes no
holding property. Simulation names positive finite trace count and depth and
never produces an exhaustive verdict.

A S.A.I.L.-owned `sail-runtime-worker` executable links the library. The parent
sends one versioned JSON request over standard input and reads one bounded
versioned response. It starts an isolated child, closes input after the
request, and owns child lifetime. Parent cancellation or deadline kills and
reaps the worker, including during binding. No missing, truncated, malformed,
oversized, wrong-version or unsuccessful worker response becomes a verdict.
The worker does not make a hard memory-limit claim. Per-process D.I.V.E. RSS
observations and cooperative checks are qualifications, not enforced RAM quotas.

## Results and evidence

The backend returns complete success, independently validated counterexample,
sampled execution, valid trace, invalid trace, unsupported input, resource stop,
rejected input, or internal error. Parent timeout/cancellation are separate
execution results and are never cached as logical answers. Every response
retains D.I.V.E.'s original versioned report and native trace when present.
Complete graph evidence is required for a model-checking success; a genuine
validated counterexample may refute a property before exhaustion. A parser
success, an `Ok(report)`, a process exit zero, or simulation completion never
substitutes for this evidence.
The response records the exact configured invariant, PROPERTY, postcondition,
reachability and deadlock checks, and a separate typed completion kind
(`none`, `safety`, `temporal`). A safety completion is not a claim that a
temporal graph was captured; JSON operation discriminators are not evidence.

Trace transport uses ITF with exact value tags, a complete ordered variable
list, complete states, and separate ending/property/loop claim metadata. A
strict reversible codec converts admitted ITF values to native `dive-trace`
version 1 and back. It preserves mathematical integers as decimal strings and
distinguishes strings, model values, sets, sequences, records and functions.
Unknown tags, partial states, duplicate variables, unsupported shapes and
representations that cannot round-trip exactly are refused. Round-trip equality
is necessary but insufficient: the original specification independently
validates the converted trace. A behavior trace establishes only its supplied
execution, not an exhaustive model proof. Runtime random draws and other
features the independent validator refuses remain unsupported.
In particular, ITF's `#unserializable` marker is not a semantic model-value
encoding. Native model values are unsupported in this transport until a
lossless representation is specified; they are never converted to strings.

## Conformance and compatibility

Tests compare embedded results with direct calls to the exact pinned native
library and consume existing recorded primary-tool fixtures where appropriate.
They execute no TLC or Java process. Positive finite examples, real property
violations, forged initial states/steps/endings, unsupported formulas,
resource stops, parent timeout/cancellation and worker failures are distinct
controls. Existing Verus conformance remains unchanged.

The common request/verdict API owns requirement IDs, provenance, content keys
and ledger semantics. This backend owns only the native job payload and
evidence; it does not invent a second ledger or duplicate caller metadata.
