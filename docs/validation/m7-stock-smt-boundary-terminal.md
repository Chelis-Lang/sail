# Normal transport control stopped on recorder loss

The [prepared component sequence](m7-stock-smt-boundary-plan.md) stopped at its
first normal fake-peer control. The private Rust build passed in 2.086 seconds
including its controller. The normal transport completed, returned one response,
and recorded stock child `Drop` completion. Its actual 46-byte input exactly
matches the command vector followed by the unchanged newline/DONE framing;
both hashes are `fccbf658da81f2041d38d0686169e3b5a1dc9b3e23c73d5fd11191c552671680`.

The sensitivity control nevertheless **failed**: the nonblocking recorder
reported 23 attempted rows, two dropped rows and 21 received rows. The missing
sequences correspond to the flush markers. Completed counters accounted for
both drops; the final drain had no additional loss or overflow. The stock
transport's successful transfer does not certify a loss-free observer. The
sender did not retain an errno, so the precise cause of each send failure is
not established by this record.

All 16 runtime/tool postseals match their pre-run identities. Owned cleanup
reported no errors, normal process exit was zero, and no timeout occurred.
The controller returned one because its sensitivity predicate rejected the
loss. Its inclusive duration was 0.903 seconds; the whole build/control sequence
closed in 2.992 seconds. The
[portable terminal record](m7-stock-smt-boundary-terminal.json) retains complete
metadata rows, exact wire equality, build identity and raw-file hashes. The
original frozen controller and artifacts remain unchanged.

The four fault controls and real forwarding arm were not attempted. No proof,
reader retry or endpoint was launched. This is recorder loss in a small
component control, not a transport failure or an explanation of the historical
[ordinary-full timeout](m7-rpc-binding-reader-terminal.md).

A separately reviewed recorder correction may absorb short bursts in a fixed
queue, while keeping stock marker calls nonblocking and reporting every loss.
It must retain the failed scope, preserve all bytes and deadlines, and prove
its normal/fault sensitivities before any real arm. See
[reader acceptance](../reader-acceptance.md), [known issues](../known-issues.md)
and [the roadmap](../ROADMAP.md).
