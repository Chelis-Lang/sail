#!/usr/bin/env python3
"""Actual private files and literal readback; no native resource/perf claim."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
from unittest.mock import patch

from edit_claim import LiteralInputFile
from edit_stream import ProtocolRefusal
from edit_workflow import Workflow

rows = []
with tempfile.TemporaryDirectory(prefix='sail-input-', dir='/tmp') as directory:
    root = Path(directory)
    owned = LiteralInputFile(root)
    data = b'a' * 65535 + b'B' + b'c' * 131073
    owned.extend(data[:17])
    owned.extend(data[17:])
    assert owned.matches(data)
    assert not owned.matches(data[:-1])
    assert not owned.matches(data[:65535] + b'X' + data[65536:])
    assert max(map(len, owned.chunks())) <= 65536
    assert os.fstat(owned.stream.fileno()).st_mode & 0o777 == 0o600
    rows.append(dict(name='literal-positive-byte-order-length-and-private-file', passed=True))
    with patch('edit_claim.MAX_BYTES', len(data)):
        try:
            owned.extend(b'x')
            raise AssertionError('overflow accepted')
        except ProtocolRefusal:
            pass
    assert owned.matches(data)
    rows.append(dict(name='same-input-cap-refuses-before-write', passed=True))
    workflow = Workflow(root / 'out', __file__, hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                        ['unused'], root)
    record = workflow.raw_input_file(workflow.output, 'child', owned)
    assert record['bytes'] == len(data)
    assert (workflow.output / record['path']).read_bytes() == data
    assert workflow.copied == len(data)
    rows.append(dict(name='whole-child-capture-readback', passed=True))
    owned.stream.seek(65535)
    owned.stream.write(b'X')
    assert not owned.matches(data)
    rows.append(dict(name='actual-file-byte-mutation-refuses-literal-equality', passed=True))
    owned.stream.truncate(4)
    try:
        list(owned.chunks())
        raise AssertionError('truncated input accepted')
    except ProtocolRefusal:
        pass
    rows.append(dict(name='actual-file-truncation-refuses', passed=True))
    owned.close()
    assert owned.stream.closed
    rows.append(dict(name='owned-descriptor-closed', passed=True))
print(json.dumps(rows, indent=2))
