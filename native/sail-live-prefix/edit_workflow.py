"""Disabled edit-loop research driver, not a verifier or exhaustive spawn census.

One fixed donor session0 supplies the first check prefix. Preparation runs in
the background; readiness is snapshotted before the scheduled edited compiler
launch. All other sessions run fresh. Registered session accounting is joined
independently of compiler exit. The pre-first-wrapper-record/exec gap remains
PENDING: a successful controlled run never establishes application acceptance.
"""
import hashlib
import json
import os
from pathlib import Path
import secrets
import selectors
import signal
import subprocess
import threading
import time

from edit_claim import DirectClaim, StreamingClaim
from edit_stream import Commands, Replies, ProtocolRefusal, donor_prefix, MAX_BYTES
from edit_transport import Broker, canonical, create_record
from edit_wire import attempt_status
from live_prefix import OneUsePool
from owned_wait4 import OwnedWait4, drain_unstarted

HERE = Path(__file__).resolve().parent
MAX_RAW = 1024 ** 3


def compiler(argv, cwd, environment, timeout):
    """Own the compiler waiter/pidfd and bounded streams, including failures.

    Compiler wait4 may include its waited descendants. It is reported
    separately and MUST NOT be added to the solver/preparation CPU records.
    No claim of whole-process-tree resource coverage or image binding is made.
    """
    owner = None
    descriptor = None
    streams = dict(stdout=bytearray(), stderr=bytearray())
    errors = []
    accounting = None
    code = None
    start = time.monotonic()
    selector = selectors.DefaultSelector()
    try:
        owner = OwnedWait4(argv, deferred=True, cwd=cwd, env=environment,
                           stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE)
        # Before releasing the sole waiter this actual child is unreaped;
        # acquiring its pidfd cannot bind a recycled PID.
        descriptor = os.pidfd_open(owner.pid)
        owner.start_reaper()
        for name in streams:
            stream = getattr(owner, name)
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
        while selector.get_map():
            remaining = start + timeout - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(argv, timeout)
            for key, _ in selector.select(min(.05, remaining)):
                block = os.read(key.fileobj.fileno(), 65536)
                if not block:
                    selector.unregister(key.fileobj)
                    continue
                available = MAX_BYTES - len(streams[key.data])
                streams[key.data].extend(block[:available])
                if len(block) > available:
                    raise ProtocolRefusal('compiler output cap; raw prefix retained')
        code = owner.wait(timeout=max(0, start + timeout - time.monotonic()))
        accounting = owner.accounting()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        errors.append(type(error).__name__ + ': ' + str(error))
    finally:
        selector.close()
        if owner is not None:
            if errors and descriptor is not None:
                try:
                    signal.pidfd_send_signal(descriptor, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                except OSError as error:
                    errors.append('compiler held-pidfd signal: ' + str(error))
            try:
                owner.cancel_unstarted()
                code = owner.wait(timeout=2)
                accounting = owner.accounting()
            except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
                errors.append('compiler completion: ' + str(error))
                if descriptor is not None:
                    try:
                        owner.cleanup_failed_accounting(descriptor)
                    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as cleanup:
                        errors.append('compiler descriptor cleanup: ' + str(cleanup))
            for name, buffer in streams.items():
                errors.extend(drain_unstarted(getattr(owner, name), buffer, MAX_BYTES))
                getattr(owner, name).close()
        if descriptor is not None:
            os.close(descriptor)
    return dict(exit=code, errors=errors, wall_seconds=time.monotonic() - start,
                accounting=accounting, **{k: bytes(v) for k, v in streams.items()})


class Workflow:
    """One donor, one scheduled edit and at most one whole fresh retry."""
    def __init__(self, output, solver, solver_sha256, argv, cwd, *,
                 edit_delay=0, attempt_timeout=120):
        if (not 0 <= edit_delay <= 60 or not 0 < attempt_timeout <= 120 or
                not argv or not all(isinstance(x, str) for x in argv)):
            raise ProtocolRefusal('fixed bounded workflow required')
        self.output = Path(output).absolute()
        self.output.mkdir(mode=0o700, exist_ok=False)
        self.solver = str(Path(solver).resolve(strict=True))
        if hashlib.sha256(Path(self.solver).read_bytes()).hexdigest() != solver_sha256:
            raise ProtocolRefusal('actual solver bytes differ')
        self.sha256 = solver_sha256
        self.argv, self.cwd = list(argv), str(Path(cwd).resolve(strict=True))
        self.delay, self.timeout = edit_delay, attempt_timeout
        self.config = self.output / 'wrapper.json'
        self.environment = dict(os.environ, VERUS_Z3_PATH=str(HERE / 'edit-interposer.py'),
                                SAIL_LIVE_PREFIX_EDIT_CONFIG=str(self.config))
        self.copied = 0
        self.pools = []
        self.attempts = []
        self.claimed_preparation = False

    def pool(self, binding):
        if binding['argv'] != [self.solver, '-smt2', '-in']:
            raise ProtocolRefusal('actual solver argv differs')
        pool = OneUsePool(self.solver, self.sha256, slots=1, max_bytes=1024**3,
                          ttl=300, run_timeout=self.timeout,
                          environment=binding['environment'], cwd=binding['cwd'],
                          experiment_numeric_time_only=True,
                          experiment_kernel_accounting_owner=True)
        self.pools.append(pool)
        return pool

    def direct(self, binding):
        return DirectClaim(self.pool(binding))

    def raw(self, directory, name, data):
        if not isinstance(data, bytes) or len(data) > MAX_RAW - self.copied:
            raise ProtocolRefusal('whole workflow raw cap; no successful report')
        self.copied += len(data)  # Reserve before write; no refund on failure.
        path = directory / name
        with path.open('xb') as stream:
            stream.write(data)
        return dict(path=str(path.relative_to(self.output)), bytes=len(data),
                    sha256=hashlib.sha256(data).hexdigest())

    def attempt(self, name, selected=None, selected_binding=None):
        """Assign ordinal0 at accept, never by retrospectively inspecting input."""
        claims = []
        def factory(ordinal, binding):
            if ordinal == 0 and selected is not None and binding == selected_binding:
                claim, fallback = selected, lambda: self.direct(binding)
                self.claimed_preparation = True
            else:
                claim, fallback = self.direct(binding), None
            claims.append(claim)
            return claim, fallback
        token = secrets.token_hex(32)
        broker = Broker(self.output / name, name, token, factory,
                        time.time() + self.timeout + 5)
        config = dict(directory=str(broker.records), socket=str(broker.path),
                      attempt=name, token=token, solver=self.solver,
                      deadline_unix=broker.deadline)
        execution = dict(exit=None, errors=[], wall_seconds=0, accounting=None,
                         stdout=b'', stderr=b'')
        try:
            temporary = self.output / 'wrapper-new.json'
            create_record(temporary, config)
            os.replace(temporary, self.config)
            execution = compiler(self.argv, self.cwd, self.environment, self.timeout)
        except (OSError, ValueError, RuntimeError) as error:
            execution['errors'].append('attempt startup: ' + str(error))
        finally:
            outcome = broker.finish()
        gate = attempt_status(execution['exit'], outcome['sessions'])
        if execution['errors'] or not outcome['passed']:
            gate.update(driver_exit=1, broker_gate=False, whole_fresh_retry_required=True)
        record = dict(name=name, compiler=execution, broker=outcome, gate=gate)
        self.attempts.append(record)
        return record

    def persist(self):
        """Retain raw per-session and child streams; no private environment dump."""
        records = []
        for attempt in self.attempts:
            directory = self.output / attempt['name']
            execution = attempt['compiler']
            rendered = dict(name=attempt['name'], gate=attempt['gate'],
                            compiler={k: v for k, v in execution.items()
                                      if k not in ('stdout', 'stderr')},
                            sessions=[], errors=attempt['broker']['errors'])
            for stream in ('stdout', 'stderr'):
                rendered['compiler'][stream] = self.raw(directory, 'compiler.' + stream,
                                                       execution[stream])
            for row in attempt['broker']['sessions']:
                result = {k: v for k, v in row.items()
                          if k not in ('claims', 'input', 'output', '_binding')}
                prefix = 'session-' + str(row['ordinal'])
                for field in ('input', 'output'):
                    if field in row:
                        result[field] = self.raw(directory, prefix + '.' + field, bytes(row[field]))
                result['children'] = []
                for index, claim in enumerate(row.get('claims', [])):
                    entry = claim.entry
                    child = dict(errors=list(claim.errors), completed=claim.completed,
                                 ready_at_edit_start=claim.snapshot_ready)
                    if entry is not None:
                        child.update(pid=entry.process.pid, accounting=entry.kernel_accounting,
                                     cleanup_errors=entry.cleanup_errors)
                        for field in ('stdout', 'stderr'):
                            child[field] = self.raw(directory, prefix + '-' + str(index) + '.' + field,
                                                    bytes(getattr(entry, field)))
                    result['children'].append(child)
                rendered['sessions'].append(result)
            records.append(rendered)
        return records

    def pool_records(self):
        """Include unused/cancelled preparation and partial failed startups."""
        directory = self.output / 'owned-children'
        directory.mkdir(mode=0o700)
        records = []
        for index, pool in enumerate(self.pools):
            entries = list(pool.entries.values())
            if pool.last_attempt is not None and pool.last_attempt not in entries:
                entries.append(pool.last_attempt)
            for number, entry in enumerate(entries):
                accounting = getattr(entry, 'kernel_accounting', None)
                errors = list(getattr(entry, 'cleanup_errors', []))
                if accounting is None:
                    errors.append('complete child accounting unavailable')
                elif not 0 < accounting['accounting']['peak_rss_bytes'] <= 1024**3:
                    errors.append('positive complete child peak bound refused')
                record = dict(pool=index, entry=number, pid=getattr(entry, 'pid', None),
                              solver_sha256=pool.identity, context=pool.binding,
                              accounting=accounting, errors=errors,
                              failure=getattr(entry, 'failure', None),
                              accounting_error=getattr(entry, 'kernel_accounting_error', None))
                for field in ('stdout', 'stderr', 'prefix', 'expected'):
                    record[field] = self.raw(directory, f'{index}-{number}.{field}',
                                             bytes(getattr(entry, field, b'')))
                records.append(record)
        return records

    def run(self, source, expected_source_sha256, edited_bytes):
        """Apply an explicit edit, restore exact bytes, retain failed retry costs."""
        path = Path(source)
        original = path.read_bytes()
        if (path.is_symlink() or not path.is_file() or not isinstance(edited_bytes, bytes) or
                hashlib.sha256(original).hexdigest() != expected_source_sha256):
            raise ProtocolRefusal('exact ordinary source/edit binding refused')
        result = dict(errors=[], qualified=False, selected=False,
                      coverage='registered wrapper census only; pre-first-record/exec gap PENDING',
                      application_acceptance=False,
                      attempts=[], preparation=None, source_restored=False)
        thread, prepared, snapshot = None, {}, None
        try:
            self.raw(self.output, 'source.original', original)
            self.raw(self.output, 'source.edited', edited_bytes)
            donor = self.attempt('donor')
            if donor['gate']['driver_exit'] != 0:
                raise ProtocolRefusal('whole donor attempt refused')
            row = donor['broker']['sessions'][0]
            binding = row['_binding']
            parser = Commands()
            batches = parser.feed(row['input'])
            parser.finish()
            prefix = donor_prefix(batches)
            reply_parser = Replies()
            replies = reply_parser.feed(row['output'])
            reply_parser.finish()
            count = len(Commands().feed(prefix))
            if len(replies) != len(batches):
                raise ProtocolRefusal('donor literal batch/reply cardinality differs')
            pool = self.pool(binding)
            launch_at = time.monotonic() + self.delay
            def prepare():
                try:
                    prepared['key'] = pool.seed(prefix, b''.join(replies[:count]))
                    if prepared['key'] is None:
                        prepared['error'] = pool.last_refusal or 'prefix startup refused'
                except (OSError, ValueError, RuntimeError) as error:
                    prepared['error'] = type(error).__name__ + ': ' + str(error)
            thread = threading.Thread(target=prepare)
            thread.start()
            path.write_bytes(edited_bytes)
            time.sleep(max(0, launch_at - time.monotonic()))
            # No wait_ready or readiness lock wait. A still-running seed is
            # unready; the edited compiler receives a fresh session instead.
            key = prepared.get('key')
            if key is not None:
                snapshot = StreamingClaim(pool, key)
            result['preparation'] = dict(ready_at_edit_start=bool(snapshot and snapshot.snapshot_ready),
                                         scheduled_launch=launch_at, snapshot=time.monotonic())
            selected = snapshot if snapshot and snapshot.snapshot_ready else None
            edited = self.attempt('edited', selected, binding)
            if edited['gate']['driver_exit'] != 0:
                # Whole new compiler process, new broker and all new children.
                # The failed attempt's replies/output/status are never reused.
                self.attempt('fresh-retry')
            result['driver_exit'] = self.attempts[-1]['gate']['driver_exit']
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            result['errors'].append(type(error).__name__ + ': ' + str(error))
            result['driver_exit'] = 1
        finally:
            try:
                path.write_bytes(original)
                result['source_restored'] = path.read_bytes() == original
            except OSError as error:
                result['errors'].append('source restoration: ' + str(error))
            if thread is not None:
                thread.join(timeout=3)
                if thread.is_alive():
                    result['errors'].append('preparation startup thread incomplete')
            if snapshot is not None:
                snapshot.close()
            for pool in self.pools:
                # All prepared and unused/nonselected fresh children remain
                # owned, including context misses and startup failures.
                try:
                    pool.close()
                except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
                    result['errors'].append('complete pool cleanup: ' + str(error))
            try:
                result['attempts'] = self.persist()
                result['owned_children'] = self.pool_records()
                if any(row['errors'] for row in result['owned_children']):
                    result['errors'].append('complete accounting for every created child refused')
                if hashlib.sha256(Path(self.solver).read_bytes()).hexdigest() != self.sha256:
                    result['errors'].append('final solver bytes differ')
            except (OSError, ValueError, RuntimeError) as error:
                result['errors'].append('bounded evidence/final identity: ' + str(error))
            if prepared:
                result['preparation_startup'] = prepared
            result['raw_bytes'] = self.copied
            result['preparation_claimed'] = self.claimed_preparation
            # `driver_exit` describes the registered protocol experiment only.
            # A runnable entry point must not report application acceptance
            # until the independent spawn-coverage boundary is qualified.
            result['application_exit'] = 1
            if result['errors'] or not result['source_restored']:
                result['driver_exit'] = 1
            (self.output / 'result.json').write_bytes(canonical(result) + b'\n')
        return result
