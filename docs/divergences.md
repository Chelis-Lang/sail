# Divergences

Registry of opt-in modes that can produce a verdict plain Verus would not
(charter rule 8). Each entry names the flag, what can differ, how output labels
it, and how its cache keys are kept apart from default-mode keys.

Default mode remains unchanged. The following mode requires explicit selection.

| Mode | Flag | What can differ | Output label | Key separation |
|---|---|---|---|---|
| accept-first-proof | `sail verify --portfolio accept-first-proof` | An independent seeded Verus run may prove a scope that the baseline cannot. A single complete run must prove the entire requested scope. | Human output labels the mode and selected seed; JSON includes every attempt and selected configuration. | `Mode::OptIn("accept-first-proof")` prefix keys, negotiated protocol-v4 mode, mode-bound disk and memory entries. Shared reads and publishing are disabled for these entries. |
