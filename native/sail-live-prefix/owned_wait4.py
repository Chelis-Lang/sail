"""Research-only, single-reaper child accounting; never a proof authority.

Popen.poll()/wait() reap a child without retaining its wait4 resource usage.
This owner instead keeps Popen alive and delegates the sole reap to one thread.
It does not validate an executable, enforce a memory limit, or authorize reuse.
Callers still need the existing image/pidfd, complete I/O and status guards.
"""
import math
import os
import subprocess
import signal
import sys
import threading


def usage_record(usage, platform):
    """Validate and retain native accounting; unsupported units refuse."""
    peak = usage.ru_maxrss
    if type(peak) is not int or peak < 0:
        raise ValueError('invalid kernel peak RSS')
    if platform == 'linux':
        unit, multiplier = 'KiB', 1024
    elif platform == 'darwin':
        unit, multiplier = 'bytes', 1
    else:
        raise ValueError('unsupported wait4 RSS units')
    cpu = [usage.ru_utime, usage.ru_stime]
    if any(type(x) not in (int, float) or not math.isfinite(x) or x < 0 for x in cpu):
        raise ValueError('invalid kernel CPU usage')
    return dict(ru_maxrss=peak, ru_maxrss_unit=unit, peak_rss_bytes=peak * multiplier,
                user_seconds=cpu[0], system_seconds=cpu[1], platform=platform)


class OwnedWait4:
    """Own a child and its sole wait4; never call the underlying Popen waiters.

    The raw Popen remains strongly held until the reaper sets returncode, so
    Popen's destructor cannot become a competing waiter. No global SIGCHLD
    handler or process-wide wait(-1) is installed. Unexpected ECHILD refuses;
    it is never interpreted as successful exit with zero memory.
    """
    def __init__(self, argv, *, deferred=False, **kwargs):
        self._done = threading.Event()
        self._start_lock = threading.Lock()
        self._reaping_started = False
        self._error = None
        self._result = None
        self._observed = None
        # Construct every thread primitive before creating a child. A failed
        # Thread constructor must not leave an ownerless live process/pipes.
        self._reaper = threading.Thread(target=self._reap, name='sail-owned-wait4')
        self._child = subprocess.Popen(argv, **kwargs)
        self.pid, self.args = self._child.pid, self._child.args
        self.stdin, self.stdout, self.stderr = (
            self._child.stdin, self._child.stdout, self._child.stderr)
        if not deferred:
            try:
                self.start_reaper()
            except RuntimeError:
                for stream in (self.stdin, self.stdout, self.stderr):
                    if stream is not None:
                        stream.close()
                raise

    def _kill_unreaped(self):
        # Before the sole reaper starts, this owned child remains unreaped and
        # its PID cannot be reused. Popen.kill() would secretly call poll().
        try:
            os.kill(self.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass

    def start_reaper(self):
        """Release the sole waiter only after the caller's pidfd/image binding.

        A deferred owner must already be assigned to a caller's finally path.
        On thread-start failure, this owner kills/reaps synchronously but leaves
        all streams available for bounded failure capture by that caller.
        """
        with self._start_lock:
            if self._reaping_started:
                raise ChildProcessError('owned reaper already released')
            self._reaping_started = True
            try:
                self._reaper.start()
            except RuntimeError:
                self._kill_unreaped()
                self._reap()
                raise

    def cancel_unstarted(self):
        """Kill/reap this still-unreaped child if binding/startup was refused."""
        with self._start_lock:
            if self._reaping_started:
                return
            self._reaping_started = True
            self._kill_unreaped()
            self._reap()

    def _reap(self):
        try:
            pid, status, usage = os.wait4(self.pid, 0)
            self._observed = dict(pid=pid, status=status, usage={name: repr(getattr(usage, name, None))[:256] for name in ('ru_maxrss', 'ru_utime', 'ru_stime')})
            if pid != self.pid:
                raise ChildProcessError('wait4 returned another child')
            code = os.waitstatus_to_exitcode(status)
            # Reaping succeeded even when later accounting validation refuses.
            # Prevent Popen's destructor from attempting a second wait.
            self._child.returncode = code
            accounting = usage_record(usage, sys.platform)
            self._result = dict(pid=pid, status=status, exit=code, accounting=accounting)
        except (OSError, ValueError) as error:
            self._error = error
        finally:
            self._done.set()

    def _checked(self):
        if self._error is not None:
            raise ChildProcessError('owned wait4 accounting failed') from self._error
        if self._result is None:
            raise ChildProcessError('owned wait4 result missing')
        return self._result

    @property
    def returncode(self):
        """Return only an actual reaped exit status, or None while running."""
        return self.poll()

    def poll(self):
        """Observe saved state without issuing another kernel wait."""
        return self._checked()['exit'] if self._done.is_set() else None

    def wait(self, timeout=None):
        """Wait for the sole reaper; timeout never consumes or invents status."""
        if not self._reaping_started:
            raise ChildProcessError('owned reaper has not been released')
        if not self._done.wait(None if timeout is None else max(0, timeout)):
            raise subprocess.TimeoutExpired(self.args, timeout)
        if self._reaper.ident is not None:
            self._reaper.join()
        return self._checked()['exit']

    def raw_observation(self):
        """Retain native returned facts even when their validation refuses."""
        return self._observed

    def accounting(self):
        """Return a copy of the complete saved record after actual reaping."""
        if not self._done.is_set():
            raise ChildProcessError('child accounting is not complete')
        result = self._checked()
        return dict(result, accounting=dict(result['accounting']))


def positive_accounting(owner, maximum):
    """Require actual exit0 and positive complete kernel peak within this cap."""
    if type(maximum) is not int or maximum <= 0:
        raise ValueError('invalid resource bound')
    result = owner.accounting()
    if result['pid'] != owner.pid or result['exit'] != 0:
        raise ValueError('owned child did not exit successfully')
    peak = result['accounting']['peak_rss_bytes']
    if not 0 < peak <= maximum:
        raise ValueError('kernel lifetime peak RSS refused')
    return result


def drain_unstarted(stream, output, maximum, timeout=2):
    """Bound failed-startup pipe capture without a new thread or hidden waiter.

    Caller already killed/reaped the owned producer and uses this only for a
    stream whose reader thread never started. Missing EOF/overflow/read errors
    remain explicit cleanup failures; every available bounded byte is retained.
    """
    import select
    import time
    errors = []
    if stream.closed:
        return errors
    deadline = time.monotonic() + timeout
    try:
        os.set_blocking(stream.fileno(), False)
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([stream], [], [], max(0, remaining))[0]:
                errors.append('failed-startup output did not reach EOF')
                break
            try:
                block = os.read(stream.fileno(), 65536)
            except BlockingIOError:
                continue
            if not block:
                break
            available = maximum - len(output)
            output.extend(block[:max(0, available)])
            if len(block) > available:
                errors.append('failed-startup output exceeded bound')
                break
    except (OSError, ValueError) as error:
        errors.append('failed-startup output read: ' + str(error))
    return errors
