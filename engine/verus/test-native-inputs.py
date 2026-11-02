#!/usr/bin/env python3
"""Observe the actual native Cargo-wrapper path with both Cargo checksum algorithms."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--stock", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    source = out / "native.rs"
    source.write_text('#![no_std]\npub const INPUT: &[u8] = include_bytes!("data.bin");\n')
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith(("SAIL_", "CARGO_")) and "VERUS_DRIVER" not in key}
    # These are Cargo-Verus's ordinary native-wrapper settings, applied to both arms.
    environment.update(__VERUS_DRIVER_VIA_CARGO__="1", RUSTC_BOOTSTRAP="1")
    def identities():
        paths = []
        for executable in (args.stock, args.engine):
            paths += [executable.with_name(name) for name in ("verus", "rust_verify", "vstd.vir", "z3")]
        return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}

    before = identities()
    (out / "identities.json").write_text(json.dumps(before, indent=2) + "\n")
    rows = []
    for algorithm in ("sha256", "blake3"):
        for number, data in enumerate((b"\xff", b"", b"\r\n")):
            (out / "data.bin").write_bytes(data)
            directory = out / f"{algorithm}-{number}"
            directory.mkdir()
            observation = directory / "observed"
            observation.mkdir()
            results = []
            for name, executable in (("stock", args.stock), ("fork", args.engine)):
                env = dict(environment)
                if name == "fork":
                    env.update(SAIL_NATIVE_COMPILER_INPUTS=str(observation),
                               SAIL_NATIVE_COMPILER_INPUTS_BINDING="a" * 64)
                command = [str(executable), "rustc", str(source), "--crate-name", "native_probe",
                           "--crate-type", "lib", "--emit=dep-info,metadata",
                           f"-Zchecksum-hash-algorithm={algorithm}",
                           "-o", str(directory / "native.rmeta")]
                result = subprocess.run(command, cwd=out, env=env, capture_output=True, timeout=120)
                for suffix, value in (("stdout", result.stdout), ("stderr", result.stderr)):
                    (directory / f"{name}.{suffix}").write_bytes(value)
                assert result.returncode == 0, (command, result.stderr)
                depinfo = (directory / "native.d").read_bytes()
                (directory / f"{name}.d").write_bytes(depinfo)
                results.append((result.stdout, result.stderr, depinfo))
            assert results[0] == results[1], (algorithm, number, "compiler streams/dep-info differ")
            document = json.loads((observation / "inputs.json").read_text())
            assert (document["complete"] and document["schema"] == 1
                    and document["kind"] == "sail-native-compiler-read-inputs"
                    and document["binding"] == "a" * 64 and document["pid"] > 0), document
            # Stock's actual compiler emits its independent byte checksums in dep-info.
            expected = {name: {"checksum": checksum, "size": int(size)}
                        for checksum, size, name in re.findall(
                            r"^# checksum:([^ ]+) file_len:([0-9]+) (.+)$",
                            results[0][2].decode(), re.MULTILINE)}
            observed = document["inputs"]["sources"]
            for path, size in ((source, source.stat().st_size), (out / "data.bin", len(data))):
                assert str(path) in expected and str(path) in observed, (expected, observed)
                assert observed[str(path)] == expected[str(path)], (path, expected, observed)
                assert expected[str(path)]["size"] == size
                assert expected[str(path)]["checksum"].startswith(algorithm + "=")
            rows.append({"algorithm": algorithm, "bytes": data.hex(), "sources": observed})
    assert identities() == before, "runtime files changed during native controls"
    (out / "summary.json").write_text(json.dumps(rows, indent=2) + "\n")
    print("PASS: native Cargo-wrapper SHA-256/BLAKE3, binary/empty/CRLF, exact stock streams and dep-info")


if __name__ == "__main__":
    main()
