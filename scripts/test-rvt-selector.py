#!/usr/bin/env python3
"""Exercise source selection through a built conformance binary, without solving.

Requires Python 3.11 or newer. Fixture executables only list a marker test. Their manifests are deliberately
synthetic: these controls qualify routing/refusal, never a Verus runtime. Run
from the matching source checkout with its pinned Rust toolchain installed.
"""

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import tomllib


def sha256(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--z3", type=Path, required=True, help="actual pinned Z3 file")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    binary, z3 = args.binary.resolve(strict=True), args.z3.resolve(strict=True)
    root = Path(__file__).resolve().parent.parent
    pins = tomllib.loads((root / "corpus/toolchain.toml").read_text())
    fork = tomllib.loads((root / "engine/verus/fork.toml").read_text())["fork"]
    host = {
        ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
        ("Darwin", "arm64"): "aarch64-apple-darwin",
    }[(platform.system(), platform.machine())]
    asset = pins["assets"][host]
    assert sha256(z3) == asset["z3_sha256"], "controls need the actual pinned solver"
    features = [
        f"sail-fork:{fork['commit']}",
        f"host:{host}",
        f"source-lock:{sha256(root / 'engine/verus/Cargo.lock')}",
        "vstd-build:vargo",
        "suite:rust-verify-test+toolchain-manifest",
    ]
    rows = []
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sail-rvt-selector-") as temporary:
        home = Path(temporary)
        cache = home / "cache"
        release = home / "release" / asset["dir"]
        release.mkdir(parents=True)
        (release / "verus").write_text("release fixture; never executed\n")
        (release / "z3").symlink_to(z3)
        env = {k: v for k, v in os.environ.items() if not k.startswith("SAIL_")}
        env.update(SAIL_CACHE_DIR=str(cache), SAIL_VERUS_HOME=str(release.parent))

        def fixture(build, identity, marker="selected"):
            runtime = build / "verus/source/target-verus/release"
            runtime.mkdir(parents=True)
            (build / "verus/source/rust_verify_test").mkdir()
            for name in ("verus", "cargo-verus"):
                (runtime / name).write_text("fixture; never executed\n")
            (runtime / "z3").symlink_to(z3)
            test = build / "list-tests"
            test.write_text(
                '#!/bin/sh\ncase " $* " in *" --ignored "*) exit 0;; esac\n'
                f"printf '%s\\n' '{marker}: test'\n"
            )
            test.chmod(0o700)
            manifest = dict(
                schema_version=1,
                commit=pins["verus"]["commit"],
                rust_toolchain=pins["verus"]["rust_toolchain"],
                z3_sha256=asset["z3_sha256"],
                features=identity,
                artifacts={str(p.relative_to(build)): sha256(p)
                           for p in (*runtime.iterdir(), test)},
                build_seconds=0,
                test_binaries=[dict(name="selector", path="list-tests", sha256=sha256(test))],
            )
            (build / "sail-build.json").write_text(json.dumps(manifest))
            return manifest

        selected = home / "explicit-suite"
        correct = fixture(selected, features)
        stock = cache / "verus-src-build" / pins["verus"]["commit"]
        fixture(stock, [], "stock_default")
        custom_stock = home / "ordinary-source"
        fixture(custom_stock, [], "custom_stock_override")
        singular = home / "singular-source"
        fixture(singular, ["singular"], "singular_override")
        engine_source = home / "engine-source"
        fixture(engine_source, features[:3] + ["vstd-build:cargo-verus-release"], "engine_override")
        variant_id = (
            f"{pins['verus']['commit']}:{pins['verus']['rust_toolchain']}:"
            + json.dumps(features)
        )
        default_fork = cache / "verus-src-build" / (
            pins["verus"]["commit"] + "-"
            + hashlib.sha256(variant_id.encode()).hexdigest()[:16]
        )
        fixture(default_fork, features, "fork_default")

        def check(name, baseline="fork", selector=selected, error=None, marker="selected"):
            child_env = dict(env)
            if selector is not None:
                child_env["SAIL_VERUS_SOURCE_BUILD"] = str(selector)
            command = [str(binary), "rust-verify-test", "--list"]
            if baseline is not None:
                command += ["--baseline", baseline]
            result = subprocess.run(command, cwd=root, env=child_env,
                                    capture_output=True, text=True, timeout=30, check=False)
            good = (result.returncode == 0 and result.stdout.strip() == f"selector::{marker}")
            if error is not None:
                good = result.returncode != 0 and error in result.stderr and not result.stdout
            rows.append(dict(name=name, command=command, returncode=result.returncode,
                             stdout=result.stdout, stderr=result.stderr, passed=good))
            args.out.write_text(json.dumps(dict(scope="selector fixtures only", rows=rows), indent=2))

        check("implicit fork accepts explicit fork suite", baseline=None)
        check("explicit fork accepts explicit fork suite")
        check("stock installer still rejects fork source", baseline="stock", error="SAIL3040")
        check("stock rejects engine source variant", baseline="stock", selector=engine_source, error="SAIL3040")
        check("fork rejects engine source variant", selector=engine_source, error="SAIL3040")
        for name, field, value in [
            ("fork rejects stock source", "features", []),
            ("fork rejects wrong fork identity", "features", ["sail-fork:wrong"]),
            ("fork rejects reordered features", "features", list(reversed(features))),
            ("fork rejects wrong upstream", "commit", "wrong"),
            ("fork rejects wrong toolchain", "rust_toolchain", "wrong"),
            ("fork rejects wrong solver", "z3_sha256", "wrong"),
            ("fork rejects corrupt artifact", "artifacts", {"list-tests": "wrong"}),
            ("fork rejects missing test binary", "test_binaries", [dict(name="missing", path="missing")]),
            ("fork rejects legacy digestless test binary", "test_binaries", [dict(name="selector", path="list-tests")]),
            ("fork rejects changed test binary digest", "test_binaries", [dict(name="selector", path="list-tests", sha256="0" * 64)]),
        ]:
            changed = copy.deepcopy(correct)
            changed[field] = value
            (selected / "sail-build.json").write_text(json.dumps(changed))
            check(name, error="SAIL3040")
        (selected / "sail-build.json").write_text(json.dumps(correct))
        check("fork default source variant", selector=None, marker="fork_default")
        check("stock default source unchanged", baseline="stock", selector=None, marker="stock_default")
        check("stock explicit ordinary suite override unchanged", baseline="stock", selector=custom_stock, marker="custom_stock_override")
        check("stock Singular suite override unchanged", baseline="stock", selector=singular, marker="singular_override")
        check("fork missing selector refuses", selector=home / "missing", error="SAIL3040")
        check("stock missing override refuses", baseline="stock", selector=home / "missing", error="SAIL3040")
        (release / "verus").unlink()
        check("fork still requires installed release", error="SAIL3003")
    failed = [r["name"] for r in rows if not r["passed"]]
    print(f"source selector controls: {len(rows) - len(failed)}/{len(rows)} pass")
    if failed:
        raise SystemExit("failed: " + ", ".join(failed))


if __name__ == "__main__":
    main()
