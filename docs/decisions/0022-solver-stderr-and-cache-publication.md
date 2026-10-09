# 0022: Solver stderr and cache publication

- Status: accepted
- Date: 2026-10-09

## Evidence

The strict warm deep suite found two genuine disagreements: quantifier warnings
were omitted by the cache in `generics::const_generics_broadcast` and
`triggers::test_trigger_on_lambda_3`. The latter changed libtest's outcome.
`corpus/repros/z3-stderr-cache-warning/` reproduces the pinned solver's stdout
and stderr independently. Issue #61 records the defect. Warnings are never
removed or normalized by the harness.

## Decision

A stdout-only cached reply is eligible only after its entire solver session
exits cleanly and stderr reaches EOF without emitting a byte. Stage paired
entries until that evidence exists, with a 64 MiB bound. Overflow, stderr,
forwarding failure or abnormal termination discards all staged entries.
Nothing on that path withholds or changes stdout replies.

Forward stderr using a safe standard-library Unix socket and a separate pump.
Before forwarding a stdout chunk, synchronously drain stderr already written
by the solver, preserving warnings before the corresponding result reaches
Verus. The pump also prevents a large stderr stream filling the socket while
stdout is idle. No unsafe code or dependency is added.

Local entry format 2 and shared signed format 2 reject older entries whose
silence was never established. Protocol version 3 and its default socket
refuse older daemons. Version 2 is reserved for the concurrent portfolio work;
its later integration uses a separate protocol version. Query keys retain the
same input/history/identity definition; output effort never enters a key.

## Validation contract

Real Z3 warning input must give the same stdout, stderr and exit code through
pass-through, fill and warm runs. Warning sessions must start Z3 again and
publish no entry. Late stderr and stderr larger than the socket buffer exercise
races and backpressure. Existing quiet-session tests must still show real cache
hits. A solver killed mid-reply now publishes no entry at all: its fault test
requires zero entries instead of requiring earlier replies to survive, while
retaining byte-prefix and exit checks. This strengthens publication safety;
cache survival after failure was never a correctness promise.

Long conformance remains local-only. Older pre-fix cache measurements cannot
establish acceptance of this corrected path; retain them as defect evidence.
