# 0108: The SMT runtime is a supported command

- **Status:** accepted
- **Date:** 2026-11-09
- **Amends:** the experimental status given to `sail runtime` and the MCP
  runtime tools in 0.1.0 (#556, recorded in `CHANGELOG.md`). No earlier ADR
  recorded that status; [ADR 0068](0068-runtime-daemon.md)'s transport design
  is unchanged.

## Context

`sail runtime` and the MCP tools `runtime_capabilities` and `runtime_execute`
run complete SMT-LIB transcripts through the bundled pinned Z3 and reuse
matching decided results. The request schema is versioned
([spec](../spec/runtime.md)), and the commands are documented, tested and
shipped in release archives. They were still hidden from `sail --help`,
their help began with an EXPERIMENTAL note, and every user-facing description
said so. The owner decided that this framing misstates the feature.

## Decision

`runtime` is listed in `sail --help` and its help carries no experimental
note. The two MCP runtime tools are listed with the supported tools and their
descriptions do not start with "Experimental:". The guides describe the
runtime without a status label. Fragility, portfolios, team caching,
`serve`, `assurance`, native temporal jobs and the neutral-contract
integration APIs keep their experimental status.

## Consequences

Changes to the runtime request or result schema follow the same compatibility
rules as other supported commands: a new request version, not a silent change.
The CLI and MCP tests assert the new listing.
