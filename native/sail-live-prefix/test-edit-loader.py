#!/usr/bin/env python3
"""Real held-file negatives; /proc maps and pidfd APIs explicitly doubled."""
import hashlib
import io
import json
import os
from pathlib import Path
import tempfile
from unittest.mock import patch

from edit_loader import RustupLoader
import live_prefix

rows = []
with tempfile.TemporaryDirectory(prefix='sail-edit-loader-') as td:
    root = Path(td).resolve()
    home = root / 'rustup'
    lib = home / 'toolchains' / 'fixture' / 'lib'
    lib.mkdir(parents=True)
    target = root / 'target'
    (target / 'debug' / 'deps').mkdir(parents=True)
    runtime = root / 'runtime'
    runtime.mkdir()
    solver = runtime / 'z3'
    solver.write_bytes(b'owned solver fixture')
    library = lib / 'libfixture.so'
    library.write_bytes(b'owned mapped fixture')
    env = dict(RUSTUP_HOME=str(home), LD_LIBRARY_PATH=str(lib) + ':' + str(target / 'debug' / 'deps'))
    def mapped(path):
        v = path.stat()
        device = f'{os.major(v.st_dev):x}:{os.minor(v.st_dev):x}'
        return f'1000-2000 r-xp 00000000 {device} {v.st_ino} {path}\n'.encode()
    maps = [mapped(solver) + mapped(library)]
    original_open = Path.open
    def proc(path, *args, **kwargs):
        return io.BytesIO(maps[0]) if str(path) == '/proc/123/maps' else original_open(path, *args, **kwargs)
    def refused(name, callback):
        try:
            callback()
        except (OSError, ValueError):
            rows.append(dict(name=name, passed=True))
        else:
            raise AssertionError(name + ' unexpectedly admitted')
    for name, changed in [
            ('preload', dict(env, LD_PRELOAD='foreign.so')),
            ('audit', dict(env, LD_AUDIT='foreign.so')),
            ('dyld', dict(env, DYLD_LIBRARY_PATH=str(lib))),
            ('relative', dict(env, LD_LIBRARY_PATH='.')),
            ('empty-component', dict(env, LD_LIBRARY_PATH=str(lib) + ':')),
            ('foreign-directory', dict(env, LD_LIBRARY_PATH=str(root))),
            ('missing-rustup', dict(LD_LIBRARY_PATH=str(lib)))]:
        refused(name, lambda changed=changed: RustupLoader(changed, target, solver))
    # Default research pool still refuses the same loader environment before
    # allocating any child; the edit-only target admission must be explicit.
    with patch.object(live_prefix.os, 'uname', return_value=type('U', (), {'sysname':'Linux'})()), \
            patch.object(live_prefix.os, 'pidfd_open', create=True), \
            patch.object(live_prefix.signal, 'pidfd_send_signal', create=True):
        refused('default-pool-loader-refusal', lambda: live_prefix.OneUsePool(
            solver, hashlib.sha256(solver.read_bytes()).hexdigest(), environment=env))
    with patch.object(Path, 'open', proc), patch.object(RustupLoader, '_alive'):
        owner = RustupLoader(env, target, solver)
        owner.observe(123, 4)
        held = list(owner.held.values())
        owner.observe(123, 4)
        owner.close()
        assert owner.record['initial'] == owner.record['final'] and not owner.record['errors']
        for fd in held:
            try: os.fstat(fd)
            except OSError: pass
            else: raise AssertionError('held library FD leaked')
        rows.append(dict(name='held-actual-files-positive-and-close', passed=True))
        owner = RustupLoader(env, target, solver)
        owner.observe(123, 4)
        library.write_bytes(b'changed actual mapped fixture')
        refused('actual-library-write', lambda: owner.observe(123, 4))
        refused('final-held-file-change', owner.close)
        library.write_bytes(b'owned mapped fixture')
        owner = RustupLoader(env, target, solver)
        maps[0] = b'x' * (2 * 1024**2)
        refused('bounded-mapping-read', lambda: owner.observe(123, 4))
        assert len(owner.record['last_maps_raw']) == 1024**2 + 1 and owner.record['last_maps_truncated']
        owner.close()
        owner = RustupLoader(env, target, solver)
        maps[0] = mapped(library)
        refused('missing-actual-executable', lambda: owner.observe(123, 4))
        owner.close()
        owner = RustupLoader(env, target, solver)
        foreign = root / 'foreign.so'; foreign.write_bytes(b'foreign')
        maps[0] = mapped(solver) + mapped(foreign)
        refused('foreign-mapped-file', lambda: owner.observe(123, 4))
        assert owner.record['last_maps_raw'] and owner.record['errors']
        owner.close()
        owner = RustupLoader(env, target, solver)
        maps[0] = mapped(solver) + mapped(library)
        owner.observe(123, 4)
        first, second = list(owner.held.values())
        os.close(first)
        refused('invalid-held-fd-final-refusal', owner.close)
        try: os.fstat(second)
        except OSError: pass
        else: raise AssertionError('later valid FD leaked after invalid descriptor')
        assert not owner.held
print(json.dumps(dict(passed=True, cases=rows, scope=__doc__), indent=2))
