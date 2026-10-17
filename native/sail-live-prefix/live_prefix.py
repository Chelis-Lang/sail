#!/usr/bin/env python3
"""Disabled one-use exact-prefix research prototype (ADR 0040 option A).

No fork, preload, extra SMT command, rewind, normalization or proof cache.
Callers must supply a complete command prefix and its exact expected stdout.
Preparation replays that entire prefix on a new pinned solver and costs real
CPU. A claimed process receives the original suffix exactly once.
"""
import hashlib
import json
import os
from pathlib import Path
import resource
import select
import signal
import stat
import subprocess
import threading
import time
from types import MappingProxyType
import response_policy


MAX_INPUT = 2 * 1024 * 1024
MAX_INPUT_LIMIT = 16 * 1024 * 1024
MAX_OUTPUT = 16 * 1024 * 1024
MAX_ENGINE = 512 * 1024 * 1024


def file_stamp(fd):
    info = os.fstat(fd)
    if not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= MAX_ENGINE:
        raise ValueError("unsupported executable image")
    return (info.st_dev, info.st_ino, info.st_mode, info.st_size,
            info.st_mtime_ns, info.st_ctime_ns)


def image_digest(fd):
    before = file_stamp(fd)
    value = hashlib.sha256()
    offset = 0
    while block := os.pread(fd, 65536, offset):
        offset += len(block)
        if offset > MAX_ENGINE:
            raise ValueError("executable image grew beyond budget")
        value.update(block)
    if file_stamp(fd) != before or offset != before[3]:
        raise ValueError("executable image changed while hashing")
    return value.hexdigest()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def path_digest(path):
    fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        return image_digest(fd)
    finally:
        os.close(fd)


def capture_running_image(pid, expected):
    """Hold and verify the kernel's actual Linux executable before input.

    The pidfd distinguishes this live child from any later reuse of its pid.
    Caller owns both returned descriptors and retains them until completion.
    """
    pidfd = image = None
    try:
        pidfd = os.pidfd_open(pid, 0)
        observer = select.poll()
        observer.register(pidfd, select.POLLIN)
        if observer.poll(0):
            raise ValueError("child exited before executable capture")
        image = os.open(f"/proc/{pid}/exe", os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC)
        stamp = file_stamp(image)
        if image_digest(image) != expected or observer.poll(0):
            raise ValueError("actual running executable image mismatch")
        return pidfd, image, stamp
    except (OSError, ValueError):
        for fd in [image, pidfd]:
            if fd is not None:
                os.close(fd)
        raise


def resident_bytes(pid):
    """Conservative RSS bound; Linux PSS is also retained as evidence."""
    if os.uname().sysname == "Linux":
        values = {}
        for line in Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines():
            key, *parts = line.split()
            if key in {"Rss:", "Pss:"}:
                values[key] = int(parts[0]) * 1024
        return values["Rss:"], values["Pss:"]
    if os.uname().sysname == "Darwin":
        result = subprocess.run(["/bin/ps", "-o", "rss=", "-p", str(pid)],
                                capture_output=True, timeout=1, check=True)
        return int(result.stdout.strip()) * 1024, None
    raise OSError("unsupported memory observer")


