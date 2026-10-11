# Proof-time coverage and later result projection

The managed runner and daemon evaluate different boundaries. A completed proof
can have exit code zero and Unknown currentness. Its proof-time input guard must
retain continuous native observation and kernel leases; the daemon cannot repair
an already Unknown establishing proof merely by validating current bytes later.

The runner's `coherent::Guard` checks its main input observer before its read
lease. The stable `native-observation-invalidated` reason therefore identifies
that observer's failed barrier, not a Python helper or sysroot lease failure.
The observer already records a first, sticky namespace reason. Managed `SAIL3114`
notes now retain that reason and the first failed existing check:
`input-barrier` before verification or `post-verification` afterward. Validation
order, short-circuit behavior, Current decisions and receipt publication are
unchanged. Recording and formatting these static labels performs no additional
poll, content read, path probe, helper request or filesystem write.

| Existing observer detail | Identified boundary |
| --- | --- |
| `namespace-mount-inventory-changed` | Mount-generation or namespace/root identity check |
| `namespace-root-binding-changed` | A declared root's existing device/inode binding check |
| `namespace-tracked-input-event` | A relevant event observed for a tracked path |
| `namespace-native-watch-coverage-lost` | Overflow, watch removal, unmount or watched-object movement/deletion |
| `namespace-event-watch-unbound` | Event descriptor not present in the owned watch map |
| `namespace-event-count-exhausted` | Existing event-count limit |
| `namespace-event-drain-budget-exhausted` | Existing 20 ms nonempty drain limit |
| `namespace-event-read-failed` | Existing queue read failure |

These labels contain no paths, configuration, environment or source contents.
They do not identify an affected file, event mask, actor or underlying cause.
The captured stage is the first *failed check*, not the time of the underlying
event. A successful context-ready record followed by a final native failure
places detection at the post-verification barrier; it cannot identify when the
event occurred during verification.

The helper lease and sysroot baseline have separate failure paths. Lease failures
already retain static binding, interpreter, helper-observer and kernel-inventory
reasons. Helper acquisition and detailed sweep timing are mainly test-only
diagnostics. Earlier sysroot namespace/file observer failures were collapsed to
`sysroot-namespace-invalidated`. New code retains that outer reason and the first
failed `namespace` or `explicit-files` observer, with its existing sticky detail.
It calls the same checks once in the same short-circuit order; formatting reads
only saved static labels. This extension is awaiting its separate validation.
The completed D20 active repair remains failed with the older coarse note and
cannot be attributed retrospectively. Existing setup sidecars record elapsed
phases, not barrier outcomes. `SAIL_FRESHNESS_TRACE=1` describes daemon projection stages;
it does not provide the main proof guard's missing historical event details.

The daemon previously appended a generic projection-validation warning to every
Unknown result. An originally Unknown proof could therefore appear to have a
new projection failure even when current input validation succeeded. Job reads
now preserve that proof's original notes without appending this explanation.
A previously Current proof that projects to Unknown still receives the warning.
Classification, complete returned evidence, frame variants and accepted repair
decisions are unchanged. Earlier failed HE reports are retained; no new proof
or reader-performance qualification follows from this diagnostic improvement.

Targeted validation covers native restored writes and preexisting writable
mappings, sticky first-barrier/detail retention, and preservation of original
Unknown notes versus an actual Current-to-Unknown projection. Test outcomes and
source identities are in [the retained record](m7-proof-coverage-diagnostics.json).
All 33 targeted Linux controls pass: two guard, nine namespace, thirteen lease,
three note and six original job integration cases. Affected all-target strict
Clippy also passes. The final non-Linux receiver-use adjustment is compiled out
on Linux; both guard controls were rechecked against the final source. A thin
macOS control loads the actual guard module and passes strict Clippy, without
claiming full macOS crate or proof validation. The complete gate and real proof,
source-engine and live-reader acceptance are outside this targeted scope.
