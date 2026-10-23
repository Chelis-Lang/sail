# Compound-field definedness correction

Status: implemented correction with local deterministic controls; native backend,
installed service and full differential acceptance remain pending. Issue 323 and
the PR 322 merge hold remain open until that evidence and protected checks pass.

## Correction and historical evidence

The correction retains the selected record expression's definedness and emits
lowering semantics version two. New source jobs require that version. Retained
reads regenerate corrected semantics with the original supported header and
compare the complete manifest. They never reexecute the buggy version-one
translation or rewrite the original source, output or backend report.

The committed version-one manifest fixtures were captured from the unmodified
lowerer at `d0414324`, before changing its semantics. They bind the exact add-one,
unsafe compound-field and guarded compound-field source fixtures. The latter two
are refused under corrected retained admission; the add-one bytes remain valid.
These fixtures exercise translation identity, not backend proof evidence.

## Executed local controls

On native x86-64 Fedora, with Rust 1.99.0 and an exclusive worktree target:

- The neutral source suite passed its ordinary controls, including all admitted
  `u8` inputs for the unsafe and guarded sources, exact overflow at 255, and
  rejection of 255 by the guarded source's precondition. The separately ignored
  native fork/stock oracle has not yet executed for this correction.
- Durable source-job controls passed: unchanged safe version-one result and
  baseline reads across restart and pruning; affected jobs and independently
  sealed baselines refused without rewriting raw bytes; fresh version-one worker
  output refused; existing tampering, cancellation and retained-output controls.
- Focused all-target strict clippy passed for `sail-verus`, `sail-daemon` and
  `sail-cli` after extracting acceptance helpers to retain the lint limits.

The first runtime test used the wrong expected precondition diagnostic prefix;
it failed visibly and was corrected to the actual `SAIL0702` message. Initial
lint attempts exceeded existing function-size limits and passed after helper
extraction. No failed attempt establishes backend or service acceptance.

## Remaining gates

Run the complete native Linux gate, pinned solver suite with the expanded actual
fork/stock fixture, installed-service source-job acceptance for unsafe
`NotProved` and guarded `Proved`, full end-to-end and the required fast/standard
conformance arms. Retain original solver unknowns and runtime failure separately.
The local host checks do not substitute for the repository's EC2 qualification
policy. No native backend result or protected check is claimed here yet.
