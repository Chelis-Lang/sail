# Structured terminal failure fixtures

These are exact source-engine outputs captured by `engine/verus/test-engine.py`
for its `success`, `failed_focus` and `late_exec` fixtures at the pinned Verus source commit
`426d8b01e7ffb910a36c15e1f870ff985bb61eef`. The two failure invocations exited 1, emitted a
full-phase terminal seal with `success: false`, and matched the stock verifier's
full verification result and diagnostics. Timing and platform fields are kept
as emitted. The success invocation exited 0. Both failure documents carry `encountered-error: true`. Because the pinned compiler
also catches internal panics into that same shape, the adapter conservatively
reruns both through stock verification, retaining exact failed engine output and
preserving the stock failure report and diagnostics. Missing, truncated or
malformed output and panic exit 101 likewise require fallback.
