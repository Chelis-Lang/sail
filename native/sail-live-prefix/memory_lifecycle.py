"""Conservative Linux owned-child terminal memory-observation classification.

An exiting task drops its mm before it becomes waitable. A failed RSS read is
not an RSS sample. Only independently evidenced terminal teardown can end the
sampling interval; the caller must still prove complete successful I/O/status.
"""
import errno
import os
from pathlib import Path
import select
import subprocess
import time

# Linux v6.8 include/linux/sched.h: PF_EXITING; kernel/exit.c do_exit sets it
# in exit_signals before exit_mm and exit_notify. Unsupported evidence refuses.
PF_EXITING = 0x00000004
TERMINAL_WAIT_SECONDS = 0.1


def task_identity(pid):
    """Read kernel PID, flags and starttime; malformed/unavailable data refuses."""
    return parse_task_identity(Path(f'/proc/{pid}/stat').read_text(), pid)


def parse_task_identity(text, pid):
    """Parse the documented stat fields without splitting a parenthesized comm."""
    if len(text) > 65536:
        raise ValueError('oversized task stat')
    head, tail = text.rsplit(')', 1)
    actual_pid = int(head.split('(', 1)[0].strip())
    fields = tail.split()
    if len(fields) < 50:
        raise ValueError('truncated task stat')
    state, flags, starttime = fields[0], int(fields[6]), int(fields[19])
    if actual_pid != pid or pid <= 0 or state not in {'R', 'S', 'D', 'T', 't', 'X', 'Z', 'P', 'I'} or starttime <= 0 or flags < 0:
        raise ValueError('unsupported owned task identity')
    return dict(pid=actual_pid, state=state, flags=flags, starttime=starttime)


def pidfd_binding(pidfd):
    """Retain descriptor inode identity, including after its task is reaped."""
    if not isinstance(pidfd, int) or pidfd < 0:
        raise ValueError('missing owned pidfd')
    info = os.fstat(pidfd)
    return [info.st_dev, info.st_ino, info.st_mode]


def bind_task_identity(pid, pidfd):
    """Bind original starttime to the held, verified live owned pidfd."""
    before = pidfd_binding(pidfd)
    if terminal_pidfd(pidfd, pid, False):
        raise ValueError('child exited before memory identity capture')
    identity = task_identity(pid)
    if pidfd_binding(pidfd) != before:
        raise ValueError('owned pidfd changed during memory identity capture')
    return dict(identity, pidfd_binding=before)


def terminal_pidfd(pidfd, pid, owned_reaped):
    """Require readable owned pidfd; invalid FDs or hangup alone cannot prove exit."""
    if not isinstance(pidfd, int) or pidfd < 0:
        raise ValueError('missing owned pidfd')
    if os.readlink(f'/proc/self/fd/{pidfd}') != 'anon_inode:[pidfd]':
        raise ValueError('descriptor is not a pidfd')
    values = [int(line.split(':', 1)[1].strip())
              for line in Path(f'/proc/self/fdinfo/{pidfd}').read_text().splitlines()
              if line.startswith('Pid:')]
    if len(values) != 1 or (values[0] != pid and not (values[0] == -1 and owned_reaped)):
        raise ValueError('pidfd is not bound to the owned child')
    observer = select.poll()
    observer.register(pidfd, select.POLLIN)
    events = observer.poll(0)
    return bool(events) and all(fd == pidfd and (mask & select.POLLIN) and not (mask & ~(select.POLLIN | select.POLLHUP)) for fd, mask in events)


def terminal_observation(process, pidfd, identity, error, prior_samples, deadline):
    """Retain every observation failure, accepting only proved terminal teardown.

    This does not authorize a sample: caller still validates successful complete
    streams, exit status, image/source identity and every earlier RSS sample.
    No eventual-exit-only exception and no missing-proc inference are permitted.
    """
    result = dict(error=type(error).__name__ + ': ' + str(error),
                  errno=getattr(error, 'errno', None), prior_samples=prior_samples,
                  terminal_race=False, owned_pid=process.pid, identity=identity)
    try:
        if (os.uname().sysname != 'Linux' or not isinstance(error, OSError) or
                error.errno not in {errno.ESRCH, errno.ENOENT} or prior_samples <= 0 or
                not identity or identity['pid'] != process.pid):
            raise ValueError('no eligible terminal-disappearance evidence')
        if pidfd_binding(pidfd) != identity['pidfd_binding']:
            raise ValueError('held pidfd identity changed')
        result['pidfd_terminal_before'] = terminal_pidfd(pidfd, process.pid, process.returncode is not None)
        if not result['pidfd_terminal_before']:
            observed = task_identity(process.pid)
            result['observed_task'] = observed
            if observed['starttime'] != identity['starttime'] or not observed['flags'] & PF_EXITING:
                raise ValueError('owned child is not proven in terminal teardown')
        remaining = min(TERMINAL_WAIT_SECONDS, deadline - time.monotonic())
        if remaining <= 0:
            raise ValueError('terminal observation deadline exceeded')
        result['terminal_exit'] = process.wait(timeout=remaining)
        if pidfd_binding(pidfd) != identity['pidfd_binding']:
            raise ValueError('held pidfd identity changed after terminal wait')
        result['pidfd_terminal_after'] = terminal_pidfd(pidfd, process.pid, process.returncode is not None)
        if result['terminal_exit'] != 0 or not result['pidfd_terminal_after']:
            raise ValueError('owned child did not finish successfully with terminal pidfd')
        result['terminal_race'] = True
    except (OSError, ValueError, KeyError, IndexError, subprocess.SubprocessError) as failure:
        result['refusal'] = type(failure).__name__ + ': ' + str(failure)
    return result
