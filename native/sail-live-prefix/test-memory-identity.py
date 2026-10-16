"""Actual stat parser controls and local poll-shape negatives; no native claim."""
import errno
import json
import os
from pathlib import Path
import sys
from unittest.mock import patch

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import memory_lifecycle as lifecycle


def main():
    rows = []
    def text(comm='worker', state='S', flags='4', start='123', pid='17'):
        fields = ['0'] * 50
        fields[0], fields[6], fields[19] = state, flags, start
        return pid + ' (' + comm + ') ' + ' '.join(fields) + '\n'
    for comm in ['worker', 'with spaces', 'left ( right)', 'nested ) (\ncomm']:
        value = lifecycle.parse_task_identity(text(comm=comm), 17)
        assert value == dict(pid=17, state='S', flags=4, starttime=123)
        rows.append(dict(case='comm-' + repr(comm), passed=True))
    for case, data in [('truncated', '17 (a) S 0 0'), ('wrong-pid', text(pid='18')),
                       ('negative-pid', text(pid='-17')), ('negative-flags', text(flags='-1')),
                       ('negative-start', text(start='-1')), ('zero-start', text(start='0')),
                       ('malformed-start', text(start='bad')), ('malformed-flags', text(flags='x')),
                       ('malformed-state', text(state='?')), ('missing-comm', '17 S ' + '0 ' * 50),
                       ('oversized', text(comm='x' * 65536))]:
        try:
            lifecycle.parse_task_identity(data, 17)
        except (ValueError, IndexError):
            rows.append(dict(case=case, passed=True))
        else:
            raise AssertionError(case)
    for case, link, info in [('ordinary-file', '/tmp/ordinary', 'Pid:\t17\n'),
                              ('foreign-pidfd', 'anon_inode:[pidfd]', 'Pid:\t18\n'),
                              ('malformed-fdinfo', 'anon_inode:[pidfd]', 'Pid:\tbad\n'),
                              ('missing-fdinfo-pid', 'anon_inode:[pidfd]', ''),
                              ('unowned-reaped', 'anon_inode:[pidfd]', 'Pid:\t-1\n')]:
        with patch.object(lifecycle.os, 'readlink', lambda _: link), patch.object(Path, 'read_text', lambda _: info):
            try:
                lifecycle.terminal_pidfd(123, 17, False)
            except ValueError:
                rows.append(dict(case=case, passed=True))
            else:
                raise AssertionError(case)
    for fd in [-1, None]:
        try:
            lifecycle.terminal_pidfd(fd, 17, False)
        except ValueError:
            rows.append(dict(case='invalid-fd-' + str(fd), passed=True))
        else:
            raise AssertionError(fd)
    # Genuine local closed descriptor: Linux-specific observer is not claimed.
    read, write = os.pipe()
    os.close(read)
    try:
        try:
            lifecycle.terminal_pidfd(read, 17, False)
        except OSError as error:
            assert error.errno in {errno.ENOENT, errno.EBADF}
            rows.append(dict(case='actual-closed-fd', passed=True))
        else:
            raise AssertionError('closed fd accepted')
    finally:
        os.close(write)
    print(json.dumps(dict(status='PASS', cases=len(rows), rows=rows,
        scope='Actual parser/local closed fd; Linux FD identity text doubled. Genuine native PID/proc/poll controls pending.')))


if __name__ == '__main__':
    main()
