#!/usr/bin/env python3
"""Stage the native worker and bind its exact installed bytes to its source pin."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import sys
import tomllib


def stage(source, destination):
    source, destination = Path(source), Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    worker = destination / 'sail-runtime-worker'
    if source.resolve() != worker.resolve():
        shutil.copyfile(source, worker)
    worker.chmod(0o755)
    pins = tomllib.loads((Path(__file__).resolve().parent.parent / 'Cargo.toml').read_text())
    revision = pins['workspace']['dependencies']['dive-api']['rev']
    system = {'Darwin': 'macos', 'Linux': 'linux'}[platform.system()]
    architecture = {'arm64': 'aarch64', 'aarch64': 'aarch64', 'x86_64': 'x86_64'}[platform.machine()]
    with worker.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    metadata = {'version': 1, 'protocol': 1, 'backend_revision': revision,
                'platform': f'{system}-{architecture}',
                'sha256': digest}
    manifest = destination / 'sail-runtime-worker.json'
    manifest.write_text(json.dumps(metadata, sort_keys=True) + '\n')
    os.chmod(manifest, 0o644)
    return metadata


if __name__ == '__main__':
    if len(sys.argv) != 3:
        raise SystemExit('usage: stage-temporal-worker.py WORKER DESTINATION_BIN_DIRECTORY')
    print(json.dumps(stage(*sys.argv[1:]), sort_keys=True))
