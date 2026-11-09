# Sail

Sail makes repeated Verus verification faster. It keeps solver answers and complete
proofs that still match your inputs, so a small edit does not have to repeat all the
work of the previous check.

Use it in place of `cargo verus verify`, or point it at a standalone Verus file.
Coding agents can use the same workflows through MCP. Sail uses its own Verus and
pinned Z3; you do not need to replace an existing Verus installation.

Start with [installation](install.md) and the [quickstart](quickstart.md). The
quickstart takes one proof through a successful check, a failing edit, and an undo.
For an existing Cargo project, the [workspace guide](workspace-verification.md)
covers Cargo verification, JSON reports, background jobs, and watch mode.

For Z3 work outside Rust, the [SMT runtime](runtime.md) accepts versioned requests
containing complete SMT-LIB transcripts and reuses matching decided results across
runs. The [MCP reference](mcp.md) shows how to give an agent access to both
verification and solver tools.

The [standalone guide](standalone-verification.md) covers individual files.

The [reuse guide](reuse.md) explains the different caches and when they apply.
[Performance](performance.md) records measurements and their workloads;
[compatibility](compatibility.md) and [known issues](known-issues.md) cover
unsupported setups and workarounds.
