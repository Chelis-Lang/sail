# Compiler filename token refinement

Status: extracted and connected to the current production decoder; native
proof, control, workspace and conformance qualification is pending. No passing
receipt from the umbrella input root is relabeled as this extraction's result.

## Source and scope

The candidate starts from main `e9862433`, retaining the compiler-read schema2,
receipt version 10 and write-ahead origin rules merged in #572. The filename
kernel and six executable guard mutations are extracted from input checkpoint
`e50ddd6e`. Its old first-output-rule decoder and broader DOM/process/provider
changes are not imported.

The current `source_inputs::dependencies` validates every newly decoded file
rule's proposal with the shared predicate. Its complete document-layout check
and original decoding remain. Rejection invalidates the entire inventory;
`update` persists `covered = false`, later loads remain incomplete and managed
currentness is Unknown. The verifier's invocation and freshly produced verdict
are independent of that refusal. Empty, NUL, carriage-return and newline names
are outside the shared token contract; carriage return is a conservative
restriction even on Unix where it can be a filename byte.
Existing version-2 source records and receipts may predate the new guard. They
retain their existing literal-path identity/currentness checks and are not
retroactively certified by this theorem. No record version is changed.

The complete input refinement and every inherited input negative remain in
scope. The query and authority roots also compile the input leaf and must qualify
the added functions; no existing function or control is removed. New controls
exercise nonempty literal/escaped names, byte mismatches, prefix/final-length
checks and refusal. Native tests preserve the existing real-rustc filename
cases and add literal-path and persisted incomplete-inventory controls.

| Gate | Result |
| --- | --- |
| Source capture, envelope and control registry | Pending |
| Direct stock-Verus input, query and authority roots | Pending |
| Complete required ordinary negative controls | Pending |
| Native focused tests and full local fast gates | Pending |
| Full fast and standard conformance | Pending; runtime admission changes require both |

The theorem is exact token encoding for supplied bytes. Native decoding,
document framing, rustc encoding, complete compiler input discovery, filesystem
observation and full service correctness remain separate obligations. No
performance or new compiler-origin proof is claimed.
