#!/usr/bin/env python3
"""Check the native-control Cargo observer's byte, status and refusal fidelity."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    script = Path(__file__).with_name("source-suite-cargo-observer.py")
    rows = []
    with tempfile.TemporaryDirectory(prefix="sail-cargo-observer-") as temporary:
        root = Path(temporary)
        for label, body, payload in [
            ("bytes-exit", "import os,sys;d=sys.stdin.buffer.read();os.write(1,b'first\\x00'+d+b'\\xfflast');os.write(2,b'err\\x00bytes');sys.exit(17)", b"stdin\x00payload"),
            ("signal", "import os,signal;os.write(1,b'before signal');os.kill(os.getpid(),signal.SIGTERM)", b""),
            ("bounded-refusal", "import os;d=b'x'*65536\nfor _ in range(257):os.write(1,d)", b""),
        ]:
            case = root / label
            case.mkdir()
            logs = case / "logs"
            cargo = case / "cargo"
            shutil.copyfile(script, cargo)
            real = case / "real-cargo.py"
            real.write_text(body)
            (case / "observer-config.json").write_text(json.dumps(dict(real_cargo=sys.executable, logs=str(logs))))
            args = [str(real)]
            direct = subprocess.run([sys.executable, *args], input=payload, capture_output=True, check=False, timeout=15)
            observed = subprocess.run([sys.executable, str(cargo), *args], input=payload, capture_output=True, check=False, timeout=15)
            assert (direct.returncode, direct.stdout, direct.stderr) == (observed.returncode, observed.stdout, observed.stderr), label
            metadata = list(logs.glob("*.json"))
            assert len(metadata) == 1
            record = json.loads(metadata[0].read_text())
            assert record["exit"] == direct.returncode
            assert record["stdout_sha256"] == hashlib.sha256(direct.stdout).hexdigest()
            raw = metadata[0].with_suffix(".stdout").read_bytes()
            if label == "bounded-refusal":
                assert not record["complete"] and len(raw) == 16 * 1024**2
                assert len(direct.stdout) > len(raw)
            else:
                assert record["complete"] and raw == direct.stdout
            rows.append(dict(name=label, passed=True, exit=record["exit"], bytes=record["total_bytes"]))
        proxy = root / "proxy"
        proxy.mkdir()
        (proxy / "rustup").write_text("#!/bin/sh\ncase \"$0\" in */cargo) printf 'cargo-proxy\\000bytes'; exit 23;; *) exit 29;; esac\n")
        (proxy / "rustup").chmod(0o700)
        (proxy / "cargo").symlink_to("rustup")
        observer = root / "proxy-observer"
        observer.mkdir()
        shutil.copyfile(script, observer / "cargo")
        (observer / "observer-config.json").write_text(json.dumps(dict(real_cargo=str(proxy / "cargo"), logs=str(observer / "logs"))))
        direct = subprocess.run([str(proxy / "cargo")], capture_output=True, check=False, timeout=15)
        observed = subprocess.run([sys.executable, str(observer / "cargo")], capture_output=True, check=False, timeout=15)
        assert (direct.returncode, direct.stdout, direct.stderr) == (observed.returncode, observed.stdout, observed.stderr)
        assert direct.returncode == 23
        rows.append(dict(name="multicall-proxy-literal-argv0", passed=True))
        # Admission overflow also forwards exact output/status, but cannot
        # produce a complete evidence record that a control might accept.
        logs = case / "logs"
        (logs / "counter").write_text("64")
        overflow = subprocess.run([sys.executable, str(cargo), "-c", "import sys;print('overflow');sys.exit(9)"], capture_output=True, check=False, timeout=15)
        assert overflow.returncode == 9 and overflow.stdout == b"overflow\n" and not overflow.stderr
        assert (logs / "overflow").exists() and len(list(logs.glob("*.json"))) == 1
        rows.append(dict(name="count-refusal", passed=True))
    print(json.dumps(dict(scope="observer only, owned child doubles, no Cargo/Verus acceptance", rows=rows), indent=2))


if __name__ == "__main__":
    main()
