#!/usr/bin/env python3
"""Show Cargo 1.99 cleaning both same-name versions for a version-specific spec."""

import json
from pathlib import Path
import subprocess
import tempfile


def run(arguments, directory, capture=False):
    return subprocess.run(
        ["cargo", "+1.99.0", *arguments], cwd=directory, check=True, text=True,
        stdout=subprocess.PIPE if capture else None,
    )


with tempfile.TemporaryDirectory(prefix="sail-cargo-clean-versions-") as temporary:
    root = Path(temporary)
    (root / "Cargo.toml").write_text(
        '[workspace]\nresolver = "2"\nmembers = ["app"]\n'
        'exclude = ["old", "new"]\n')
    for directory, version in [("old", "1.0.0"), ("new", "2.0.0")]:
        package = root / directory
        (package / "src").mkdir(parents=True)
        (package / "Cargo.toml").write_text(
            f'[package]\nname = "same-name"\nversion = "{version}"\n'
            'edition = "2021"\n')
        (package / "src/lib.rs").write_text("")
        (package / "build.rs").write_text(
            'fn main() {\n'
            '    println!("cargo:rerun-if-changed=build.rs");\n'
            f'    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))'
            f'.parent().unwrap().join("count-{directory}");\n'
            '    let old: usize = std::fs::read_to_string(&path).ok()'
            '.and_then(|s| s.parse().ok()).unwrap_or(0);\n'
            '    std::fs::write(path, (old + 1).to_string()).unwrap();\n'
            '}\n')
    app = root / "app"
    (app / "src").mkdir(parents=True)
    (app / "src/lib.rs").write_text("")
    (app / "Cargo.toml").write_text(
        '[package]\nname = "app"\nversion = "0.1.0"\nedition = "2021"\n'
        '[dependencies]\nold = { package = "same-name", path = "../old" }\n'
        'new = { package = "same-name", path = "../new" }\n')
    run(["--version", "--verbose"], root)
    run(["build", "--offline"], root)
    metadata = json.loads(run(["metadata", "--offline", "--format-version", "1"],
                              root, capture=True).stdout)
    old_id = next(package["id"] for package in metadata["packages"]
                  if package["name"] == "same-name" and package["version"] == "1.0.0")
    for spec in ["same-name@1.0.0", old_id]:
        before = [int((root / f"count-{name}").read_text()) for name in ("old", "new")]
        run(["clean", "--verbose", "-p", spec], root)
        run(["build", "--offline"], root)
        after = [int((root / f"count-{name}").read_text()) for name in ("old", "new")]
        print(json.dumps({"spec": spec, "before": before, "after": after}), flush=True)
        assert after == [count + 1 for count in before], "Cargo behavior changed; inspect logs"
