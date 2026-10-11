# Retained tspec timeout: phase and runner audit

The original plain tspec timeout occurred after stock `rust_verify` started and
launched three pinned Z3 children. It was not an observation of a compiler that
had yet to start because Cargo was downloading dependencies. No complete JSON
document, per-function result or final verification outcome was retained.
[The audit record](m7-standard-tspec-retained-audit.json) binds the exact e0ca
runner and pinned Verus source objects alongside the original process snapshot. The [retained sleeping observation](m7-standard-tspec-sleeping-observation-e0.md)
binds the syscall, pipe-occupancy and timeout captures used below.

At the retained instant, the harness had two threads in `anon_pipe_read`, one
in `hrtimer_nanosleep`, and its leader in `futex_do_wait`. The exact runner drains
stdout and stderr on independent threads while another thread polls `try_wait`
and the unchanged deadline every 50 ms. These observed waits are consistent
with stream draining and timer polling; no capture-file write wait was sampled.
They do not map a user stack or prove the role of every thread.

The stock verifier's 16 observed threads were sleeping and its CPU count had
not advanced for about 504 seconds. Three solver children waited on input;
later non-consuming occupancy checks found zero queued bytes in their nine
stdio pipe descriptors. AIR uses separate writer/reader request channels and
an echoed completion delimiter. Empty kernel pipes do not reveal those channels,
the reader's userspace buffers, the responsible mutex or the current function.
The reported futex PC was absent from the retained maps, so it cannot identify
a source-level lock owner.

The runner reached its original 3,600-second deadline, killed the owned process
group, waited for its leader and classified `SAIL3022`. Timeout drops the reader
join handles; completion joins them after the child exits. The recorded six
unit-group remnants were dead zombies after timeout, and the complete suite
continued to its [terminal failed report](m7-default-standard-e0ca-terminal.md).
No evidence here establishes a runner deadlock or a verification disagreement.
The cause remains unknown.

[Issue 68's historical managed/cache wait](../reports/2026-10-10-live-workspace.md)
used a different workflow and runtime scope. Similar sleeping symptoms do not
establish a shared cause. This audit made no retry, proof launch, attachment,
new limit, thread-policy change or stream normalization.
