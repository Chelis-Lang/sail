# ADR 0067: Isolated native D.I.V.E. runtime worker

Status: proposed for implementation review.

The owner authorizes Oyster-critical runtime features concurrently with the
Verus performance program. Temporal jobs use D.I.V.E. directly; a TLC adapter,
Java execution and a command-line D.I.V.E. route are excluded.

Pin `dive-api` to revision `1025cd6a0510183bc16abc1e6e3eea4f9305c5a9`.
Add `sail-dive` for immutable native inputs, strict trace transport and typed
backend evidence, and `sail-runtime-worker` as a S.A.I.L.-owned executable
linking that library. Neither is a dependency of the existing shim/Verus hot
path. Their only internal edge is worker to backend until the common runtime
API is integrated.

D.I.V.E.'s cooperative budget cannot interrupt binding or one long evaluation,
and there is no public cancellation handle. The parent therefore owns a
separate worker process, kills and reaps it on cancellation/deadline, and never
turns those events into logical results. There is no hard RAM guarantee.

Use a frozen contained source/configuration closure, exact-state native
mathematical-integer checking and explicit deterministic options. Retain
original native reports and trace bytes. ITF rendering is reversible admitted
transport plus independent semantic validation, never replacement evidence.
The temporal job specification defines unsupported boundaries and conformance.

Existing Verus commands and performance paths do not change. Private git
authentication for source builds is deployment configuration and is never
embedded in source, worker input, evidence or the binary.
