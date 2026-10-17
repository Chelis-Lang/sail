# Native producer input controls

Status: implemented isolated fixture; native execution pending. These controls
qualify neither default activation nor performance of the experimental vstd
compiler-boundary replacement. The product source and ordinary worker gates
remain separate from the fixture child commit.

Build only the disposable control binaries with the feature enabled:

```sh
cargo build --locked --release -p sail-cli -p sail-shim \
  --features sail-verus/sail-boundary-input-fixture
```

Copy both executables into an owned fixture directory. Run
`scripts/validation/native-producer-control.py` with explicit `--sail`,
`--runtime`, `--lock`, `--policy`, and fresh `--out` paths. Defaults match the
native producer worker layout. Python 3.12 and native Linux `strace` are required.
The policy is the fixture source's `engine/verus/vstd-boundary-inputs.json`.
The runtime must implement the native compiler-read completion protocol.

The feature adds two pauses only around an admitted native observation: after
source-key/private-graph admission before the actual compiler spawn, and after
that exact child has exited before producer completion. It selects the explicit
`SAIL_NATIVE_BOUNDARY_PACKAGE`; default builds contain no fixture hooks. Each
pause has a 180-second bound. The coordinator runs three cases, each with a
300-second bound, stops on a failed case, and retains partial results:

- **Clean A:** a genuine successful selected native compiler establishes producer
  records and the managed project completes with current targets.
- **Source A → B → A:** append an unused proc-macro canary after admission, execute
  the actual native compiler, retain its source-read document, then restore A
  before producer completion. Require the compiler's retained B size/checksum to
  differ from the admitted A policy and require no producer record for that PID.
  The enclosing project may correctly refuse currentness; exit zero alone does
  not qualify this negative.
- **Transitive A → invalid B → A:** select `unicode_ident` only when absent from
  explicit `--extern`; corrupt its public dependency artifacts after independent
  private A copies have been admitted. After actual successful compilation,
  restore public A before completion. Require current project completion and
  producer publication. Trace the owned Sail caller externally, preserving the
  actual compiler PID: successful opens in that compiler/thread/child tree must
  include private A and exclude the substituted public B paths.

All modified inputs are restored on exit. On a failed attempt the owned process
session is stopped before restoration. Single-job Cargo execution prevents
unrelated concurrent native units from consuming the temporary public B input.
This is a correctness control, not a timing measurement.

A temporary, recorded solver adapter uses the existing shim trace protocol. It
unsets the daemon socket, sets the trace directory from a fixed fixture variable,
and executes the retained original shim. It is restored afterward. Clean A and
protected transitive B must have equal complete per-target function records
(excluding measured function timing only), verifier summaries and raw JSON
diagnostics. Repeated function names remain separate list entries. They must
also have identical complete literal solver-input SHA256
multisets, including every stream's multiplicity. Raw input/output/metadata and
all strace files remain in the evidence directory. There is no query filtering
or normalization. The canary-source case is excluded from this equality claim.
The separate ordinary performance/gate runs use the original binaries without
this adapter.

Limits are explicit: 16 MiB per caller stream, 64 MiB total strace, and 256 MiB
of complete solver traces per case; exceeding a bound refuses the control.
Missing admission, completion, compiler-read evidence, producer records, trace
members or literal query equality also refuses qualification. Local fixture
helper checks and feature compilation are not native compiler acceptance.
