# Managed cold compilation and proof reuse

`sail verify --incremental` owns its Cargo target and complete proof receipts
under the cache's project/context directories. The project key binds the exact
workspace path bytes. The context binds invocation arguments, engine selection,
runtime/configuration bytes and relevant environment. Source input revisions
and compiler-declared inputs separately bind each retained proof. Unchanged
complete receipts can skip verification; deleting evidence, changing input
content or losing observation coverage requires fresh work. Cargo saying
`fresh` is insufficient proof evidence. See [ADR 0023](decisions/0023-managed-incremental-workspace-verification.md)
and the [freshness contract](freshness.md).

A newly isolated target has no ordinary compiled dependency artifacts or proof
receipts. Even a one-function project can first compile its macro dependency
chain and verify vstd. Different projects and different engine/function/flag or
runtime contexts currently own different targets. Native CI's retained cold
example required two verifier invocations for vstd and the requested demo; its
next unchanged run reused both reports with zero invocations. The
[local timeout comparison](validation/m7-managed-cold-777-vs-f94.md) records a
separate unresolved 240-second failure during this cold path. Neither ordinary
dependency preparation nor compiler startup may be described as an established
warm proof check.

The current product does not share Cargo dependency artifacts across those
targets. A future production compilation cache could remove repeated ordinary
macro compilation, but it needs its own decision and evidence. A safe key must
bind the actual compiler/toolchain, target kind/specification, effective flags,
wrapper behavior, dependency graph and source/configuration/build-script inputs
by content. Produced artifacts need immutable publication and complete byte
binding; consumers need private writable targets, so concurrent Cargo runs
cannot mutate another context's artifacts. Diagnostic paths and compiler-defined
path-sensitive output must remain correct. Cargo metadata or timestamps alone
cannot establish this identity.

Verified dependencies such as vstd also require complete matching proof evidence;
copying their compiled artifacts alone cannot authorize reuse. Existing missing
receipt handling deliberately cleans and verifies such targets again. A cache
for ordinary non-verification dependencies and reuse of verified dependency
reports are different contracts. No shared-target or prewarming change has been
implemented from the cold-start hypothesis, and no cold acceptance bound has
been lengthened. The [roadmap](ROADMAP.md) still requires correctness and actual
product acceptance for any performance claim.

`ci/test-managed.py` now writes per-invocation timing and observation-only
progress events outside its temporary fixture. On timeout it captures only the
owned descendant tree and run-sidecar inventory: CPU ticks, process identity,
crate selectors, working directories and FD targets where available. It never
reads a full process environment or archives runtime configuration contents.
These files are private diagnostics; portable records select nonsecret facts and
hashes. Capture bounds are 1 MiB of read/link bytes, 128 processes, 512 inventory
entries, 128 tasks and 64 FDs per process, with a half-second scheduling budget.
Kernel operations cannot be interrupted by an elapsed-time check; capture may be
partial and cannot prove a cause. Capture errors still reach the original owned
kill/reap path. The 240-second verification timeout and all acceptance assertions
remain unchanged; evidence work is cleanup after a failed deadline, never an
extension that can admit a late result.
Reads use nonblocking, no-follow descriptors and refuse nonregular files, so a
substituted FIFO cannot make diagnostics wait for a writer. The small evidence
controls run in the workspace gate; they do not prewarm or invoke Verus.
