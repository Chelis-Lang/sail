"""Research-only, single-reaper child accounting; never a proof authority.

Popen.poll()/wait() reap a child without retaining its wait4 resource usage.
This owner instead keeps Popen alive and delegates normal reaping to one thread.
It does not validate an executable, enforce a memory limit, or authorize reuse.
Callers still need the existing image/pidfd, complete I/O and status guards.
"""
import math
import os
import resource
import subprocess
import signal
import sys
import threading
import time


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


class _OwnedChild(subprocess.Popen):
    """No implicit bare-PID destructor waiter; the assigned owner reaps.

    This POSIX research owner explicitly closes all streams and reaps through
    wait4 or its error-only held-pidfd cleanup. Missing accounting must not let
    Popen.__del__ silently wait against an old PID after ownership is lost.
    """
    def __del__(self):
        pass


class OwnedWait4:
    """Own a child and its sole wait4; never call the underlying Popen waiters.

    The raw Popen stays strongly held and its automatic destructor waiter is
    disabled, including when accounting fails and status remains unknown. No global SIGCHLD
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
        self._cleanup_observed = None
        # Construct every thread primitive before creating a child. A failed
        # Thread constructor must not leave an ownerless live process/pipes.
        self._reaper = threading.Thread(target=self._reap, name='sail-owned-wait4')
        # Observe the actual spawning process without changing the child's
        # complete-lifetime resource criterion or the spawn mechanism.
        parent = usage_record(resource.getrusage(resource.RUSAGE_SELF), sys.platform)
        self.parent_before_spawn = dict(peak_rss_bytes=parent['peak_rss_bytes'],
                                        resident_bytes=None, resident_source=None)
        if sys.platform == 'linux':
            with open('/proc/self/statm', 'rb') as stream:
                raw = stream.read(4097)
            fields = raw.split()
            if len(raw) > 4096 or len(fields) < 2 or not fields[1].isdigit():
                raise ValueError('bounded parent statm observation refused')
            resident = int(fields[1]) * os.sysconf('SC_PAGE_SIZE')
            if resident <= 0:
                raise ValueError('positive parent resident observation required')
            self.parent_before_spawn.update(resident_bytes=resident,
                                            resident_source='/proc/self/statm')
        self._child = _OwnedChild(argv, **kwargs)
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
            self._result = dict(pid=pid, status=status, exit=code, accounting=accounting,
                                parent_before_spawn=self.parent_before_spawn)
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

    def cleanup_failed_accounting(self, pidfd, timeout=2):
        """Kill/reap only the held original pidfd after the sole waiter failed.

        waitid supplies actual termination status, not rusage. Accounting stays
        refused, even with actual exit0. Never wait or signal a bare possibly
        reused PID, and never compete with an unfinished wait4 owner.
        """
        if (not self._reaping_started or not self._done.is_set() or
                self._error is None or type(pidfd) is not int or pidfd < 0 or
                not 0 < timeout <= 2):
            raise ChildProcessError('failed-accounting cleanup preconditions refused')
        if self._reaper.ident is not None:
            self._reaper.join(timeout=timeout)
            if self._reaper.is_alive():
                raise ChildProcessError('sole reaper still active')
        record = dict(accounting_available=False, pid=self.pid, errors=[])
        self._cleanup_observed = record
        # Even poll() raises on the saved accounting error. Signal this held
        # original descriptor independently of poll/returncode.
        try:
            signal.pidfd_send_signal(pidfd, signal.SIGKILL)
            record['signal'] = 'sent-held-pidfd-SIGKILL'
        except ProcessLookupError:
            record['signal'] = 'held-pidfd-already-terminated'
        except OSError as error:
            record['errors'].append('held-pidfd signal: ' + str(error))
        if not hasattr(os, 'P_PIDFD') or not hasattr(os, 'waitid'):
            record['errors'].append('native P_PIDFD waitid unavailable')
            raise ChildProcessError('safe descriptor reap unavailable')
        deadline = time.monotonic() + timeout
        while True:
            try:
                observed = os.waitid(os.P_PIDFD, pidfd, os.WEXITED | os.WNOHANG)
            except ChildProcessError as error:
                # ECHILD alone is not a successful status. This originally
                # bound descriptor must separately be terminal to establish
                # that the old child is no longer live/waitable by this owner.
                import select
                poll = select.poll();poll.register(pidfd, select.POLLIN)
                events = poll.poll(0)
                record['terminal_poll_events'] = [list(event) for event in events]
                # A pidfd becomes readable on termination; HUP may accompany
                # reaping. ERR/NVAL or any unrequested event is not evidence
                # of original-child termination.
                terminal = (len(events) == 1 and events[0][0] == pidfd and
                            bool(events[0][1] & select.POLLIN) and
                            not events[0][1] & ~(select.POLLIN | select.POLLHUP))
                record.update(waitid_error=str(error), terminal_pidfd=terminal,
                              no_longer_waitable=True)
                if not terminal:
                    record['errors'].append('nonterminal held pidfd with ECHILD')
                    raise ChildProcessError('accounting error cleanup incomplete') from error
                return dict(record)
            if observed is not None:
                record['waitid'] = dict(si_pid=observed.si_pid, si_status=observed.si_status,
                                        si_code=observed.si_code, si_signo=observed.si_signo)
                if observed.si_pid != self.pid:
                    record['errors'].append('held pidfd wait returned unexpected original PID')
                    raise ChildProcessError('descriptor termination identity refused')
                if observed.si_code == os.CLD_EXITED:
                    code = observed.si_status
                elif observed.si_code in (os.CLD_KILLED, os.CLD_DUMPED):
                    code = -observed.si_status
                else:
                    record['errors'].append('waitid did not return termination')
                    raise ChildProcessError('descriptor termination status refused')
                self._child.returncode = code
                record['actual_exit'] = code
                return dict(record)
            if time.monotonic() >= deadline:
                record['errors'].append('held-pidfd reap deadline exceeded')
                raise subprocess.TimeoutExpired(self.args, timeout)
            time.sleep(.005)

    def cleanup_observation(self):
        """Return error-only reap facts; they never substitute for rusage."""
        return self._cleanup_observed

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
