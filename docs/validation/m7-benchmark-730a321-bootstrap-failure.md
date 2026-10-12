# Real-project repair warmup failure at 730a321

The unchanged original three-round benchmark **failed before measured rounds**.
The [portable record](../../bench/validation/m7-linux-repair-benchmark-730a321-bootstrap-failure-2026-10-12.json)
binds frozen source, ordinary installed products, the qualified source engine,
raw evidence and cleanup. This was local Linux amd64 Docker emulation on an
arm64 development machine. Its original 600-second invocation bounds,
three rounds, full verdict/diagnostic checks and twofold median requirement for
each of HumanEval and Vest were unchanged.

## Observed result

Four HumanEval warmup records survive. The second repair warmup completed two
compiler invocations but reported `accepted: false`, with Unknown freshness
for focused and full evidence. Its full report includes:

```text
SAIL3114: coherent kernel leases or native input coverage were unavailable or broken during verification (native-observation-invalidated; barrier=post-verification; detail=namespace-root-binding-changed); no current proof receipt is published
```

The benchmark correctly rejected that report with `SAIL0942: repair warmup
lacks Current accepted full evidence`. The native wrapper took 837.985 seconds;
the host controller took 841.722 seconds. There are **zero measured samples**,
Vest was not reached, and no whole-loop speedup is available. Setup times are
not benchmark samples. An additional warmup or relaxed acceptance check would
not satisfy the original workflow.

Both owned daemons stopped normally with exit code zero. The independent
birth-bound closure found no live candidates or unreadable rows; ten terminated
unreaped processes had complete dead-task evidence. Source, products, thirteen
runtime artifacts, eight patched source files, lock and corpus identities were
unchanged, and the container birth was unchanged. This certifies only the
owned run's closure. The raw archive retains 1,474 regular files; scratch and
both caches remain preserved separately in the original native namespace.

## Investigation and correction boundary

Read-only terminal metadata shows an explicit generated `serde_core` input in
each compiler family's target tree, while the reusable family build-script
exports were absent. This does **not** identify the historical first changed
root: the old observer recorded only its refusal reason. Its binding check
compares device/inode identity or absence, not directory modification times.
Private observation JSON files are not themselves proof runtime roots.

Source review exposes a potential bootstrap cycle: an uncertified ordinary
build-script generation is cleaned, regeneration changes an explicitly guarded
input, proof freshness becomes Unknown, and ordinary generation publication
requires Current proof evidence. The team is implementing a separate continuous
guard for ordinary compiler-generation certification. Such a certificate must
cover actual script inputs, declarations, unit identity and outputs on its own;
it cannot refresh the dirty proof guard, omit generated proof inputs, accept an
Unknown proof or publish a successful proof receipt. New external inputs and
ambiguous or unsupported script dependencies must refuse reuse.

A separate bounded first-binding diagnostic is being implemented to identify
future mismatches by path digest and captured identity without extra barrier
I/O. Neither candidate has native qualification yet. The original failed
namespace remains untouched; a corrected product requires a fresh run.

A read-only terminal runtime-size diagnostic finds 418 identity records totaling
1,852,005,612 logical bytes, with a largest file of 199,557,488 bytes. A bootstrap
hash budget of one GiB would refuse this workload before considering package
inputs. The candidate therefore needs an explicit adequate streaming budget;
this size observation is neither a hash qualification nor a timing result.

The [complete workspace/solver and original small repair fixture](m7-core-730a321-validation.md)
and [complete reader/retained workflow](m7-reader-730a321-terminal.md) are distinct
passing scopes. They do not replace this failure. M7 remains unaccepted and
PR #74 remains draft.
