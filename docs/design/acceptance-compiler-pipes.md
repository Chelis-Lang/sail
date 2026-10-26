# Owned compiler pipe capture

This source candidate connects actual owned compiler endpoints to the shared
completion gate for [reads and launches](https://github.com/Chelis-Lang/sail/issues/374).
It has received source review and formatting only. Compilation, direct
stock-Verus checking, broken controls and integrated native/conformance
qualification remain pending. It does not establish complete compiler inputs
or complete the service proof.

The previous owned completion accepts modeled stdout/stderr vectors and proves
their preservation. A caller-supplied vector alone does not establish that its
bytes came from the owned child. Production now uses a separate typed path:

| Boundary | Source and operation |
|---|---|
| Original endpoint extraction | Private `Spawned::take_stdout` / `take_stderr` consume the actual `Child` endpoint, clone its retained `Arc` lease and assign the original stdout/stderr role |
| Actual read | Private `NativePipeHost::read` delegates to that exact owned endpoint; `capture_pipe_read` appends only the returned prefix of the supplied buffer |
| Actual EOF | A zero result from a nonempty supplied buffer sets EOF; child exit, poll readiness/hangup, an absent endpoint and a zero-sized read cannot set it |
| Private sealing | `finish_pipe_capture` mints `SealedPipe` only after EOF without a latched failure; otherwise it returns the exact fresh bytes as unavailable |
| Owned completion | `complete_owned_compiler_pipes` requires both private EOF tokens to match the original attempt's lease and distinct roles, then applies the existing configuration/PID/wait reducer |

`PipeCapture` and `SealedPipe` fields are private; neither has a clone, default
or deserialization constructor. The private native `PipeOutput` has no
caller-vector constructor, and `PipeReader` exposes neither its underlying
reader nor a raw descriptor. The journal's actual production completion accepts
only `Completed`, constructed by its owned child. The older pure raw reducer
remains available for modeled reduction and existing proof controls; native
raw-vector completion is absent. Existing native journal fixture adapters remain
test-only.

`CompilerPipeReadHost` describes one actual primitive read, its endpoint owner
and role, and the classification of its actual error. Its logical observation
trace records the returned prefix, actual EOF, empty-buffer read or error. It
does not require successful I/O. Its optional `next_read` specification lets
checked finite fixtures describe one concrete byte/error/EOF operation; it
adds no native prediction, whole-output assumption or whole-compiler premise.
`FinitePipeHost::read` implements those fixture operations by actually writing
its owned byte into the supplied buffer or returning its selected error/zero
result. Constructive controls include nonempty output before actual EOF.

Identity/role mismatch, reads after EOF, fatal read failures and addressable
capture overflow latch refusal. Actual reads continue after an observation
identity refusal, preserving fresh I/O. Interrupted and would-block errors
remain retryable and do not manufacture EOF. Refused completion retains both
original byte sequences and actual wait outcome. A failed or signaled compiler
can have complete stream/process observations; that is not proof success.

## Actual native callers

The verifying adapter keeps its original stdout worker and streamed stderr
forwarding. The worker calls the private reader's `read_all`; the stderr loop
forwards each original returned prefix and seals only after its existing
zero-result break. The original child wait, compiler-finished notification,
stdout join, cleanup guard and diagnostic emission order stay in place.

The engine's original stock fallback still calls the owned child's
`wait_with_output`. On Unix its implementation closes owned stdin first,
drains both original endpoints with nonblocking reads and poll, waits for the
same child and constructs typed completion. Only real read results establish
EOF. Poll has no normal timeout, and child termination does not discard output
from a descendant that still holds a pipe open. An actual read/poll failure
uses the existing error path. On unsupported non-Unix providers the original
standard-library output operation still returns fresh output, with explicitly
unavailable stream evidence and no compiler-completion admission.

Outer Cargo relays keep their existing raw endpoint forwarding through
managed-only extraction methods. They cannot supply typed compiler EOF
tokens. Planning `Command::output` similarly retains fresh output and optional
environment observation, without exposed child/EOF authority. No new in-run
file, argument, environment control, transport or runtime selection is added.

## Qualification plan and limits

The source-only fixture in `proofs/refinement/stream_capture.rs` covers ordered
nonempty read prefixes, actual EOF, no/empty-buffer reads, sticky fatal errors,
retryable errors, foreign leases, swapped roles, post-EOF reads and typed
completion of successful, failing and signaled waits. The native byte-buffer
unit fixtures cover actual read prefixes and EOF plus equal bytes with a
different retained `Arc` or role. Neither fixture has been executed locally.

The integration includes this module, fixture, private constructors, exact read
trait/event declarations and native bindings in the closed canonical capture.
All twenty new controls remain unqualified, including full seal/completion
always-refuse controls. Actual whole-root function/count discovery is pending;
the previous inventories remain strict checks and do not qualify this extension.
Run all positive functions and every existing/new broken control with direct
stock Verus. Run the ordinary native workspace gate and unchanged solver, full e2e,
fast/standard conformance and macOS gates. Exercise original fallback output
with both pipes larger than pipe capacity, piped stdin needing EOF, and child
exit before a descendant's delayed output. Two unrun native regression tests
exercise those cases; a filesystem handshake releases the descendant only
after the original child wait, avoiding a scheduling assumption. The last case must preserve all
output and finish only at actual endpoint EOF; deadlines used by existing
cleanup tests remain unchanged. Measure capture cost on a quiet native host
before selection. No existing assertion, proof limit or timeout may be relaxed.

The native trust assumptions are actual `Child` endpoint extraction/read/wait
semantics, original strong `Arc` lease identity, safe fd/poll behavior and
correct Rust execution. Pipe ownership does not prove that the compiler is the
only writer: descendants may inherit the endpoint. This slice does not prove
which executable image was launched, authenticate a fork control channel,
certify all physical/opaque compiler I/O, or establish an atomic filesystem
observation point. It creates no `CompilerDomain` or Current authority from a
descriptor, count, PID or EOF token. Full compiler-input closure
([#373](https://github.com/Chelis-Lang/sail/issues/373)) and the remaining
launch/observation architecture remain open.
