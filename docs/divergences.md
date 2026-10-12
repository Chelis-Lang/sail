# Divergences

Registry of opt-in modes that can produce a verdict plain Verus would not
(charter rule 8). Each entry names the flag, what can differ, how output labels
it, and how its cache keys are kept apart from default-mode keys.

Default mode remains unchanged. The following mode requires explicit selection.

| Mode | Flag | What can differ | Output label | Key separation |
|---|---|---|---|---|
| accept-first-proof | `sail verify --portfolio accept-first-proof` | An independent seeded Verus run may prove a scope that the baseline cannot. A single complete run must prove the entire requested scope. | Human output labels the mode and selected seed; JSON includes every attempt and selected configuration. | `Mode::OptIn("accept-first-proof")` prefix keys, negotiated protocol-v4 mode, mode-bound disk and memory entries. Shared reads and publishing are disabled for these entries. |

The explicit `--engine interactive` source engine also adapts supported Cargo
`no-vstd = true` metadata to the launcher's equivalent literal option (VI-0011).
Stock Cargo otherwise chooses the wrong macro mode before decoding that same
requested option and can fail before checking any proof. This adaptation is
labelled in the project result's notes and the target's `launcher-options.json`.
Its oracle is the actual launcher with identical Cargo argument bytes and the
same effective `--no-vstd`: full diagnostics, function verdicts and complete SMT
input must agree. Independent full target verification and final compiler checks
remain required. Engine receipts bind the engine manifest, Sail binary and
Cargo configuration; solver entries still bind the exact actual input bytes and
solver configuration. See `docs/verus-issues/VI-0011-cargo-no-vstd-launcher-mode.md`.

The source engine is not a verdict-changing mode: its reports keep
`mode: default` and record `engine` provenance, and `sail-conformance engine`
compares its full phase with plain Verus. It has no opt-in options: it
prepares no lowering for reuse.
