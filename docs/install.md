# Install Sail

Actual archive checks and their limits are recorded in the
[installer validation](validation/install.md).

Sail currently supports Linux x86-64 with glibc 2.39 or newer (Ubuntu 24.04,
for example), and Apple-silicon macOS. It ships its own Verus and pinned Z3.
Your existing Verus installation is left alone. Sail is distributed as
prebuilt archives on [GitHub releases](https://github.com/Chelis-Lang/sail/releases)
under the MIT licence.

## Guided installation

Install a C toolchain, rustup and Python 3.9 or newer first. Use the installer
from the selected Sail checkout. Choose an explicit version; the installer
does not select a moving latest release:

```sh
python3 scripts/install.py --version VERSION --install-toolchain
```

With authenticated GitHub CLI (`gh`), the installer downloads through it;
otherwise it uses ordinary GitHub release downloads. It never stores
credentials. A downloaded archive can instead be installed offline, with its
adjacent `.sha256` file:

```sh
python3 scripts/install.py --archive ./sail-VERSION-aarch64-apple-darwin.tar.gz \
  --prefix ~/.local/opt
```

When authenticated `gh` is available, the installer first verifies the
archive's [build-provenance attestation](#verify-provenance) and refuses an
archive that fails it. Without signed-in `gh`, it prints a note and continues with the
checksum alone; `--require-attestation` makes the attestation mandatory.
The installer verifies the checksum, checks the platform and archive layout,
and validates the actual bundled runtime before making the installation
available. It refuses an existing installation directory. An interrupted or
failed check does not publish a partial installation; retry the same command.
Every installed file and directory gets exactly the mode the archive records,
whatever your umask, because the shipped vstd proof is admitted only for an
installation that matches the release index's modes.
`--install-toolchain` explicitly installs the bundled verifier's Rust pin with
`rust-src` through rustup. Without it, no Rust installation is attempted.
The default toolchain, shell profiles and existing installations are unchanged.
Add the printed `PATH` line to your shell profile. The archive remains together
under the prefix; do not move just its executables.

The [README](../README.md#install) also gives manual download and extraction
commands requiring no Python.

## Verify provenance

The `.sha256` file comes from the same release as its archive, so it detects
a damaged download but not a replaced one. Each release archive also carries
a GitHub [artifact attestation](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations)
signed for `Chelis-Lang/sail`'s release workflow. To check a downloaded
archive yourself, with GitHub CLI signed in:

```sh
gh attestation verify sail-VERSION-TARGET.tar.gz --repo Chelis-Lang/sail
```

It must report a verified attestation for the archive's SHA-256. Check the
checksum as well (`shasum -a 256 -c sail-VERSION-TARGET.tar.gz.sha256`).
The archives are built by the project's release script, not on GitHub's
runners; the attestation binds each archive's digest to this repository, so a
substituted asset fails it. See [releasing](releasing.md#attestations).

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
forwarded verifier flags can cause this. So can an installation whose
directory modes differ from the release index. An installation made under
umask `002` (Ubuntu's default) by `scripts/install.py` from release candidates
up to 0.1.0-rc.7 is refused this way, with a note that wrongly blames the
toolchain or Verus; reinstall it. Verification continues normally, with
exact solver caching still available. Do not rewrite a real project's lockfile
merely to reproduce a benchmark's bundle hit.

To switch back, use your existing `cargo verus verify` command. Remove a Sail
installation by removing its directory and the corresponding `PATH` entry.
Managed project caches can be reclaimed independently with
`sail cache projects prune`; see [disk use](workspace-hygiene.md).
