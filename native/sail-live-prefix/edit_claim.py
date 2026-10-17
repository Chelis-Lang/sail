"""Source-only streaming adapter for the existing named, owned solver entry.

No default entry or resource classifier changes. The external research driver
must own the whole compiler attempt: an abort after a reply cannot be repaired
by swapping children. The disabled interposer supplies no application acceptance.
"""
import os
from contextlib import ExitStack
import select
import subprocess
import time

from edit_stream import Commands, Replies, PrefixReplay, ProtocolRefusal, MAX_BYTES
from owned_wait4 import positive_accounting


class StreamingClaim:
    """Freeze readiness once, then consume complete literal caller batches.

    The pool's existing deferred wait4 owner, pidfd/image guards, bounded raw
    drains and cancellation remain the actual process authority. The adapter
    never waits for an entry to become ready and never accepts another child.
    """

    def _initialize(self, pool):
        self.pool = pool
        self.entry = None
        self.replay = None
        self.input = bytearray()
        self.child_input = bytearray()
        self.reply_parser = Replies()
        self.input_parser = Commands()
        self.output_offset = 0
        self.errors = []
        self.snapshot_time = time.monotonic()
        self.snapshot_ready = False
        self.completed = False
        self.closed = False

    @staticmethod
    def _policy(pool):
        if not pool.experiment_kernel_accounting_owner or not pool.experiment_numeric_time_only:
            raise ProtocolRefusal('named response and kernel-owner policies required')
        if (pool.slots != 1 or pool.bytes_per_slot != 1024**3 or
                pool.ttl > 600 or pool.run_timeout > 300):
            raise ProtocolRefusal('fixed one-child resource/wall bounds required')

    def __init__(self, pool, key):
        self._initialize(pool)
        try:
            self._policy(pool)
            with ExitStack() as held:
                if not pool.lock.acquire(blocking=False):
                    raise ProtocolRefusal('pool-busy-at-edit-start')
                held.callback(pool.lock.release)
                entry = pool.entries.get(key)
                if pool.closed or entry is None:
                    raise ProtocolRefusal('missing or unbound preparation')
                if not entry.changed.acquire(blocking=False):
                    raise ProtocolRefusal('entry-busy-at-edit-start')
                held.callback(entry.changed.release)
                self.snapshot_time = time.monotonic()
                # A second caller must not cancel the first caller's owned
                # child. Assign cleanup only for this unclaimed entry.
                if entry.claimed or entry.closed:
                    raise ProtocolRefusal('preparation already consumed')
                self.entry = entry
                if (not entry.ready or entry.failure or
                        time.monotonic() >= entry.deadline or entry.process.poll() is not None):
                    raise ProtocolRefusal('not-ready-at-edit-start')
                if not pool._identity_unchanged() or not entry._image_ok():
                    raise ProtocolRefusal('edit-start executable identity refused')
                if entry.loader is not None:
                    entry.loader.observe(entry.pid, entry.pidfd)
                parser = Commands()
                commands = parser.feed(entry.prefix)
                if parser.finish():
                    raise ProtocolRefusal('unmatched prepared prefix trivia')
                actual = bytes(entry.stdout)
                replies = self.reply_parser.feed(actual)
                self.reply_parser.finish()
                self.replay = PrefixReplay([x['raw'] for x in commands], replies,
                                           ready_at_edit_start=True)
                entry.claimed = True
                entry.ready = False
                entry.deadline = time.monotonic() + pool.run_timeout
                self.snapshot_ready = True
                self.output_offset = len(actual)
                self.child_input.extend(entry.prefix)
                # Prefix submission completed before readiness. Only this
                # adapter writes suffix input, bypassing an empty buffer.
                os.set_blocking(entry.process.stdin.fileno(), False)
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            self.errors.append(str(error))
            if self.entry is not None:
                # Signal now without waiting for readiness/reaping. Completion
                # and full failed-preparation cost are retained by close().
                self._fail_entry('streaming edit-start refusal: ' + str(error))

    def _fail_entry(self, reason):
        try:
            self.entry.fail(reason)
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            self.errors.append('owned signal: ' + str(error))

    def _check(self, *, allow_terminal=False):
        entry = self.entry
        with entry.changed:
            if entry.failure or entry.stderr or time.monotonic() >= entry.deadline:
                raise ProtocolRefusal('child failure/stderr/deadline')
            if not allow_terminal and entry.process.poll() is not None:
                raise ProtocolRefusal('child exited before caller EOF')

    def _write(self, data):
        if len(data) > MAX_BYTES - len(self.child_input):
            raise ProtocolRefusal('complete child input budget')
        self.child_input.extend(data)
        offset = 0
        descriptor = self.entry.process.stdin.fileno()
        while offset < len(data):
            self._check()
            _, writable, _ = select.select([], [descriptor], [], .01)
            if writable:
                try:
                    count = os.write(descriptor, data[offset:offset + 65536])
                except BlockingIOError:
                    continue
                if count <= 0:
                    raise ProtocolRefusal('suffix write made no progress')
                offset += count

    def _reply(self):
        while True:
            self._check()
            with self.entry.changed:
                data = bytes(self.entry.stdout[self.output_offset:])
                self.output_offset += len(data)
                replies = self.reply_parser.feed(data)
                if replies:
                    if len(replies) != 1:
                        raise ProtocolRefusal('unsolicited extra reply batch')
                    self.reply_parser.finish()
                    if self.entry.loader is not None:
                        self.entry.loader.observe(self.entry.pid, self.entry.pidfd)
                    return replies[0]
                if self.entry.process.poll() is not None:
                    raise ProtocolRefusal('child exited before complete reply batch')
                self.entry.changed.wait(.01)

    def batch(self, raw):
        """Retain input before routing; return raw bytes or an explicit abort."""
        try:
            if self.closed or self.completed:
                raise ProtocolRefusal('attempt already completed')
            if not isinstance(raw, bytes) or len(raw) > MAX_BYTES - len(self.input):
                raise ProtocolRefusal('caller input budget')
            self.input.extend(raw)
            batches = self.input_parser.feed(raw)
            if len(batches) != 1 or self.input_parser.finish():
                raise ProtocolRefusal('complete single caller batch required')
            if not self.snapshot_ready:
                return {'action': 'fresh-before-reply', 'reason': self.errors[0]}
            self._check()
            # Observe before consume can mark a prepared reply as emitted.
            # A refusal on the first reply must still permit fresh-before-reply.
            if (self.entry.loader is not None and
                    self.replay.index < len(self.replay.commands)):
                self.entry.loader.observe(self.entry.pid, self.entry.pidfd)
            decision = self.replay.consume(raw)
            if decision['action'] == 'continue-original-child':
                self._write(raw)
                reply = self._reply()
                self.replay.emitted = True
                return {'action': 'original-child-reply', 'reply': reply}
            if decision['action'] != 'prepared-reply':
                self._fail_entry(decision['reason'])
                self.errors.append(decision['reason'])
            return decision
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            self.errors.append(str(error))
            if self.entry is not None:
                self._fail_entry('streaming batch refusal: ' + str(error))
            emitted = self.replay is not None and self.replay.emitted
            return {'action': 'abort-whole-attempt' if emitted else 'fresh-before-reply',
                    'reason': str(error)}

    def finish(self, trailing=b''):
        """Complete one original child, then require actual zero exit/accounting."""
        try:
            if self.errors or not self.snapshot_ready or self.completed:
                raise ProtocolRefusal('refused or duplicate completion')
            if self.replay.index != len(self.replay.commands):
                raise ProtocolRefusal('caller ended before complete prepared history')
            parser = Commands()
            if parser.feed(trailing) or parser.finish() != trailing:
                raise ProtocolRefusal('only exact trailing trivia allowed at EOF')
            if not isinstance(trailing, bytes) or len(trailing) > MAX_BYTES - len(self.input):
                raise ProtocolRefusal('caller input budget')
            self.input.extend(trailing)
            self._write(trailing)
            if self.input != self.child_input:
                raise ProtocolRefusal('actual child and caller input differ')
            self.entry.process.stdin.close()
            self.entry.process.wait(timeout=max(0, self.entry.deadline - time.monotonic()))
            for thread in self.entry.threads:
                thread.join(timeout=2)
            with self.entry.changed:
                self._check(allow_terminal=True)
                if any(t.is_alive() for t in self.entry.threads):
                    raise ProtocolRefusal('incomplete owned I/O')
                if self.output_offset != len(self.entry.stdout):
                    raise ProtocolRefusal('unmatched final child output')
                positive_accounting(self.entry.process, self.pool.bytes_per_slot)
                if not self.pool._identity_unchanged():
                    raise ProtocolRefusal('final executable path identity changed')
                self.entry.finalized = True
                self.completed = True
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            self.errors.append(str(error))
        finally:
            self.close()
        return self.completed and not self.errors

    def abort(self, reason):
        """Record a caller/IPC failure; emitted replies require a whole abort."""
        self.errors.append(reason)
        if self.entry is not None:
            self._fail_entry(reason)
        if self.replay is not None:
            return self.replay.fail(reason)
        return {'action': 'fresh-before-reply', 'reason': reason}

    def close(self):
        """Retain failures and actual accounting; never manufacture zero cost."""
        if self.closed:
            return
        self.closed = True
        if self.entry is not None:
            try:
                self.entry.close(cancel=not self.completed)
            except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
                self.errors.append('owned cleanup: ' + str(error))


