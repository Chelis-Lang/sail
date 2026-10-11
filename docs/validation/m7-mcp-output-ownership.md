# MCP complete-response ownership and output locking

The implementation consumes already-owned Result/tool JSON graphs and builds
the complete JSON-RPC frame before taking the output mutex. Both complete JSON
text and structured content remain. Cancellation registration survives encoding
and the final under-lock check; owned cleanup cannot remove a newer cancellation
slot after request-ID reuse. [ADR 0034](../decisions/0034-mcp-response-encoding-ownership.md)
documents the boundary. No cache, transport deadline, proof, solver or schema
change accompanies it.

The private emulated Linux check uses Rust 1.99.0, two Cargo jobs and two library
test workers. Final affected controls pass **46/46**, and all-target Clippy with
warnings denied passes. The [machine-readable record](m7-mcp-output-ownership.json)
binds source, Cargo lock, actual tested ELF and retained raw logs. All actors are
closed; the shared Cargo target and proof endpoints were untouched.

The added controls exercise:

- A paused actual large serializer leaves the output lock available and allows
  the server's actual ping response to publish. The old lock-first critical
  section deterministically fails that same availability predicate.
- An actual cancellation notification during encoding leaves no cancelled
  frame. Another response can publish. Cancellation while waiting for the
  output lock is checked again before writing.
- Unwind cleanup and ID replacement preserve the newer cancel handle.
- Concurrent large replies remain separate complete newline frames, with exact
  embedded Unicode/newlines/models. Consuming and borrowed wrappers produce
  identical JSON graphs and bytes, including full JSON text.
- Typed Result projection preserves optional repair fields and null/large
  result payloads. The large string's allocation moves through the owned graph.

Three preceding complete-library scopes failed **44/45** at the unchanged
owned-read EOF assertion. The first two discarded the read error; a diagnostic
scope preserved **EINTR, OS error 4**. The same unchanged case passed separately
in the retained ELF, which did not qualify the complete suite. The final fixture
retries only Interrupted syscalls under one absolute original two-second peer
clock, resets the socket timeout to the remaining budget, and still requires
zero-byte EOF. Other errors, non-EOF data and deadline expiry fail. The original
one-second client cancellation and no-CLI-retry assertions remain unchanged.
Injected Interrupted/EOF and negative controls exercise that policy. This
explains the captured new EINTR failure, not earlier unclassified failures.

The initial `cargo fmt` attempt also failed while the newly declared test module
had not yet been written; it made no compiler or acceptance claim. That setup
scope and every test result remain in the JSON record. Raw streams and the final
ELF are retained as ignored validation artifacts; their hashes are committed.

This is an affected-code check, not an installed proof-tool, full workspace,
four-reader latency or whole repair-loop qualification. The original 447 reader
failure measured Python only and did not execute this MCP path. No speed gain is
claimed. Complete response text and structured graphs still require temporary
memory; the SDK's cache-family budget does not cover those output allocations.
Slow stdout I/O still holds the publication mutex. Cancellation cannot retract
a frame whose write has started.
