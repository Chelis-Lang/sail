# 0138: Replay the exact session prefix for profile measurements

- **Status:** implemented foundation; native Linux gate and focused review pass
- **Date:** 2026-11-14
- **Tracking:** #712 package B

## Context

The proof-health spike found that slicing assertions changes near-limit solver
answers. A profile measurement must retain the solver's actual preceding
history. Its prototype also reconstructed command text with newlines and could
include the next check while searching for a statistics request.

## Decision

Add a diagnostic-only prefix selector in sail-portfolio. Selection uses the
zero-based check index in the original process session. Feed the existing
structural splitter in bounded chunks, retain all original bytes from offset
zero through the selected command and borrow that exact slice. Preserve earlier
checks, statistics, comments, whitespace, options and stack operations. Never
include a later command or append a diagnostic request. Refuse unclassified,
structurally incomplete or exit commands before selection, missing checks and
sessions above 64 MiB. SMT validity remains the solver's responsibility.

No execution, verdict, retained-session admission, cache, function join or user
command is added. Full trace parsing, worker lifetime/cancellation, authenticated
solver/session selection and plain-versus-traced comparison remain separate
profile implementation work. A traced result describes its measurement only.

## Consequences

Later profile work can replay full history without inventing a sliced search
state or rewriting solver bytes. This foundation does not qualify the complete
profile workflow or replace its real-project and Mac acceptance. There is no
new dependency or layering edge. Source/model proof bodies are unchanged.

Actual component qualification is recorded in
[the validation record](../validation/profile-session-prefix.md).
