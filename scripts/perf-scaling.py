#!/usr/bin/env python3
"""Generate reproducible Verus workspaces for dependency and file-count scaling.

Real-project qualification remains separate. These generated programs isolate
the growth of validation work; they are not representative speedup evidence.
"""

import argparse
import json
from pathlib import Path


def edges(shape, index):
    if index == 0:
        return []
    if shape == "chain":
        return [index - 1]
    if shape == "shared":
        return [0]
    offset = index % 3
    return [index - 1, index - 2] if offset == 0 else [index - offset]


def generate(out, shape, packages, files, targets, padding, vstd):
    if min(packages, files, targets) < 1 or padding < 0:
        raise ValueError("packages, files and targets must be positive; padding nonnegative")
    if not (vstd / "Cargo.toml").is_file():
        raise ValueError("--vstd must name the pinned vstd source directory")
    out.mkdir(parents=True, exist_ok=False)
    names = [f"sail-scale-{index:04}" for index in range(packages)]
    graph = {name: [names[dep] for dep in edges(shape, index)]
             for index, name in enumerate(names)}
    depended = {dep for dependencies in graph.values() for dep in dependencies}
    leaves = [name for name in names if name not in depended]
    graph["sail-scale-root"] = leaves
    members = ", ".join(json.dumps("units/" + name) for name in graph)
    (out / "Cargo.toml").write_text(
        '[workspace]\nresolver = "2"\nmembers = [' + members + ']\n'
        'default-members = ["units/sail-scale-root"]\n')
    for name, dependencies in graph.items():
        directory = out / "units" / name
        (directory / "src").mkdir(parents=True)
        manifest = ('[package]\nname = ' + json.dumps(name) + '\nversion = "0.0.0"\n'
                    'edition = "2024"\npublish = false\n\n'
                    '[package.metadata.verus]\nverify = true\n\n[dependencies]\n'
                    'vstd = { path = ' + json.dumps(str(vstd)) + ' }\n')
        for dep in dependencies:
            manifest += dep + ' = { path = "../' + dep + '" }\n'
        modules = []
        for index in range(files if name != "sail-scale-root" else 1):
            module = f"f{index:04}"
            modules.append("pub mod " + module + ";\n")
            body = ('use vstd::prelude::*;\nverus! {\n'
                    'pub open spec fn identity(x: u64) -> u64 { x }\n'
                    'pub fn probe(x: u64) -> (result: u64) ensures result == identity(x) { x }\n')
            for dep in dependencies:
                rust_name = dep.replace("-", "_")
                body += ('pub proof fn uses_' + rust_name + '(x: u64)\n'
                         '    ensures ' + rust_name + '::f0000::identity(x) == x {}\n')
            (directory / "src" / (module + ".rs")).write_text(body + '}\n//' + 'x' * padding + '\n')
        (directory / "src/lib.rs").write_text("".join(modules))
        if name == "sail-scale-root":
            for index in range(targets):
                source = f"src/bin{index:04}.rs"
                (directory / source).write_text(
                    'use vstd::prelude::*;\nverus! {\nfn main() {\n'
                    '    let result = sail_scale_root::f0000::probe(1u64);\n'
                    '    assert(result == 1u64);\n}\n}\n')
                manifest += ('\n[[bin]]\nname = "sail-scale-bin-' + str(index) + '"\n'
                             'path = ' + json.dumps(source) + '\n')
        (directory / "Cargo.toml").write_text(manifest)
    record = {"schema_version": 1, "shape": shape, "packages": packages,
              "files_per_package": files, "root_binary_targets": targets,
              "padding_bytes_per_module": padding, "dependencies": graph}
    (out / "sail-scaling.json").write_text(json.dumps(record, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=lambda value: Path(value).resolve())
    parser.add_argument("--vstd", required=True, type=lambda value: Path(value).resolve())
    parser.add_argument("--shape", choices=("chain", "diamond", "shared"), required=True)
    parser.add_argument("--packages", type=int, default=16)
    parser.add_argument("--files", type=int, default=16)
    parser.add_argument("--targets", type=int, default=1)
    parser.add_argument("--padding", type=int, default=0)
    args = parser.parse_args()
    generate(args.out, args.shape, args.packages, args.files, args.targets, args.padding, args.vstd)


if __name__ == "__main__":
    main()
