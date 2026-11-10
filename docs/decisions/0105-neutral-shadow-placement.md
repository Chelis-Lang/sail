# 0105: Place neutral shadows away from inherited Cargo configuration

- **Status:** superseded by [ADR 0109](0109-oyster-retired-v030-scope.md): the surface this decision describes was removed in v0.3.0. The last implementation is tagged `archive/neutral-contracts-final`. Previously: accepted
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
  configured parent: `$XDG_RUNTIME_DIR/sail-CACHE-neutral-contracts-v1`, then
  `sail-UID-CACHE-neutral-contracts-v1` in the system temporary directory,
  where `CACHE` is the first 16 hex digits of the SHA-256 of the cache's
  resolved path. Each cache therefore keeps a root of its own, as it did
  inside the cache, and shadows of different caches or Sail versions never
  meet the immutable-shadow drift check. A candidate is used only if it is a
  real directory (not a link), owned by the current user, with no permission
  bits for group or others. Verifier build output stays in
  `CACHE/neutral-contract-backend-v1`.
- The refusal policy is unchanged. The full configuration check still runs on
  the chosen shadow before metadata and after the backend run. When no
  candidate qualifies, the default placement is used and refused.
- The `SAIL3140` refusal now names its cause and the setting that avoids it:
  a configured Cargo home (`CARGO_HOME`), or a configured cache parent with no
  private placement (`SAIL_CACHE_DIR`).

## Consequences

Neutral jobs run under a configured home directory when `CARGO_HOME` names a
Cargo home without configuration files. Such a Cargo home starts with an empty
registry cache, so the first job downloads the pinned `vstd` from crates.io,
and an offline host must prepare that registry beforehand. Private shadow roots
are outside the cache's size accounting and garbage collection; they hold only
generated source, manifests and lockfiles. A shadow in a runtime directory is
recreated after the directory is cleared; it holds only the generated source,
manifest and lockfile, so this costs little. Another user can only cause a
refusal by planting a configuration file in a shared parent, never an
admission. Non-Unix hosts have no private candidate and keep the actionable
refusal.