class _DirectRoute:
    """No stored response: every batch runs on this fresh original child."""
    commands = ()
    index = 0
    emitted = False

    def consume(self, batch):
        return {'action': 'continue-original-child', 'input': batch}

    def fail(self, reason):
        return {'action': 'abort-whole-attempt' if self.emitted else 'fresh-before-reply',
                'reason': reason}


class DirectClaim(StreamingClaim):
    """Fresh interactive child for donor capture or a pre-reply miss.

    This waits only for the newly launched child's startup/binding, never for
    speculative preparation. No command or cached reply exists at startup.
    The same owned wait4 and complete final resource admission remain required.
    """

    def __init__(self, pool):
        self._initialize(pool)
        self.direct_startup = True
        try:
            self._policy(pool)
            key = pool.seed(b'', b'')
            if key is None:
                raise ProtocolRefusal('fresh startup refused: ' + str(pool.last_refusal))
            self.entry = pool.entries[key]
            if not pool.wait_ready(key, min(2, pool.run_timeout)):
                raise ProtocolRefusal('fresh child startup incomplete/refused')
            with pool.lock, self.entry.changed:
                entry = self.entry
                if (entry.claimed or entry.closed or entry.failure or not entry.ready or
                        entry.stdout or entry.stderr or not pool._identity_unchanged() or
                        not entry._image_ok()):
                    raise ProtocolRefusal('fresh original child binding refused')
                self.replay = _DirectRoute()
                entry.claimed = True
                entry.ready = False
                entry.deadline = time.monotonic() + pool.run_timeout
                self.snapshot_ready = True
                os.set_blocking(entry.process.stdin.fileno(), False)
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            self.errors.append(str(error))
            if self.entry is not None:
                self._fail_entry('fresh streaming refusal: ' + str(error))
