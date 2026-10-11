# Managed cold timeout versus native CI

The complete local end-to-end run at `777b86a` passed legacy output/JSON/MCP,
portfolio and five installed MCP transport controls. Its first managed cold
verification then exceeded the original 240-second limit; repair was not
reached. The cold output is empty. Its 594-byte stderr ends after ordinary macro
and vstd compilation announcements at `Checking verus_builtin`. Those messages
announce work; they do not show completion or identify the active process at
timeout. Original scratch cleanup removed its sidecars and process evidence.
The cause remains unresolved; this record does not attribute it to emulation,
load, hashing, solver exhaustion or a deadlock.

[The native CI job](https://github.com/Chelis-Lang/sail/actions/runs/38098393759/job/114349041910)
at `f94bbda` passed every unchanged managed assertion. Its retained cold report
completed in 112.083 seconds; Cargo reported 109 seconds. It contains a complete
Current requested demo proof with one verified function and vstd dependency
evidence with 1,871 verified functions, using two verifier invocations. Warm
reuse completed in 3.195 seconds with zero invocations; deleting the selected
receipt required one fresh invocation, and the restored-mtime failing edit also
required one fresh invocation and agreed with plain Verus. The harness,
invocation, adapter and managed runtime-guard files have no diff between these
two commits. [The portable comparison](m7-managed-cold-777-vs-f94.json) retains
the exact report, artifact and log hashes. Native success does not erase the
local timeout or explain its cause.

The [cold-start guide](../managed-cold-start.md) describes current artifact
ownership and the distinction between dependency compilation and proof reuse.
Timeout diagnostics now retain existing managed progress events, operation
timing, a bounded read-only snapshot of this process tree, and run-sidecar file
inventory before the original owned process-group kill. No compiler arguments,
proof assertions, fixture preparation or deadline change. Later completion
during diagnostic cleanup still fails as the original timeout. The additional
observations are private evidence, not authority for a proof or a causal claim.

Ten targeted controls test bounded descendant
selection, kernel stat parsing, identity loss, symlink/config omission, progress
retention, nonblocking special-file refusal, diagnostic failure followed by
owned kill/reap and rejection of a late complete response. Linux ran all ten;
macOS ran nine, with the Linux real completed-group control separate from its
deterministic cross-platform counterpart. The initial real macOS late-completion
control encountered the original cleanup's `killpg` permission error after its
child had already exited; that failure remains recorded, and the original
cleanup semantics were not changed. They do not rerun
or qualify a verifier, the failed end-to-end phase, or the 200 ms reader goal.
