# Private source dependencies and the temporal extension

Sail's main workspace (the repository root) has no private dependency. A clone
with no credentials builds, tests and lints it, and public release archives
are built from it. Their `sail` reports native temporal jobs unavailable:
"native temporal support is not included in this sail build".

Native temporal jobs link the private D.I.V.E. library (`Chelis-Lang/dive`) at
a pinned full revision. That code lives in a separate Cargo workspace,
[`extensions/temporal/`](../extensions/temporal/Cargo.toml), with its own
`Cargo.lock`, which the root workspace excludes ([ADR 0083](decisions/0083-temporal-extension-workspace.md)):

| Crate | Role |
| --- | --- |
| `sail-dive` | Native jobs, the isolated worker client and `sail_dive::backend::Dive`, the runtime's `TemporalBackend` |
| `sail-runtime-worker` | The isolated native worker process |
| `sail-temporal` | The temporal-enabled `sail`: `sail_cli::run_with` with the D.I.V.E. backend |

The extension uses Sail's crates by path, so its `sail` is the same command
line with the backend linked. Its manifest repeats the root manifest's lints,
package metadata, profiles and shared dependency requirements; keep them in
step. The root `rust-toolchain.toml` and `clippy.toml` apply to it.
`sail_api::temporal::DIVE_REVISION` must equal the extension's pin; a test in
`sail-dive` checks the manifest and the lock.

## Building the extension

With access to `Chelis-Lang/dive` (existing Git credentials work normally):

```sh
cargo build --manifest-path extensions/temporal/Cargo.toml --target-dir target/temporal --locked
python3 scripts/stage-temporal-worker.py target/temporal/debug/sail-runtime-worker target/temporal/debug
target/temporal/debug/sail runtime capabilities
```

`ci/run.sh extension` (or `ci/local.sh extension`) runs the extension's fmt,
clippy and tests. `cargo xtask check-layering` checks both workspaces without
resolving dependencies, so it needs no credential.

## Local check, not CI

Temporal jobs do not ship in 0.1.0, so Sail's CI does not build or test the
extension, and no Sail CI job receives a private-dependency credential. Run
`ci/local.sh extension` (with D.I.V.E. access) before pushing any change to
`extensions/`, `sail-runtime::temporal` or the `TemporalBackend` interface,
and put its result in the PR. Main-workspace CI cannot catch a break in the
extension.

## Credential

Existing credentials with access to both repositories work. For a dedicated
one, create a fine-grained GitHub personal access token restricted to the
private `Chelis-Lang/sail` and `Chelis-Lang/dive` repositories, with
**Contents: read**, and set it as `SAIL_DEPENDENCY_READ_TOKEN` for the local
command. Oyster's CI holds it as an Actions secret of the same name. GitHub
CLI cannot create such a token, and these repositories disable deploy keys, so
create it in GitHub settings; never send it in chat.

`ci/run.sh` gives the credential helper only to its `extension` and `release`
steps. The secret is never a Docker build argument or embedded in an image,
URL, persisted Git configuration or evidence file. The helper answers only
HTTPS requests for these two repositories; requests for other paths or hosts
receive no credential. Cargo invokes Git through its command-line fetcher. To
select the dedicated credential for one local command, set it in the
environment and run, for example:

```sh
scripts/with-private-dependencies.sh cargo fetch --locked --manifest-path extensions/temporal/Cargo.toml
```

Do not paste a token into a command, issue, log or pull request. `ci/local.sh`
forwards an already configured environment credential into the running
container only.

Changing this credential or the helper's readiness flag does not invalidate
an otherwise unchanged CLI proof. Downloaded source bytes remain checked.
Explicit source and build-script reads of those environment variables remain
receipt inputs; see [freshness](freshness.md#which-environment-variables-are-proof-inputs).

With no configured credential, an extension source fetch fails. A missing
private dependency never becomes an unavailable backend that a build silently
skips. Compiled worker identity and the exact D.I.V.E. revision are checked
again at execution; successful source download alone is not backend
qualification.

## Internal temporal-enabled archives

`SAIL_RELEASE_TEMPORAL=1 ci/local.sh release` (Linux, with the credential in
the environment) or, after building the extension's release binaries,
`SAIL_RELEASE_TEMPORAL=1 scripts/release.sh stage OUT ENGINE_BUILD` stages an
archive tree whose `bin/sail` comes from the extension, with
`bin/sail-runtime-worker` and its installation manifest. It is for internal
and Oyster use only: `scripts/release.sh VERSION` refuses to draft a public
release with it set, and its `THIRD-PARTY-NOTICES` is the public one, which
does not list D.I.V.E. or its dependencies.
