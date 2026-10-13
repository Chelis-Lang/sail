# Structured terminal failure fixtures

These are exact source-engine outputs captured by `engine/verus/test-engine.py`
for its `success`, `failed_focus` and `late_exec` fixtures at the pinned Verus source commit
`426d8b01e7ffb910a36c15e1f870ff985bb61eef`. The two failure invocations exited 1, emitted a
full-phase terminal seal with `success: false`, and matched the stock verifier's
full verification result and diagnostics. Timing and platform fields are kept
as emitted. The success invocation exited 0. Both failure documents carry
`encountered-error: true`; that field alone cannot distinguish normal compiler
failure from a caught internal panic.

`terminal-success.jsonl` and `terminal-failure.jsonl` are minimal typed terminal
protocol fixtures for the explicit normal-path marker. With this marker and
normal exit 0 or 1, the exact documents remain original engine evidence. A
legacy, false, duplicate or malformed marker, missing/truncated/malformed output,
and panic exit 101 require stock fallback. Tests preserve failed engine output
and verify that fallback records a second actual child invocation.
