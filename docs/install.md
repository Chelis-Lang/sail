# Install Sail

Actual archive checks and their limits are recorded in the
[installer validation](validation/install.md).

Sail currently supports Linux x86-64 with glibc 2.39 or newer (Ubuntu 24.04,
for example), and Apple-silicon macOS. It ships its own Verus and pinned Z3.
Your existing Verus installation is left alone. The repository and release
drafts remain private until the owner's rollout.

## Guided installation

Install a C toolchain, rustup and Python 3.9 or newer first. Use the installer
from the selected Sail checkout. Choose an explicit version; the installer
does not select a moving latest release:

```sh
python3 scripts/install.py --version VERSION --install-toolchain
```

Authenticated GitHub CLI supplies private release access. Without authenticated
`gh`, the installer uses ordinary GitHub release downloads, which become usable
when the owner publishes. It never stores credentials. A downloaded archive
can instead be installed offline, with its adjacent `.sha256` file:

```sh
python3 scripts/install.py --archive ./sail-VERSION-aarch64-apple-darwin.tar.gz \
  --prefix ~/.local/opt
```

The installer verifies the checksum, checks the platform and archive layout,
and validates the actual bundled runtime before making the installation
available. It refuses an existing installation directory. An interrupted or
failed check does not publish a partial installation; retry the same command.
`--install-toolchain` explicitly installs the bundled verifier's Rust pin with
`rust-src` through rustup. Without it, no Rust installation is attempted.
The default toolchain, shell profiles and existing installations are unchanged.
Add the printed `PATH` line to your shell profile. The archive remains together
under the prefix; do not move just its executables.

The [README](../README.md#install) also gives manual download and extraction
commands requiring no Python.

## First verification and existing projects

```sh
sail engine status
sail new --lib demo
cd demo
sail verify
```

`sail engine status` validates and identifies the selected runtime and prints
its `rust_toolchain` pin. Missing Rust sources cause proof-bundle reuse to be
skipped; install that pin with `rust-src` using the command printed by Sail.
An explicit project or environment toolchain selection takes precedence over
Sail's managed Cargo selection.

For an existing compatible project, run `sail verify` with the arguments you
would give `cargo verus verify`. Installation does not edit its source,
manifest or lockfile. Sail supports one Verus version, not arbitrary historical
Verus/vstd combinations. The [compatibility guide](compatibility.md) describes
that pin and its known project limitations.

On a fresh project, a `SAIL3138` note (in `--output json`, or on stderr with
`SAIL_EXPLAIN=1`) explains why a shipped vstd proof cannot be used. A different dependency resolution, extra compiler components or
forwarded verifier flags can cause this. Verification continues normally, with
exact solver caching still available. Do not rewrite a real project's lockfile
merely to reproduce a benchmark's bundle hit.

To switch back, use your existing `cargo verus verify` command. Remove a Sail
installation by removing its directory and the corresponding `PATH` entry.
Managed project caches can be reclaimed independently with
`sail cache projects prune`; see [disk use](workspace-hygiene.md).
