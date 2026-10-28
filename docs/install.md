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
`--install-toolchain` explicitly installs the bundled verifier's Rust pin with
`rust-src` through rustup. Without it, no Rust installation is attempted.
The default toolchain, shell profiles and existing installations are unchanged.
Add the printed `PATH` line to your shell profile. The archive remains together
under the prefix; do not move just its executables.

To install without Python, see [manual installation](#manual-installation).

## Manual installation

Install the prerequisites first: a C toolchain and `curl`. On Debian or
Ubuntu, run `sudo apt-get install -y build-essential curl`. On macOS, run
`xcode-select --install`.

Pick the archive for your platform:

| Platform | Archive |
|---|---|
| Linux x86-64, glibc 2.39 or later | `sail-VERSION-x86_64-unknown-linux-gnu.tar.gz` |
| macOS on Apple silicon | `sail-VERSION-aarch64-apple-darwin.tar.gz` |

Download it with its `.sha256` file, check it, unpack it and put its `bin/`
on `PATH`:

```sh
VERSION=0.1.0                      # the release you are installing
TARGET=x86_64-unknown-linux-gnu    # or aarch64-apple-darwin
base="https://github.com/Chelis-Lang/sail/releases/download/v$VERSION"
curl -fLO "$base/sail-$VERSION-$TARGET.tar.gz"
curl -fLO "$base/sail-$VERSION-$TARGET.tar.gz.sha256"
shasum -a 256 -c "sail-$VERSION-$TARGET.tar.gz.sha256"   # or: sha256sum -c
mkdir -p ~/.local/opt
tar -C ~/.local/opt -xzf "sail-$VERSION-$TARGET.tar.gz"
export PATH="$HOME/.local/opt/sail-$VERSION-$TARGET/bin:$PATH"   # add to your shell profile
sail --version
```

The check must print `OK`. Keep the unpacked directory together: `sail`
finds `sail-z3` beside it in `bin/` and its Verus in `engine/`. On macOS, an
archive downloaded with a browser is quarantined; clear that with
`xattr -dr com.apple.quarantine ~/.local/opt/sail-$VERSION-$TARGET`.

Then install the Rust toolchain Verus pins, with its sources:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup toolchain install 1.99.1 --profile minimal --component rust-src
sail engine status                 # checks Sail's Verus against its pins
```

This doesn't change your default toolchain.

## Rust toolchain and the preverified vstd

`sail verify` runs Cargo with the pinned toolchain on its own, so the
release's preverified `vstd` (recorded with Cargo `1.99.1`) applies whatever
your default Cargo is. A `RUSTUP_TOOLCHAIN` you set, a project's
`rust-toolchain.toml`, a `rustup override` or an explicit `CARGO` takes
precedence, and then `vstd` may be verified again. Sail never installs a
toolchain itself.

The preverified `vstd` applies only to one exact setup:
- the `vstd` version `sail new` writes, with its default dependency
  resolution;
- the pinned toolchain with `rust-src` and no extra components, since
  `rustc-dev` or `llvm-tools` change what rustc loads;
- no forwarded Verus flags other than a supported `--rlimit`.

Otherwise Sail verifies `vstd` in the project, with the same results, and
answers identical solver queries from signed replies shipped in the archive.

Verus flags after `--` reach `vstd` too, as with `cargo verus verify`. To keep
them off `vstd`, pass `--fwd-verus-args-to roots` before `--`, as with stock:
`sail verify --fwd-verus-args-to roots -- --rlimit 100`.

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
forwarded verifier flags can cause this. Verification continues normally, with
exact solver caching still available. Do not rewrite a real project's lockfile
merely to reproduce a benchmark's bundle hit.

To switch back, use your existing `cargo verus verify` command. Remove a Sail
installation by removing its directory and the corresponding `PATH` entry.
Managed project caches can be reclaimed independently with
`sail cache projects prune`; see [disk use](workspace-hygiene.md).
