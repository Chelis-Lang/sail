#!/usr/bin/env python3
"""Compare the Cargo source regions Sail's build-script protocol models (ADR 0084).

    scripts/cargo-protocol-audit.py REFERENCE CANDIDATE...

Each argument is an extracted Cargo source tree at the commit a release's
`cargo --version --verbose` reports, for example from
`https://codeload.github.com/rust-lang/cargo/tar.gz/<commit>`. REFERENCE is a
tree already in `QUALIFIED_CARGO` (crates/sail-verus/src/managed/
build_environment.rs). For every CANDIDATE, each region is printed as
identical or with its differences, after dropping `use` lines and comments
and normalizing the 1.99 module move (`crate::core::` to `crate::workspace::`
and `crate::compiler::`).

Exit status 0 means every region is identical. A difference is not
automatically disqualifying: it needs a written argument (ADR 0084 records
the accepted ones) and the real-Cargo self-test (`ci/run.sh cargo-range`).
"""

import argparse
import difflib
from pathlib import Path
import sys


def find(tree, suffix):
    hits = [
        path
        for path in tree.rglob(suffix.rsplit("/", 1)[-1])
        if path.as_posix().endswith(suffix) and "/tests/" not in path.as_posix()
    ]
    if len(hits) != 1:
        raise LookupError(f"{suffix}: {len(hits)} candidates")
    return hits[0].read_text(encoding="utf-8")


def normalize(text):
    lines = []
    for line in text.splitlines():
        line = line.strip()
        if line.startswith(("use ", "//")):
            continue
        line = line.replace("crate::core::compiler::", "crate::compiler::")
        line = line.replace("crate::core::", "crate::workspace::")
        line = line.replace("HashMap::new()", "HashMap::default()")
        line = line.replace("HashSet::new()", "HashSet::default()")
        lines.append(line)
    return lines


def between(text, start, end):
    first = text.index(start)
    return text[first:text.index(end, first)]


def block(text, start):
    first = text.index(start)
    depth = 0
    for index in range(first, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[first:index + 1]
    raise LookupError(f"{start}: unterminated")


BUILD = "compiler/custom_build.rs"
TARGET_INFO = "build_context/target_info.rs"
REGIONS = {
    # CARGO_FEATURE_* and CARGO_CFG_* (build_environment::generated).
    "script cfg and feature environment": lambda tree: between(
        find(tree, BUILD), "for feat in &unit.features", "Also inform the build script"),
    # OUT_DIR, CARGO_MANIFEST_DIR (build_inputs_package_environment).
    "script package environment": lambda tree: between(
        find(tree, BUILD), 'cmd.env("OUT_DIR"', "for (var, value) in artifact::get_env"),
    "script CARGO_MANIFEST_LINKS": lambda tree: between(
        find(tree, BUILD), "if let Some(links) = unit.pkg.manifest().links()",
        "if let Some(trim_paths)"),
    "CARGO_PKG_* (fill_env)": lambda tree: between(
        find(tree, "compiler/compilation.rs"), 'cmd.env("CARGO_MANIFEST_DIR", pkg.root())',
        "cmd.cwd(pkg.root())"),
    "envify": lambda tree: block(find(tree, "compiler/mod.rs"), "fn envify"),
    # The query Sail's wrapper observes and re-runs with --print=cfg alone.
    "target-info rustc query": lambda tree: between(
        find(tree, TARGET_INFO), "let mut process = rustc.workspace_process();",
        "let (output, error) = rustc"),
    "target-info cfg parse": lambda tree: between(
        find(tree, TARGET_INFO), "let cfg = lines", "// recalculate"),
    "target-info proc_macro filter": lambda tree: between(
        find(tree, TARGET_INFO), "fn not_user_specific_cfg", "/// All the target"),
    "cargo-platform cfg parser": lambda tree: find(tree, "cargo-platform/src/cfg.rs"),
    # build_inputs: script output declarations and the JSON message.
    "rerun-if-* output parsing": lambda tree: between(
        find(tree, BUILD), '"rerun-if-changed" =>', '"metadata" =>'),
    "build-script-executed message": lambda tree: block(
        find(tree, "util/machine_message.rs"), "pub struct BuildScript<"),
    # --unit-graph fields build_environment and build_unit_plan read.
    "unit-graph serialization": lambda tree: find(tree, "compiler/unit_graph.rs"),
    "CompileKind serialization": lambda tree: block(
        find(tree, "compiler/compile_kind.rs"), "impl serde::ser::Serialize for CompileKind"),
    "Profile fields": lambda tree: block(find(tree, "profiles.rs"), "pub struct Profile {"),
    "run-custom-build profile": lambda tree: block(
        find(tree, "profiles.rs"), "pub fn get_profile_run_custom_build"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("reference", type=Path)
    parser.add_argument("candidates", type=Path, nargs="+")
    arguments = parser.parse_args()
    identical = True
    for candidate in arguments.candidates:
        print(f"== {candidate} against {arguments.reference}")
        for name, extract in REGIONS.items():
            try:
                expected = normalize(extract(arguments.reference))
                actual = normalize(extract(candidate))
            except (LookupError, ValueError, OSError) as error:
                print(f"  {name}: cannot extract ({error})")
                identical = False
                continue
            if expected == actual:
                print(f"  {name}: identical")
                continue
            identical = False
            print(f"  {name}: differs")
            for line in difflib.unified_diff(expected, actual, lineterm="", n=0):
                if not line.startswith(("---", "+++")):
                    print(f"    {line}")
    return 0 if identical else 1


if __name__ == "__main__":
    sys.exit(main())
