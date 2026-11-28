#!/usr/bin/env python3
"""Worker staging shape and exact content identity; no verification claim."""
import importlib.util
import hashlib
import json
from pathlib import Path
import tempfile

spec = importlib.util.spec_from_file_location('stage_worker', Path(__file__).with_name('stage-temporal-worker.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
with tempfile.TemporaryDirectory(prefix='sail-worker-stage-') as temporary:
    root = Path(temporary)
    source = root/'worker'
    source.write_bytes(b'fixture worker bytes')
    target = root/'bin'
    metadata = module.stage(source, target)
    assert metadata['sha256'] == hashlib.sha256(source.read_bytes()).hexdigest()
    # The exact D.I.V.E. release, as sail_api::temporal::DIVE_VERSION states it.
    api = (Path(__file__).resolve().parent.parent/'crates/sail-api/src/temporal.rs').read_text()
    assert f'pub const DIVE_VERSION: &str = "{metadata["backend_revision"]}";' in api, metadata
    assert json.loads((target/'sail-runtime-worker.json').read_text()) == metadata
    assert (target/'sail-runtime-worker').stat().st_mode & 0o777 == 0o755
    assert module.stage(target/'sail-runtime-worker', target) == metadata
# The pin may be a bare crates.io requirement or a table; either must be exact.
def manifest(dependency):
    return f'[workspace.dependencies]\ndive-api = {dependency}\n'
assert module.dive_version(manifest('"=0.2.0"')) == '0.2.0'
assert module.dive_version(manifest('{ version = "=0.2.0", git = "https://example.invalid", rev = "0" }')) == '0.2.0'
for loose in ['"0.2.0"', '"^0.2"', '"="', '{ git = "https://example.invalid" }', '{ version = 2 }']:
    try:
        module.dive_version(manifest(loose))
    except SystemExit:
        pass
    else:
        raise AssertionError(f'accepted inexact dive-api requirement {loose}')
print('temporal worker staging controls passed')
