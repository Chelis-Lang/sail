#!/usr/bin/env python3
"""Protocol fixture for the caller-owned trusted source proof worker."""
import hashlib
import json
import os
from pathlib import Path
import sys

module, config = map(Path, sys.argv[1:3])
mode = os.environ.get('FIXTURE_MODE', 'proved')
passed = sys.argv[3:]
assert not passed or passed[0] == '--obligation-cache' and len(passed) == 2
receipt = Path(passed[1]) if passed else None
retained = int(receipt is not None and receipt.exists())
row = dict(format='dive-source-obligations-v1', result='proved',
           module_sha256=hashlib.sha256(module.read_bytes()).hexdigest(),
           config_sha256=hashlib.sha256(config.read_bytes()).hexdigest(),
           retained=retained, proof={'fixture_checked': True, 'module_parent': str(module.parent)})
if mode == 'stale':
    row['module_sha256'] = '0' * 64
elif mode == 'mutated-during-check':
    module.write_text('mutated')
elif mode == 'failed':
    row.pop('proof')
    row['result'] = 'inconclusive'
    print(json.dumps(row))
    sys.exit(2)
elif mode == 'false-success':
    row['result'] = 'inconclusive'
elif mode == 'fresh-retained':
    row['retained'] = 1
if receipt is not None and mode == 'proved' and not retained:
    receipt.write_text('fixture cache')
print(json.dumps(row))
