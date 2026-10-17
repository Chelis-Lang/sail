# Spec formal spelling sensitivity (VI-0023)

These two sources differ only in the spec function's first formal and its bound
reference. The client module is unchanged. They are reduced research fixtures,
not registered conformance entries, and have not yet been executed.

Run each with the same actual runtime, crate name and ordinary options, logging
SMT into separate owned directories. Keep literal stdout/stderr/status and all
logs. Compare the `client` solver-input streams with only log-only `;;` source
comments omitted, as the existing VI-0019 compiler regression does. Require an
actual solver check and the defining axiom; an empty log is a refusal. The new
candidate regression additionally requires its positional parameter spelling.

The current-runtime baseline must first reproduce the change; the candidate
must retain logical structure and make the unchanged caller's input stable.
Also run a deliberately false client assertion under each source spelling and
compare each candidate verdict/diagnostic with the corresponding baseline.
Source-name or span edits are not grounds to discard diagnostics. Native
shadowing, recursive-copy, unused-argument and generic trait-default tests are
required before a real-project timing screen.

Historical evidence and qualification limits are recorded in
[VI-0023](../../../docs/verus-issues/VI-0023-spec-parameter-names.md) and
[ADR 0054](../../../docs/decisions/0054-stable-spec-parameter-names.md).
