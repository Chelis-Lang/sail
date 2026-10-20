#!/usr/bin/env python3
"""Prints the Rust toolchain `sail verify` runs Cargo with (ADR 0058).

The e2e phases compare `sail verify` with stock
`RUSTUP_TOOLCHAIN=<this> cargo verus verify`. It is the caller's
`RUSTUP_TOOLCHAIN` when set, which Sail keeps. Otherwise it is the engine's
pinned toolchain (`sail engine status`), which must be installed with
`rust-src` (the CI image installs it): Sail then selects it for the
temporary fixture projects, which have no toolchain file or rustup override.
A missing pin fails here instead of letting both sides silently fall back to
the default Cargo, which would leave the pinned selection untested.

    ci/stock-toolchain.py SAIL
"""

import json
import os
import subprocess
import sys


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: ci/stock-toolchain.py SAIL")
    caller = os.environ.get("RUSTUP_TOOLCHAIN")
    if caller:
        print(caller)
        return
    status = subprocess.run([sys.argv[1], "engine", "status", "--output", "json"],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if status.returncode != 0:
        raise SystemExit("e2e: cannot read the engine's pinned toolchain: "
                         + status.stderr.decode(errors="replace").strip())
    pinned = json.loads(status.stdout)["rust_toolchain"]
    # Never install anything: a missing toolchain is reported, not fetched.
    components = subprocess.run(
        ["rustup", "component", "list", "--installed", "--toolchain", pinned],
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False,
        env=dict(os.environ, RUSTUP_AUTO_INSTALL="0"))
    if components.returncode != 0 or "rust-src" not in components.stdout.decode().split():
        raise SystemExit(
            f"e2e: Rust {pinned} with rust-src is required for the stock comparison "
            f"(`rustup toolchain install {pinned} --profile minimal --component rust-src`)")
    print(pinned)


if __name__ == "__main__":
    main()
