"""Narrow Linux Rustup loader context for the disabled application experiment.

The caller environment is unchanged. This binds actual file-backed mappings of
an owned pinned Z3, not arbitrary loader variables or a general immutable-input
proof. The native image/toolchain is owned and unchanged during the experiment.
"""
import os
import hashlib
from pathlib import Path
import select
import stat

from live_prefix import image_digest, file_stamp


class RustupLoader:
    """Admit only Rustup/Cargo library search paths and held mapped files."""

    def __init__(self, environment, target, solver):
        names = {n for n in environment if n.startswith(('LD_', 'DYLD_'))}
        if names != {'LD_LIBRARY_PATH'}:
            raise ValueError('only explicit Rustup LD_LIBRARY_PATH is supported')
        home_text = environment.get('RUSTUP_HOME')
        value = environment.get('LD_LIBRARY_PATH')
        if (not isinstance(home_text, str) or not home_text or
                not isinstance(value, str) or not 0 < len(value) <= 16384 or
                len(value.split(':')) > 16 or not isinstance(target, (str, Path))):
            raise ValueError('bounded explicit Rustup/target paths required')
        home, target = Path(home_text), Path(target)
        if not home.is_absolute() or not target.is_absolute():
            raise ValueError('absolute bound Rustup/target paths required')
        home, target = home.resolve(strict=True), target.resolve(strict=True)
        self.rustup = home / 'toolchains'
        self.solver = Path(solver).resolve(strict=True)
        allowed = {target / 'debug', target / 'debug' / 'deps', self.solver.parent}
        self.paths = []
        for text in value.split(':'):
            path = Path(text)
            if not text or not path.is_absolute() or not path.is_dir():
                raise ValueError('empty/relative/missing loader directory refused')
            actual = path.resolve(strict=True)
            parts = actual.relative_to(self.rustup).parts if actual.is_relative_to(self.rustup) else ()
            rustup = (len(parts) == 2 and parts[1] == 'lib') or (
                len(parts) == 5 and parts[1:3] == ('lib', 'rustlib') and
                parts[3:] == ('x86_64-unknown-linux-gnu', 'lib'))
            if actual not in allowed and not rustup:
                raise ValueError('foreign loader directory refused: ' + text)
            self.paths.append(dict(literal=text, actual=str(actual)))
        self.held = {}
        self.mapping = None
        self.record = dict(policy='edit-rustup-mapped-files-v1', paths=self.paths,
                           initial=None, final=None, errors=[])

    @staticmethod
    def _alive(pidfd):
        poll = select.poll(); poll.register(pidfd, select.POLLIN)
        if poll.poll(0):
            raise ValueError('owned process is not live during library binding')

    def observe(self, pid, pidfd):
        """Bind before a reply; reject any later mapped-file/segment change."""
        self._alive(pidfd)
        with Path(f'/proc/{pid}/maps').open('rb') as stream:
            raw = stream.read(1024**2 + 1)
        mapping, observations = {}, {}
        self.record['last_maps_sha256'] = hashlib.sha256(raw).hexdigest()
        self.record['last_maps_raw'] = raw.decode('utf-8', errors='replace')
        self.record['last_maps_truncated'] = len(raw) > 1024**2
        self.record['last_observed'] = observations
        try:
            if len(raw) > 1024**2:
                raise ValueError('mapping record cap')
            for line in raw.decode('utf-8', errors='strict').splitlines():
                fields = line.split(None, 5)
                if len(fields) < 5:
                    raise ValueError('malformed process mapping')
                if int(fields[4]) == 0:
                    continue
                if len(fields) != 6 or not fields[5].startswith('/') or fields[5].endswith(' (deleted)'):
                    raise ValueError('unknown/deleted file mapping')
                path = Path(fields[5]); actual = path.resolve(strict=True)
                if path != actual or not (actual.is_relative_to(self.solver.parent) or
                        actual.is_relative_to('/usr/lib') or actual.is_relative_to(self.rustup)):
                    raise ValueError('foreign mapped library: ' + str(path))
                key = str(actual)
                mapping.setdefault(key, []).append((fields[1], fields[2], fields[3], fields[4]))
                if key in observations:
                    continue
                if len(observations) >= 64:
                    raise ValueError('mapped-file count cap')
                fd = self.held.get(key)
                if fd is None:
                    fd = os.open(actual, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC)
                    self.held[key] = fd
                info = os.fstat(fd)
                device = ':'.join(f'{part:x}' for part in (os.major(info.st_dev), os.minor(info.st_dev)))
                mapped_device = ':'.join(f'{int(part, 16):x}' for part in fields[3].split(':'))
                if not stat.S_ISREG(info.st_mode) or info.st_ino != int(fields[4]) or device != mapped_device:
                    raise ValueError('mapped file/held FD identity mismatch')
                stamp = file_stamp(fd)
                if sum(v['stamp'][3] for v in observations.values()) + stamp[3] > 256 * 1024**2:
                    raise ValueError('mapped-file aggregate byte cap')
                observations[key] = dict(stamp=stamp, sha256=image_digest(fd))
            if self.solver.as_posix() not in observations or sum(v['stamp'][3] for v in observations.values()) > 256 * 1024**2:
                raise ValueError('missing executable or mapped-file byte cap')
            self._alive(pidfd)
            if self.mapping is None:
                self.mapping = mapping
                self.record['initial'] = observations
            elif mapping != self.mapping or observations != self.record['initial']:
                raise ValueError('mapped library/file identity changed')
        except (OSError, ValueError, UnicodeError) as error:
            self.record['errors'].append(str(error))
            raise

    def close(self):
        """Retain final full held-file hashes, then release every descriptor."""
        actual, errors = {}, []
        for path, fd in self.held.items():
            try:
                stamp = file_stamp(fd)
                if sum(v['stamp'][3] for v in actual.values()) + stamp[3] > 256 * 1024**2:
                    raise ValueError('final mapped-file aggregate byte cap')
                actual[path] = dict(stamp=stamp, sha256=image_digest(fd))
            except (OSError, ValueError) as error:
                errors.append(path + ': ' + str(error))
        self.record['final'] = actual
        if self.record['initial'] is not None and actual != self.record['initial']:
            errors.append('final mapped-file identity changed')
        for path, fd in self.held.items():
            try:
                os.close(fd)
            except OSError as error:
                errors.append(path + ' close: ' + str(error))
        self.held.clear()
        self.record['errors'].extend(errors)
        if errors:
            raise ValueError('; '.join(errors))
