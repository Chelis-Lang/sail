# 0105: Place neutral shadows away from inherited Cargo configuration

- **Status:** accepted
- **Date:** 2026-11-08
- **Implements:** item 6 of [ADR 0103](0103-scoped-neutral-widening.md)

## Context

Neutral translation refuses every unreviewed Cargo configuration file, because
such a file can select another compiler, patch vstd or change build settings.
Cargo reads `.cargo/config[.toml]` from every parent of the directory it runs
in. The owned shadow workspace lived inside the Sail cache, and the default
cache sits under the user's home directory. Any user with
`~/.cargo/config.toml` therefore had every neutral job refused, with one
diagnostic that did not say which file or which setting would avoid it.

The same file is also the Cargo home configuration when `CARGO_HOME` is unset,
and no placement avoids that.

## Decision

- Shadows stay in `CACHE/neutral-contracts-v1` when no parent of the cache has
  a Cargo configuration file, so existing placements are unchanged.
- Otherwise the shadow root moves to the first private candidate without a
  configured parent: `$XDG_RUNTIME_DIR/sail-neutral-contracts-v1`, then
  `sail-UID-neutral-contracts-v1` in the system temporary directory. A
  candidate is used only if it is a real directory (not a link), owned by the
  current user and writable by no other user. Verifier build output stays in
  `CACHE/neutral-contract-backend-v1`.
- The refusal policy is unchanged. The full configuration check still runs on
  the chosen shadow before metadata and after the backend run. When no
  candidate qualifies, the default placement is used and refused.
- The `SAIL3140` refusal now names its cause and the setting that avoids it:
  a configured Cargo home (`CARGO_HOME`), or a configured cache parent with no
  private placement (`SAIL_CACHE_DIR`).

## Consequences

Neutral jobs run under a configured home directory when `CARGO_HOME` names a
Cargo home without configuration files. A shadow in a runtime directory is
recreated after the directory is cleared; it holds only the generated source,
manifest and lockfile, so this costs little. Another user can only cause a
refusal by planting a configuration file in a shared parent, never an
admission. Non-Unix hosts have no private candidate and keep the actionable
refusal.
