#!/usr/bin/env python3
"""Stage the native worker and bind its exact installed bytes to its D.I.V.E. release.

The worker is the `sail-runtime-worker` binary of the sail-solve package
(ADR 0172). The root Cargo.toml requires dive-api at one exact version, which
sail_api::temporal::DIVE_VERSION states too.
"""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import sys
import tomllib


def dive_version(manifest):
    """The exact dive-api version a root Cargo.toml text requires.

    Accepts a bare requirement (`dive-api = "=X"`) or a table with `version`.
    """
    dependency = tomllib.loads(manifest)['workspace']['dependencies']['dive-api']
    requirement = dependency if isinstance(dependency, str) else dependency.get('version', '')
    if not isinstance(requirement, str) or not requirement.startswith('=') or not requirement[1:]:
        raise SystemExit(f'dive-api must be required at one exact version, not {requirement!r}')
    return requirement[1:]


def stage(source, destination):
    source, destination = Path(source), Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    worker = destination / 'sail-runtime-worker'
    if source.resolve() != worker.resolve():
        shutil.copyfile(source, worker)
    worker.chmod(0o755)
    revision = dive_version((Path(__file__).resolve().parent.parent / 'Cargo.toml').read_text())
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
