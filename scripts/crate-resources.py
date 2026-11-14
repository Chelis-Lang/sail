#!/usr/bin/env python3
"""Copy authoritative repository inputs into standalone Cargo packages.

Run ``python3 scripts/crate-resources.py sync`` after changing a listed source,
and ``python3 scripts/crate-resources.py check`` before packaging or publishing.
Copies retain the exact bytes; their source files remain authoritative.
"""

import argparse
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parent.parent
RESOURCES = {
    "sail-api": (
        "crates/sail-cli/tests/fixtures/attr/report.golden.json",
        "crates/sail-verus/tests/fixtures/requirement-attribution/body-edit-requested-target.json",
        "crates/sail-verus/tests/fixtures/requirement-attribution/positive-requested-target.json",
    ),
    "sail-cli": (
        "ci/engine-artifact.py",
        "engine/verus/artifact-linux.json",
        "engine/verus/artifact-macos.json",
    ),
    "sail-daemon": (
        "crates/sail-cli/tests/fixtures/attr/report.golden.json",
        "crates/sail-verus/tests/fixtures/requirement-attribution/positive-requested-target.json",
    ),
    "sail-kani": (
        "engine/kani/bundle-linux.json",
    ),
    "sail-mcp": (
        "crates/sail-cli/tests/fixtures/attr/report.golden.json",
    ),
    "sail-portfolio": (
        "crates/sail-smtlib/tests/fixtures/verus-toy.in.smt2",
        "crates/sail-smtlib/tests/fixtures/verus-toy.out.txt",
    ),
    "sail-runtime": (
        "corpus/toolchain.toml",
    ),
    "sail-shim": (
        "corpus/repros/z3-stderr-cache-warning/input.smt2",
        "crates/sail-smtlib/tests/fixtures/verus-toy.in.smt2",
    ),
    "sail-verus": (
        "corpus/repros/verus-ambiguous-friendly-name/input.rs",
        "corpus/toolchain.toml",
        "engine/verus/Cargo.lock",
        "engine/verus/artifact-linux.json",
        "engine/verus/artifact-macos.json",
        "engine/verus/fork.toml",
        "engine/verus/retained-public-inputs.json",
        "engine/verus/retained-public-vest-inputs.json",
        "engine/verus/retained-vest-inputs.json",
        "engine/verus/vstd-boundary-inputs.json",
        "engine/verus/vstd-boundary-vest-inputs.json",
        "scripts/shipped-deps/Cargo.lock",
        "scripts/shipped-deps/Cargo.toml",
    ),
}

# Cargo excludes nested packages; a metadata-only copied manifest must not
# retain the Cargo.toml filename that marks its directory as a package.
RESOURCE_NAMES = {
    "scripts/shipped-deps/Cargo.toml": "scripts/shipped-deps/manifest.toml",
}


def main() -> int:
    """Synchronize or check every package-local copy in a fixed order."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("sync", "check"))
    args = parser.parse_args()
    errors = []
    count = 0
    try:
        for crate, sources in RESOURCES.items():
            directory = ROOT / "crates" / crate / "resources"
            expected = {directory / RESOURCE_NAMES.get(source, source) for source in sources}
            for source in sources:
                destination = directory / RESOURCE_NAMES.get(source, source)
                original = (ROOT / source).read_bytes()
                if args.command == "sync":
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    if not destination.is_file() or destination.read_bytes() != original:
                        destination.write_bytes(original)
                elif not destination.is_file() or destination.read_bytes() != original:
                    errors.append(f"stale or missing: {destination.relative_to(ROOT)}")
                count += 1
            if directory.is_dir():
                for path in sorted(directory.rglob("*")):
                    if (path.is_file() or path.is_symlink()) and path not in expected:
                        errors.append(f"unexpected resource: {path.relative_to(ROOT)}")
    except OSError as error:
        print(f"crate-resources: {error}", file=sys.stderr)
        return 1
    if errors:
        for error in errors:
            print(f"crate-resources: {error}", file=sys.stderr)
        print("Run python3 scripts/crate-resources.py sync after source changes.", file=sys.stderr)
        return 1
    print(f"crate-resources: {count} exact copies {'synchronized' if args.command == 'sync' else 'checked'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
