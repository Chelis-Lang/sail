# Interrupted c2a4b23 benchmark attempt

The owner reports the original three-round benchmark host controller exiting 1
on `ENOSPC` while writing `primary.tmp`; Docker health probes then time out.
Retained host `stdout` and `stderr` are both empty. The controller checkpoint
still says `running` and has no recorded phases. The
[portable interruption record](m7-benchmark-c2a4b23-interrupted.json) seals those
three surviving files and the initial source/product/runtime/corpus bindings.
The error and host exit are operator observations, not captured by the empty
streams.

Status is UNCERTAIN. Native execution, warmup outcomes, samples, closure and final
immutable seals are unknown until recovery. Empty host files do not establish
that native work never started or that its actors stopped. No benefit or
benchmark acceptance follows. This file-only review performs no native probe,
retry, lifecycle action or controller change.

The original three rounds, 600-second invocation limits, at least 2× median
improvement on each project, spread and exact full-result oracles remain intact.
The earlier [730 warmup failure](m7-benchmark-730a321-bootstrap-failure.md) is a
different retained scope: verification completed but Current full admission
failed on a native binding change. This interrupted c2 attempt supplies no native
result that confirms, refutes or explains that failure. The
[c2 gate, solver](m7-core-c2a4b23-validation.md) and
[complete original repair fixture](m7-repair-c2a4b23-terminal.md) passes remain
separate; neither provides benchmark credit.

[Later recovery](m7-benchmark-c2a4b23-recovery.md) preserves three warmups and no
measured samples. It does not supply an original native terminal or closure and
does not modify this initial interruption record.