class Entry:
    """Pool-owned live process; never instantiated outside OneUsePool."""
    def __init__(self, pool, key, prefix, expected):
        self.pool, self.key, self.prefix, self.expected = pool, key, prefix, expected
        self.lock = threading.RLock()
        self.changed = threading.Condition(self.lock)
        self.stdout, self.stderr = bytearray(), bytearray()
        self.failure = None
        self.claimed = self.written = self.ready = self.closed = False
        self.observed = self.finalized = False
        self.prefix_output_bytes = 0
        self.deadline = time.monotonic() + pool.ttl
        self.peak_rss = self.peak_pss = 0
        self.pidfd = self.image_fd = None
        self.threads = []
        self.loader = None
        self.kernel_accounting = None
        self.kernel_accounting_error = None
        self.cleanup_errors = []
        self.pid = None
        if pool.experiment_kernel_accounting_owner:
            from owned_wait4 import OwnedWait4
            # Assignment precedes binding/startup: finally always owns this child.
            self.process = OwnedWait4(pool.argv, deferred=True, stdin=subprocess.PIPE,
                                      stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                      env=pool.environment, cwd=pool.cwd)
        else:
            self.process = subprocess.Popen(pool.argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                            stderr=subprocess.PIPE, env=pool.environment, cwd=pool.cwd)
        self.pid = self.process.pid
        try:
            self.pidfd, self.image_fd, self.image_stamp = capture_running_image(self.pid, pool.identity)
            if not self._image_ok():
                raise ValueError("actual running executable image mismatch")
            if pool.experiment_kernel_accounting_owner:
                self.process.start_reaper()
            if pool.rustup_loader_target is not None:
                from edit_loader import RustupLoader
                self.loader = RustupLoader(pool.environment, pool.rustup_loader_target, pool.z3)
        except (OSError, ValueError, RuntimeError):
            self.close(cancel=True)
            raise
        try:
            self.threads = [threading.Thread(target=self._write_prefix),
                            threading.Thread(target=self._drain, args=(self.process.stdout, self.stdout)),
                            threading.Thread(target=self._drain, args=(self.process.stderr, self.stderr)),
                            threading.Thread(target=self._monitor)]
            for thread in self.threads:
                thread.start()
        except (OSError, ValueError, RuntimeError):
            self.close(cancel=True)
            raise

    def _alive(self):
        with self.changed:
            if self.pidfd is None:
                return False
            observer = select.poll()
            observer.register(self.pidfd, select.POLLIN)
            return not observer.poll(0)

    def _open_running_image(self):
        # This kernel link names the actual executable inode, including an
        # unlinked image, rather than whatever now occupies its old pathname.
        return os.open(f"/proc/{self.pid}/exe", os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC)

    def _image_ok(self):
        if not self._alive() or self.image_fd is None:
            return False
        current = self._open_running_image()
        try:
            return (file_stamp(current) == self.image_stamp == file_stamp(self.image_fd) and
                    image_digest(current) == self.pool.identity and
                    image_digest(self.image_fd) == self.pool.identity and self._alive())
        finally:
            os.close(current)

    def _kill_owned(self):
        with self.changed:
            if self.pidfd is not None:
                try:
                    signal.pidfd_send_signal(self.pidfd, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            elif self.pool.experiment_kernel_accounting_owner:
                self.process.cancel_unstarted()
            elif self.process.poll() is None:
                # Only constructor failure before threads or a waiter exist.
                self.process.kill()

    def fail(self, message):
        with self.changed:
            self.failure = self.failure or message
            self.ready = False
            self.changed.notify_all()
        try:
            self._kill_owned()
        except (OSError, ValueError) as error:
            if not self.pool.experiment_kernel_accounting_owner:
                raise
            self.cleanup_errors.append("owned signal refusal: " + str(error))

    def _ready(self):
        if self.failure or self.claimed:
            return
        # Any newly observed bytes invalidate a previous readiness decision.
        # A partial additional response must never leave the old state latched.
        self.ready = False
        if self.pool.experiment_numeric_time_only:
            try:
                state = response_policy.prefix_state(self.expected, bytes(self.stdout))
            except ValueError as error:
                self.fail("unsupported experimental prefix output: " + str(error))
                return
            mismatch, equal = state == 'mismatch', state == 'equal'
        else:
            mismatch = not self.expected.startswith(self.stdout)
            equal = self.stdout == self.expected
        if self.stderr or mismatch:
            self.fail("unexpected prefix output")
        elif self.written and (self.observed or self.pool.experiment_kernel_accounting_owner) and equal:
            if self.loader is not None and self.prefix:
                try:
                    self.loader.observe(self.pid, self.pidfd)
                except (OSError, ValueError) as error:
                    self.fail('Rustup library binding: ' + str(error))
                    return
            self.prefix_output_bytes = len(self.stdout)
            self.ready = True
            self.changed.notify_all()

    def _write_prefix(self):
        try:
            written = self.process.stdin.write(self.prefix)
            if self.pool.experiment_kernel_accounting_owner and written != len(self.prefix):
                raise OSError("incomplete prefix write")
            self.process.stdin.flush()
            with self.changed:
                self.written = True
                self._ready()
        except (OSError, ValueError) as error:
            self.fail("prefix write: " + str(error))

    def _drain(self, stream, output):
        try:
            while block := os.read(stream.fileno(), 65536):
                with self.changed:
                    if len(output) + len(block) > MAX_OUTPUT:
                        if self.pool.experiment_kernel_accounting_owner:
                            output.extend(block[:max(0, MAX_OUTPUT - len(output))])
                        self.fail("output budget exceeded")
                        return
                    output.extend(block)
                    self._ready()
        except (OSError, ValueError) as error:
            if not self.closed:
                self.fail("output read: " + str(error))

    def _monitor(self):
        while not self.closed:
            with self.changed:
                if self.failure:
                    return
                try:
                    exited = self.process.poll() is not None
                except (OSError, ValueError) as error:
                    self.fail("owned status observation: " + str(error))
                    return
                if exited:
                    if not self.claimed:
                        self.fail("solver exited before claim")
                    return
                if time.monotonic() >= self.deadline:
                    self.fail("preparation/idle/run deadline exceeded")
                    return
            if self.pool.experiment_kernel_accounting_owner:
                # No /proc sampling or terminal-race classifier in this named
                # path. Complete lifetime peak is required after actual wait4.
                # Writes and drains already refresh readiness on every changed
                # buffer. Repeating it here rehashes idle mapped libraries under
                # the claim lock; claim and each exposed reply validate them
                # afresh. Status/deadline monitoring above remains continuous.
                time.sleep(0.01)
                continue
            try:
                rss, pss = resident_bytes(self.pid)
            except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
                if self.claimed and self.process.poll() is not None:
                    return
                self.fail("memory observation: " + str(error))
                return
            with self.changed:
                if rss > self.pool.bytes_per_slot:
                    self.fail("resident memory budget exceeded")
                    return
                self.peak_rss = max(self.peak_rss, rss)
                self.peak_pss = max(self.peak_pss, pss or 0)
                self.observed = True
                self._ready()
            time.sleep(0.01)

    def wait_ready(self, timeout):
        deadline = time.monotonic() + timeout
        with self.changed:
            while not self.ready and not self.failure and time.monotonic() < deadline:
                self.changed.wait(max(0, deadline - time.monotonic()))
            return self.ready and not self.failure

    def consume(self, whole_input):
        with self.changed:
            if (not self.ready or self.failure or self.claimed or self.closed or
                    not whole_input.startswith(self.prefix) or len(whole_input) > self.pool.max_input_bytes):
                return None
            if time.monotonic() >= self.deadline or self.process.poll() is not None:
                self.fail("entry expired before claim")
                return None
            try:
                if not self._image_ok():
                    self.fail("claim executable image mismatch")
                    return None
                if not self.pool.experiment_kernel_accounting_owner:
                    rss, pss = resident_bytes(self.pid)
                else:
                    rss = pss = None
            except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
                self.fail("claim image/memory observation: " + str(error))
                return None
            if (rss is not None and rss > self.pool.bytes_per_slot) or time.monotonic() >= self.deadline:
                self.fail("claim budget or deadline exceeded")
                return None
            if rss is not None:
                self.peak_rss = max(self.peak_rss, rss)
                self.peak_pss = max(self.peak_pss, pss or 0)
            self.claimed = True
            self.ready = False
            self.deadline = time.monotonic() + self.pool.run_timeout
        try:
            suffix = whole_input[len(self.prefix):]
            written = self.process.stdin.write(suffix)
            if self.pool.experiment_kernel_accounting_owner and written != len(suffix):
                raise OSError("incomplete suffix write")
            self.process.stdin.close()
            code = self.process.wait(timeout=self.pool.run_timeout + 2)
            for thread in self.threads:
                thread.join(timeout=2)
            with self.changed:
                if self.failure or any(thread.is_alive() for thread in self.threads):
                    return None
                if self.pool.experiment_kernel_accounting_owner:
                    from owned_wait4 import positive_accounting
                    self.kernel_accounting = self.process.accounting()
                    positive_accounting(self.process, self.pool.bytes_per_slot)
                result = dict(stdout=bytes(self.stdout), stderr=bytes(self.stderr), exit=code,
                            pid=self.pid, prefix_stdout=bytes(self.stdout[:self.prefix_output_bytes]),
                            suffix_stdout=bytes(self.stdout[self.prefix_output_bytes:]),
                            prefix_sha256=sha(self.prefix), key=self.key,
                            sampled_peak_rss=self.peak_rss, sampled_peak_pss=self.peak_pss)
                if self.pool.experiment_kernel_accounting_owner:
                    result.update(kernel_accounting=self.kernel_accounting,
                                  sampled_peak_rss=None, sampled_peak_pss=None)
                return result
        except (OSError, ValueError, subprocess.TimeoutExpired) as error:
            self.fail("consume: " + str(error))
            return None
        finally:
            self.close()
            self.finalized = True

    def _close_kernel(self, cancel):
        from owned_wait4 import drain_unstarted
        if getattr(self, 'kernel_closed', False):
            return
        errors = []
        with self.changed:
            if cancel and not self.finalized:
                self.failure = self.failure or "pool closed or entry reclaimed"
            self.closed = True
            self.ready = False
            self.changed.notify_all()
        try:
            self.process.cancel_unstarted()
            if self.process.poll() is None:
                self._kill_owned()
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            errors.append('owned cancellation: ' + str(error))
        try:
            self.process.wait(timeout=2)
            self.kernel_accounting = self.process.accounting()
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            self.kernel_accounting_error = str(error)
            errors.append('owned completion/accounting: ' + str(error))
            if self.pidfd is not None:
                try:
                    self.process.cleanup_failed_accounting(self.pidfd)
                except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as cleanup_error:
                    errors.append('held-pidfd error cleanup: ' + str(cleanup_error))
        for thread in self.threads:
            if thread is not threading.current_thread() and thread.ident is not None:
                thread.join(timeout=2)
                if thread.is_alive():
                    errors.append('owned I/O worker did not finish')
        for index, stream, output in [(1, self.process.stdout, self.stdout),
                                       (2, self.process.stderr, self.stderr)]:
            if index >= len(self.threads) or self.threads[index].ident is None:
                errors.extend(drain_unstarted(stream, output, MAX_OUTPUT))
        for stream in (self.process.stdin, self.process.stdout, self.process.stderr):
            try:
                stream.close()
            except (OSError, ValueError) as error:
                errors.append('stream close: ' + str(error))
        for thread in self.threads:
            if thread is not threading.current_thread() and thread.ident is not None:
                thread.join(timeout=2)
                if thread.is_alive():
                    errors.append('owned worker remained after stream close')
        if self.loader is not None:
            try:
                self.loader.close()
            except (OSError, ValueError) as error:
                errors.append('final Rustup library binding: ' + str(error))
        with self.changed:
            for name in ('image_fd', 'pidfd'):
                descriptor = getattr(self, name)
                if descriptor is not None:
                    try:
                        os.close(descriptor)
                    except OSError as error:
                        errors.append(name + ' close: ' + str(error))
                    setattr(self, name, None)
        self.cleanup_errors.extend(errors)
        self.kernel_closed = True
        if errors:
            self.failure = self.failure or '; '.join(errors)
            raise OSError('; '.join(errors))

    def close(self, cancel=False):
        if self.pool.experiment_kernel_accounting_owner:
            return self._close_kernel(cancel)
        if cancel and not self.finalized:
            self.fail("pool closed or entry reclaimed")
        with self.changed:
            self.closed = True
            self.ready = False
            self.changed.notify_all()
        if self.process.poll() is None:
            self._kill_owned()
        self.process.wait(timeout=2)
        for stream in [self.process.stdin, self.process.stdout, self.process.stderr]:
            try:
                stream.close()
            except (OSError, ValueError):
                pass
        for thread in self.threads:
            if thread is not threading.current_thread() and thread.ident is not None:
                thread.join(timeout=2)
        with self.changed:
            for name in ["image_fd", "pidfd"]:
                fd = getattr(self, name)
                if fd is not None:
                    os.close(fd)
                    setattr(self, name, None)


class OneUsePool:
    """One bounded owner of warming, ready and claimed solver processes.

    Count includes every process state. RSS slices conservatively reserve the
    configured total budget; sampling is an admission/safety threshold, not an
    OS hard memory limit. Returned buffers are separately capped at 16MiB per
    stream. This is a per-pool prototype, not a host-wide fairness policy.
    """
    def __init__(self, z3, expected_sha256, *, slots=1, max_bytes=128 * 1024 * 1024,
                 ttl=5, run_timeout=5, environment=None, cwd=None, max_input_bytes=MAX_INPUT,
                 experiment_numeric_time_only=False, experiment_kernel_accounting_owner=False,
                 experiment_rustup_loader_target=None):
        if (os.uname().sysname != "Linux" or not hasattr(os, "pidfd_open") or
                not hasattr(signal, "pidfd_send_signal")):
            raise ValueError("live reuse requires Linux held-executable and pidfd guards; use fresh fallback")
        if any(type(value) is not int for value in [slots, max_bytes, max_input_bytes]):
            raise ValueError("count and byte budgets require integer bounds")
        if not 1 <= slots <= 8 or max_bytes <= 0 or not 0 < ttl <= 3600 or not 0 < run_timeout <= 3600:
            raise ValueError("unsupported pool bounds")
        if not 1 <= max_input_bytes <= MAX_INPUT_LIMIT:
            raise ValueError("input budget exceeds the 16MiB prototype ceiling")
        if type(experiment_numeric_time_only) is not bool:
            raise ValueError("experimental policy requires explicit boolean")
        if type(experiment_kernel_accounting_owner) is not bool or (experiment_kernel_accounting_owner and not experiment_numeric_time_only):
            raise ValueError('kernel accounting requires the named response experiment')
        self.experiment_kernel_accounting_owner = experiment_kernel_accounting_owner
        self.experiment_numeric_time_only = experiment_numeric_time_only
        self.z3 = Path(z3).resolve()
        if path_digest(self.z3) != expected_sha256:
            raise ValueError("pinned solver digest mismatch")
        self.identity = expected_sha256
        self.environment = MappingProxyType(dict(os.environ if environment is None else environment))
        self.rustup_loader_target = None
        if experiment_rustup_loader_target is not None and not experiment_kernel_accounting_owner:
            raise ValueError('Rustup context requires named owned edit experiment')
        if any(name.startswith(("LD_", "DYLD_")) for name in self.environment):
            if experiment_rustup_loader_target is None:
                raise ValueError("loader environment is unsupported")
            from edit_loader import RustupLoader
            # Validate before allocating a child; retain the exact environment.
            RustupLoader(self.environment, experiment_rustup_loader_target, self.z3)
            self.rustup_loader_target = str(Path(experiment_rustup_loader_target).resolve(strict=True))
        if any(value != resource.RLIM_INFINITY for value in resource.getrlimit(resource.RLIMIT_CPU)):
            raise ValueError("finite CPU budget is unsupported")
        self.cwd = str(Path(cwd or os.getcwd()).resolve())
        self.argv = (str(self.z3), "-smt2", "-in")  # No global timeout or external channel flags.
        self.slots, self.bytes_per_slot = slots, max_bytes // slots
        self.max_input_bytes = max_input_bytes
        self.ttl, self.run_timeout = ttl, run_timeout
        self.entries = {}
        self.lock = threading.Lock()
        self.closed = False
        self.last_refusal = None
        self.last_attempt = None
        self.binding = dict(solver_sha256=self.identity, argv=self.argv, cwd=self.cwd,
                            environment_sha256=sha(json.dumps(sorted(self.environment.items()),
                                                             ensure_ascii=True).encode()))
        if self.rustup_loader_target is not None:
            self.binding['loader_experiment'] = 'edit-rustup-mapped-files-v1'
            self.binding['loader_target'] = self.rustup_loader_target
        if self.experiment_numeric_time_only:
            self.binding["response_experiment"] = response_policy.EXPERIMENT
        if self.experiment_kernel_accounting_owner:
            self.binding["resource_experiment"] = "owned-wait4-lifetime-peak-v1"

    def _identity_unchanged(self):
        try:
            return path_digest(self.z3) == self.identity
        except (OSError, ValueError):
            return False

    def seed(self, prefix, expected_stdout):
        """Start real prefix replay, or refuse before allocating another slot.

        A complete prefix and its expected responses must be supplied from a
        validated command/response boundary. Empty expected output establishes
        only submission, not that trailing declarations are already processed.
        Wall-limited query results cannot be promoted to cached proofs here.
        """
        if not isinstance(prefix, bytes) or not isinstance(expected_stdout, bytes):
            return None
        if len(prefix) > self.max_input_bytes or len(expected_stdout) > MAX_OUTPUT:
            return None
        if self.experiment_numeric_time_only:
            try:
                response_policy.project(expected_stdout)
            except ValueError as error:
                self.last_refusal = str(error)
                return None
        key = sha(json.dumps(self.binding, sort_keys=True).encode() +
                  len(prefix).to_bytes(8, "little") + prefix +
                  len(expected_stdout).to_bytes(8, "little") + expected_stdout)
        with self.lock:
            if self.closed:
                return None
            for old_key, entry in list(self.entries.items()):
                if entry.finalized or (entry.failure and not entry.claimed and entry.process.poll() is not None):
                    entry.close(cancel=True)
                    del self.entries[old_key]
            if len(self.entries) >= self.slots or key in self.entries:
                return None
            if not self._identity_unchanged():
                return None
            try:
                if self.experiment_kernel_accounting_owner:
                    # Retain even a partially initialized failed startup for raw
                    # stream/accounting/error evidence; never a reusable entry.
                    entry = Entry.__new__(Entry)
                    self.last_attempt = entry
                    entry.__init__(self, key, prefix, expected_stdout)
                else:
                    entry = Entry(self, key, prefix, expected_stdout)
            except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
                self.last_refusal = str(error)
                return None
            self.entries[key] = entry
            return key

    def wait_ready(self, key, timeout=5):
        entry = self.entries.get(key)
        return entry is not None and entry.wait_ready(timeout)

    def consume(self, key, whole_input):
        with self.lock:
            entry = self.entries.get(key)
            if self.closed or entry is None or not self._identity_unchanged():
                return None
        if not isinstance(whole_input, bytes):
            return None
        return entry.consume(whole_input)

    def close(self):
        with self.lock:
            self.closed = True
            entries = list(self.entries.values())
        for entry in entries:
            entry.close(cancel=True)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
