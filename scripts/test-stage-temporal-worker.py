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
print('temporal worker staging controls passed')
