# sail-solve

Sail makes Verus verification fast and usable from the command line and coding
agents. Verify an existing Rust file or Cargo project, watch edits, retain
successful proofs, and independently validate focused repairs.

Install the first crates.io prerelease with Rust 1.99 or newer, a C toolchain,
Rustup and Python 3:

```sh
rustup toolchain install 1.99.1 --profile minimal --component rust-src
cargo +1.99.1 install sail-solve --version 0.2.0-rc.1 --locked
sail engine install
```

Save this as `proof.rs`:

```rust
use vstd::prelude::*;
verus! {
    proof fn arithmetic() {
        assert(1 + 1 == 2);
    }
}
fn main() {}
```

```sh
sail verify proof.rs
sail watch proof.rs
```

Verification reports the pinned Verus verdicts and diagnostics. Changing the
assertion to `1 + 1 == 3` produces a verification error; unchanged successful
inputs can reuse their established proof. `sail mcp` exposes verification and
jobs over stdio for coding agents. Run `sail --help` for the CLI commands.

The package installs `sail` and its `sail-solve-z3` companion. Engine installation
downloads Sail's exact pinned Verus/Z3 runtime and validates the archive digest,
provenance and every runtime file. Supported runtime platforms are Linux x86-64
with glibc 2.39 or newer and macOS on Apple silicon. No source checkout or
separate Verus installation is needed.

The first registry version is a prerelease while full release qualification
continues. Cargo installations build the CLI from source and establish their
own proofs; GitHub release archives additionally provide preverified vstd and
signed solver replies. Both routes keep the same proof-admission checks.

[Installation](../../docs/install.md) ·
[Standalone files](../../docs/standalone-verification.md) ·
[Background preparation](../../docs/background-preparation.md) ·
[MCP](../../docs/mcp.md) ·
[Project](../../README.md)
