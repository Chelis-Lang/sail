#!/usr/bin/env python3
"""Actual compiler-loader byte observations, including non-UTF-8 includes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--stock", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True)
    source = out / "proof.rs"
    binary_path = out / "data.bin"
    for number, data in enumerate((b"\xff", b"\xfe", b"a", b"", b"\r\n")):
        previous = binary_path.stat() if binary_path.exists() else None
        binary_path.write_bytes(data)
        if number == 1:
            os.utime(binary_path, ns=(previous.st_atime_ns, previous.st_mtime_ns))
        source.write_text('use vstd::prelude::*;\nverus! { proof fn witness() {\n'
                          'let data = include_bytes!("data.bin"); assert(data.len() == '
                          + str(len(data)) + ');\n} }\nfn main() {}\n')
        for spelling in ("relative", "absolute"):
            label = f"{number}-{spelling}"
            observation = out / label
            observation.mkdir()
            command = [str(source) if spelling == "absolute" else "proof.rs",
                       "--compile", "-Zchecksum-hash-algorithm=sha256", "-o", str(out / "proof")]
            results = []
            for name, executable in (("stock", args.stock), ("fork", args.engine)):
                env = dict(os.environ)
                env.pop("SAIL_COMPILER_INPUTS", None)
                env.pop("SAIL_COMPILER_INPUTS_BINDING", None)
                if name == "fork":
                    env.update(SAIL_COMPILER_INPUTS=str(observation),
                               SAIL_COMPILER_INPUTS_BINDING="a" * 64)
                result = subprocess.run([str(executable), *command], cwd=out, env=env,
                                        capture_output=True, timeout=180)
                (out / (label + "-" + name + ".stdout")).write_bytes(result.stdout)
                (out / (label + "-" + name + ".stderr")).write_bytes(result.stderr)
                assert result.returncode == 0, (label, name, result.returncode, result.stderr)
                results.append(result)
            assert results[0].stdout == results[1].stdout, (label, "stdout differs")
            assert results[0].stderr == results[1].stderr, (label, "diagnostics differ")
            document = json.loads((observation / "inputs.json").read_text())
            assert document["complete"] and document["schema"] == 2, document
            sources = document["phases"]["verification"]["sources"]
            binary = [value for path, value in sources.items() if Path(path).name == "data.bin"]
            assert binary == [{"checksum": "sha256=" + hashlib.sha256(data).hexdigest(),
                               "size": len(data)}], (label, binary)
    print("PASS: compiler-returned non-UTF-8 and UTF-8 bytes; same-size mutation; relative/absolute inputs; exact stock outputs")


if __name__ == "__main__":
    main()
