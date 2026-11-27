# Experimental bounded Kani checks

This candidate is under qualification for v0.4.0. It is absent from every
published release and is not yet selected. Check the installed help before use.
The pinned original Kani backend currently supports native Linux x86-64.

Prepare the original tools with Python 3.11 or newer available:

```sh
sail engine install --backend kani
sail engine status --backend kani
```

Setup downloads the exact original Kani 0.68.0 bundle and six original Rust
component archives. It checks their identities and all payload files without
executing downloaded installers. A conflicting or corrupt installation refuses;
setup never overwrites it. Allow approximately 2.1 GB for the original payloads,
plus the private tool stage and each check's build/capture output.

In a Cargo project with a hand-written `#[kani::proof]` harness, prepare the
project dependencies and select the exact harness name reported by Kani:

```sh
cargo fetch --locked
sail verify --backend kani --harness pilot::proof --unwind 6 --output json
```

Root-level harness names, such as `proof`, are valid exact names too; module
harnesses use their complete `module::proof` name. No suffix matching is used.

Use `--project DIR` for another project and `--cargo-home DIR` for its prepared
Cargo home. Checks run offline with fresh targets. `--timeout-ms N` sets the
child-process budget from 1 to 600000 milliseconds, default 600000. Tool and
filesystem inspection is separate from that child-process budget. Ctrl-C
cancels and reaps the active original driver.

The JSON document has kind `sail-kani-bounded-result`, the explicit backend,
harness and unwind bound, the original decoded observation and a capture path.
Read `observation.outcome`:

| Outcome | Meaning | Exit |
|---|---|---|
| `bounded_success` | All selected checks succeed within the explicit bound | 0 |
| `counterexample` | An original non-unwinding property fails within that bound | 1 |
| `bound_insufficient` | Unwinding checks fail; no property proof or refutation follows | 1 |
| Setup, cancellation or decoding refusal | No admitted backend observation | 2 |

These results never mean ordinary `verified`. They establish neither an
unbounded proof nor source currentness. Source, Cargo configuration, dependencies
and build scripts remain ordinary project inputs; a reusable authenticated input
closure is not implemented. No proof receipt, Verus job or result-cache entry is
created. Every invocation runs afresh. Original request/streams/export/compiler
data remain under the reported capture directory. Failed setup and interrupted
checks can leave diagnostic partial captures, which are not successful results.

Kani mode requires its own explicit options and refuses Verus engine, portfolio,
background and forwarded options. `sail verify` without `--backend kani` keeps the
Verus workflow. There is no Kani MCP endpoint in this candidate. Function-contract
and generated-harness modes are unsupported. Native Linux CLI/direct-pilot,
package and extension gates pass; Mac build validation and
release adoption remain pending.
See [the original-driver record](validation/kani-original-driver.md) and
[payload preparation](validation/kani-payload-installation.md) for component
results and their limits.
