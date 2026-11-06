# Temporal packaging

Native temporal jobs link the D.I.V.E. library (`Chelis-Lang/dive`) at a
pinned full revision. Its packages are part of Sail's workspace and are never
published ([ADR 0102](decisions/0102-fold-the-temporal-backend-into-the-workspace.md)):

| Crate | Role |
| --- | --- |
| `sail-dive` | Native jobs, the isolated worker client and `sail_dive::backend::Dive`, the runtime's `TemporalBackend` |
| `sail-runtime-worker` | The isolated native worker process |
| `sail-temporal` | The temporal-enabled `sail` (binary `sail-temporal`): `sail_cli::run_with` with the D.I.V.E. backend |

`sail_api::temporal::DIVE_REVISION` must equal the root manifest's pin; a test
in `sail-dive` checks the manifest and the lock. `NATIVE_SCHEMA_VERSION` moves
with it.

## What ships where

- **Release archives.** `bin/sail` is the temporal-enabled `sail-temporal`,
  with `bin/sail-runtime-worker` and its installation manifest beside it.
  `THIRD-PARTY-NOTICES` covers D.I.V.E. and its dependencies.
- **crates.io.** crates.io refuses git dependencies, so no published crate
  depends on D.I.V.E. or the temporal packages. `cargo xtask check-layering`,
  part of the required `Workspace gate`, enforces this. `cargo install
  sail-solve` builds a `sail` whose capability reports temporal
  `installed: false`: "native temporal support is not included in this sail
  build". That changes when D.I.V.E. publishes its crates to a registry.

## Building

```sh
cargo build --locked -p sail-temporal -p sail-runtime-worker
ci/run.sh extension-install DIR
DIR/bin/sail runtime capabilities
```

`ci/run.sh extension-install DIR` builds the release binaries into `DIR/bin`,
installs `sail-temporal` as `sail`, stages the worker manifest, and fails
unless the installed `sail` reports temporal installed at the pinned
revision. The directory holds no verifier engine, and the step checks only the
temporal capability.

`ci/run.sh extension` runs clippy and tests for the three packages alone;
`ci/run.sh gate` and CI's `Workspace gate` cover them with the rest of the
workspace.

Compiled worker identity and the exact D.I.V.E. revision are checked again at
execution; a successful build alone is not backend qualification.
