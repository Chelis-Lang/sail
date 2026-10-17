"""Disabled edit-loop research driver, not a verifier or exhaustive spawn census.

One fixed donor session0 supplies the first check prefix. Preparation runs in
the background; readiness is snapshotted at the explicit edit-start default or
prospective first-session-claim boundary, before request reads/replies. All other
sessions run fresh. Registered session accounting is joined
independently of compiler exit. The pre-first-wrapper-record/exec gap remains
PENDING: a successful controlled run never establishes application acceptance.
"""
import hashlib
import json
import os
from pathlib import Path
import secrets
import resource
import selectors
import signal
import stat
import subprocess
import threading
import time

from edit_claim import DirectClaim, StreamingClaim, LiteralInputFile
from edit_census import census
from edit_stream import Commands, Replies, ProtocolRefusal, donor_prefix, MAX_BYTES
from edit_transport import Broker, canonical, create_record
from edit_wire import attempt_status
from live_prefix import OneUsePool
from owned_wait4 import OwnedWait4, drain_unstarted

HERE = Path(__file__).resolve().parent
MAX_RAW = 1024 ** 3
CONTEXT_ENV = 'SAIL_LIVE_PREFIX_SOLVER_CONTEXT'


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


def cpu_snapshot():
    return (resource.getrusage(resource.RUSAGE_SELF), resource.getrusage(resource.RUSAGE_CHILDREN))


def cpu_delta(before):
    after = cpu_snapshot()
    return {name: dict(user_seconds=end.ru_utime - start.ru_utime,
                      system_seconds=end.ru_stime - start.ru_stime)
            for name, start, end in zip(('driver_self', 'driver_waited_children'), before, after)}


class Workflow:
    """One donor, one scheduled edit and at most one whole fresh retry."""
    def __init__(self, output, solver, solver_sha256, argv, cwd, *,
                 edit_delay=0, attempt_timeout=120, rustup_loader_target=None,
                 readiness_boundary='edit-start', donor_context=None, donor_prefix_batches=None):
        if (not 0 <= edit_delay <= 60 or not 0 < attempt_timeout <= 480 or
                not argv or not all(isinstance(x, str) for x in argv) or
                readiness_boundary not in ('edit-start', 'first-session-claim')):
            raise ProtocolRefusal('fixed bounded workflow required')
        if (donor_context is None) != (donor_prefix_batches is None):
            raise ProtocolRefusal('frozen historical context and cut required together')
        if donor_context is not None and (not isinstance(donor_context, str)
                or not donor_context.startswith('v1:') or len(donor_context) > 4096
                or any(c in donor_context for c in '\n\r\0')
                or type(donor_prefix_batches) is not int or not 1 <= donor_prefix_batches <= 100000):
            raise ProtocolRefusal('bounded frozen historical context/cut refused')
        self.output = Path(output).absolute()
        self.output.mkdir(mode=0o700, exist_ok=False)
        self.input_directory = self.output / 'input-spool'
        self.input_directory.mkdir(mode=0o700)
        self.input_files = []
        self.solver = str(Path(solver).resolve(strict=True))
        if hashlib.sha256(Path(self.solver).read_bytes()).hexdigest() != solver_sha256:
            raise ProtocolRefusal('actual solver bytes differ')
        self.sha256 = solver_sha256
        self.argv, self.cwd = list(argv), str(Path(cwd).resolve(strict=True))
        self.delay, self.timeout = edit_delay, attempt_timeout
        self.readiness_boundary = readiness_boundary
        self.donor_context, self.donor_prefix_batches = donor_context, donor_prefix_batches
        self.config = self.output / 'wrapper.json'
        self.environment = dict(os.environ, VERUS_Z3_PATH=str(HERE / 'edit-interposer.py'),
                                SAIL_LIVE_PREFIX_EDIT_CONFIG=str(self.config))
        if donor_context is not None:
            # Only the source-bound compiler child Command may assign the tag.
            self.environment.pop(CONTEXT_ENV, None)
            self.environment['SAIL_LIVE_PREFIX_CONTEXT_RESEARCH'] = '1'
        self.copied = 0
        self.raw_lock = threading.Lock()
        self.pools = []
        self.attempts = []
        self.claimed_preparation = False
        self.trace_program = None
        self.rustup_loader_target = rustup_loader_target

    def pool(self, binding, role='fresh-session'):
        if binding['argv'] != [self.solver, '-smt2', '-in']:
            raise ProtocolRefusal('actual solver argv differs')
        pool = OneUsePool(self.solver, self.sha256, slots=1, max_bytes=1024**3,
                          ttl=300, run_timeout=min(self.timeout, 270),
                          max_input_bytes=MAX_BYTES,
                          environment=binding['environment'], cwd=binding['cwd'],
                          experiment_numeric_time_only=True,
                          experiment_kernel_accounting_owner=True,
                          experiment_rustup_loader_target=self.rustup_loader_target)
        self.pools.append(pool)
        pool.edit_workflow_input_directory = self.input_directory
        pool.edit_workflow_input_files = self.input_files
        pool.edit_workflow_prefix_batches = self.donor_prefix_batches
        pool.edit_workflow_role = role
        return pool

    def direct(self, binding, role='fresh-session'):
        return DirectClaim(self.pool(binding, role))

    def raw(self, directory, name, data):
        with self.raw_lock:
            if not isinstance(data, bytes) or len(data) > MAX_RAW - self.copied:
                raise ProtocolRefusal('whole workflow raw cap; no successful report')
            self.copied += len(data)  # Reserve before write; no refund on failure.
        path = directory / name
        with path.open('xb') as stream:
            if stream.write(data) != len(data):
                raise ProtocolRefusal('incomplete owned raw write')
        return dict(path=str(path.relative_to(self.output)), bytes=len(data),
                    sha256=hashlib.sha256(data).hexdigest())

    def retained(self, record):
        """Read an owned retained stream with its exact bytes/size/hash binding."""
        path = self.output / record['path']
        if not path.resolve().is_relative_to(self.output.resolve(strict=True)) or record['bytes'] > MAX_BYTES:
            raise ProtocolRefusal('retained stream path/size refused')
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd, 'rb') as stream:
            before = os.fstat(stream.fileno())
            if not stat.S_ISREG(before.st_mode) or before.st_size != record['bytes']:
                raise ProtocolRefusal('retained stream identity/size refused')
            data = stream.read(MAX_BYTES + 1)
            after = os.fstat(stream.fileno())
            if (len(data) != record['bytes'] or hashlib.sha256(data).hexdigest() != record['sha256']
                    or (before.st_dev, before.st_ino, before.st_size, before.st_ctime_ns, before.st_mtime_ns)
                    != (after.st_dev, after.st_ino, after.st_size, after.st_ctime_ns, after.st_mtime_ns)):
                raise ProtocolRefusal('retained stream changed')
        return data

    def raw_input_file(self, directory, name, source):
        """Retain literal owned child bytes with bounded copy and readback."""
        with self.raw_lock:
            if len(source) > MAX_RAW - self.copied:
                raise ProtocolRefusal('whole workflow raw cap; no successful report')
            self.copied += len(source)  # No refund on failed/partial writes.
        path = directory / name
        digest = hashlib.sha256()
        size = 0
        with path.open('xb') as stream:
            for chunk in source.chunks():
                if stream.write(chunk) != len(chunk):
                    raise ProtocolRefusal('incomplete owned child input capture')
                digest.update(chunk)
                size += len(chunk)
        if size != len(source):
            raise ProtocolRefusal('owned child input capture length differs')
        with path.open('rb') as stream:
            for chunk in source.chunks():
                if stream.read(len(chunk)) != chunk:
                    raise ProtocolRefusal('literal child input capture readback differs')
            if stream.read(1):
                raise ProtocolRefusal('child input capture extra bytes')
        return dict(path=str(path.relative_to(self.output)), bytes=size,
                    sha256=digest.hexdigest())

    def spill_session(self, directory, row):
        """Save closed session buffers before releasing their duplicate memory."""
        stored = row.setdefault('_stored_streams', {})
        prefix = 'session-' + str(row['ordinal'])
        for field in ('input', 'output'):
            if field in row and field not in stored:
                stored[field] = self.raw(directory, prefix + '.' + field, bytes(row[field]))
        claims = row.get('claims', [])
        # The current worker owns these references after serve()/close(). Never
        # clear an active stream or an entry owned by another/prepared session.
        for claim in claims:
            if not isinstance(claim, StreamingClaim) or not claim.closed or claim.pool not in self.pools:
                raise ProtocolRefusal('closed owned claim required before buffer release')
            entry = claim.entry
            if entry is not None and (any(t.is_alive() for t in entry.threads)
                    or entry.kernel_accounting is None
                    or not (entry is claim.pool.last_attempt or entry in claim.pool.entries.values())):
                raise ProtocolRefusal('owned child I/O/accounting must finish before buffer release')
        for index, claim in enumerate(claims):
            claim.retained_inputs = {}
            for field in ('input', 'child_input'):
                data = getattr(claim, field)
                equal = (data.matches(row['input']) if isinstance(data, LiteralInputFile)
                         else data == row.get('input')) if 'input' in stored else False
                if equal:
                    record = dict(stored['input'], alias='exact bytes equal to completed caller input')
                elif isinstance(data, LiteralInputFile):
                    record = self.raw_input_file(directory, prefix + '-' + str(index) + '.' + field, data)
                else:
                    record = self.raw(directory, prefix + '-' + str(index) + '.' + field, bytes(data))
                claim.retained_inputs[field] = record
        verified = set()
        records = list(stored.values()) + [r for c in claims for r in c.retained_inputs.values()]
        for record in records:
            identity = (record['path'], record['bytes'], record['sha256'])
            if identity not in verified:
                self.retained(record)
                verified.add(identity)
        # All saves succeeded. Preserve every stream via its bound record;
        # failed writes leave all still-owned buffers in memory for failure capture.
        for field in stored:
            row.pop(field, None)
        for claim in claims:
            claim.input.clear()
            claim.child_input.clear()

    def attempt(self, name, selected=None, selected_binding=None, *, selected_factory=None):
        """Assign ordinal0 at accept, never by retrospectively inspecting input."""
        started = time.monotonic()
        cpu_started = cpu_snapshot()
        claims = []
        selected_occurrences = []
        def factory(ordinal, binding):
            candidate = selected
            eligible = (ordinal == 0 if self.donor_context is None else
                        binding['environment'].get(CONTEXT_ENV) == self.donor_context)
            if eligible and selected_binding is not None:
                selected_occurrences.append(ordinal)
                if len(selected_occurrences) != 1:
                    raise ProtocolRefusal('duplicate historical solver context')
            if eligible and binding == selected_binding and selected_factory is not None:
                # Broker calls this after authenticating the header, before
                # receiving any request batch. Never inspect future input.
                candidate = selected_factory(ordinal)
            if eligible and candidate is not None and binding == selected_binding:
                claim, fallback = candidate, lambda: self.direct(binding, name + '-pre-reply-fallback')
                self.claimed_preparation = True
            else:
                claim, fallback = self.direct(binding, name), None
            claims.append(claim)
            return claim, fallback
        token = secrets.token_hex(32)
        broker = Broker(self.output / name, name, token, factory,
                        time.time() + self.timeout + 5,
                        session_completed=lambda row: self.spill_session(self.output / name, row))
        config = dict(directory=str(broker.records), socket=str(broker.path),
                      attempt=name, token=token, solver=self.solver,
                      deadline_unix=broker.deadline)
        execution = dict(exit=None, errors=[], wall_seconds=0, accounting=None,
                         stdout=b'', stderr=b'')
        try:
            temporary = self.output / 'wrapper-new.json'
            create_record(temporary, config)
            os.replace(temporary, self.config)
            argv = self.argv
            if self.trace_program is not None:
                traces = broker.root / 'traces'
                traces.mkdir(mode=0o700)
                argv = [self.trace_program, '-ff', '-q', '-ttt', '-xx', '-s', '65536',
                        '-e', 'trace=%process', '-o', str(traces / 'process'), '--', *argv]
            execution = compiler(argv, self.cwd, self.environment, self.timeout)
        except (OSError, ValueError, RuntimeError) as error:
            execution['errors'].append('attempt startup: ' + str(error))
        finally:
            outcome = broker.finish()
        gate = attempt_status(execution['exit'], outcome['sessions'])
        if self.donor_context is not None and selected_binding is not None and len(selected_occurrences) != 1:
            outcome['passed'] = False
            outcome['errors'].append('frozen historical solver context missing or ambiguous')
        trace = None
        if self.trace_program is not None:
            trace = self.trace(broker.root, outcome)
            if not trace['passed']:
                gate.update(driver_exit=1, broker_gate=False, whole_fresh_retry_required=True)
        if execution['errors'] or not outcome['passed']:
            gate.update(driver_exit=1, broker_gate=False, whole_fresh_retry_required=True)
        if any(b'strace:' in line for line in execution['stderr'].splitlines()):
            execution['errors'].append('tracing diagnostic; raw stderr retained')
            gate.update(driver_exit=1, broker_gate=False, whole_fresh_retry_required=True)
        record = dict(name=name, compiler=execution, broker=outcome, gate=gate, census=trace,
                      selected_context_occurrences=selected_occurrences,
                      whole_attempt_wall_seconds=time.monotonic() - started,
                      cpu_observation=cpu_delta(cpu_started))
        self.attempts.append(record)
        return record

    def trace(self, directory, outcome):
        """Read bounded ordinary process trace files before parsing/admitting."""
        traces, records, errors = {}, [], []
        try:
            paths = sorted((directory / 'traces').iterdir())
        except OSError as error:
            paths = []
            errors.append('process trace directory: ' + str(error))
        for path in paths:
            row = dict(name=path.name)
            records.append(row)
            try:
                import re
                match = re.fullmatch(r'process\.([1-9][0-9]*)', path.name)
                if match is None:
                    raise ProtocolRefusal('unknown process trace path')
                fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
                with os.fdopen(fd, 'rb') as stream:
                    before = os.fstat(stream.fileno())
                    row.update(mode=before.st_mode, size=before.st_size)
                    if not stat.S_ISREG(before.st_mode):
                        raise ProtocolRefusal('nonregular process trace')
                    available = min(MAX_BYTES, MAX_RAW - self.copied)
                    raw = stream.read(available + 1)
                    # Reserve the full observation before another independent
                    # trace, even if the following identity/size check refuses.
                    self.copied += min(available, len(raw))
                    row.update(bytes_observed=len(raw), sha256=hashlib.sha256(raw[:available]).hexdigest())
                    after = os.fstat(stream.fileno())
                    if (len(raw) > available or len(raw) != before.st_size or
                            (before.st_dev, before.st_ino, before.st_size, before.st_ctime_ns, before.st_mtime_ns) !=
                            (after.st_dev, after.st_ino, after.st_size, after.st_ctime_ns, after.st_mtime_ns)):
                        raise ProtocolRefusal('process trace cap/change; original file retained')
                traces[int(match[1])] = raw
            except (OSError, ValueError, RuntimeError) as error:
                row['error'] = type(error).__name__ + ': ' + str(error)
                errors.append(row['error'])
        result = census(traces, self.environment['VERUS_Z3_PATH'], outcome['sessions'],
                        outcome['starts'], outcome['finishes'])
        result['files'] = records
        result['errors'].extend(errors)
        result['passed'] = not result['errors']
        return result

    def persist(self):
        """Retain raw per-session and child streams; no private environment dump."""
        records = []
        self.persistence_errors = []
        def verify_retained(record):
            try:
                self.retained(record)
            except (OSError, ValueError, RuntimeError) as error:
                self.persistence_errors.append(dict(path=record['path'], error=str(error)))
        for attempt in self.attempts:
            directory = self.output / attempt['name']
            execution = attempt['compiler']
            rendered = dict(name=attempt['name'], gate=attempt['gate'], census=attempt['census'],
                            whole_attempt_wall_seconds=attempt['whole_attempt_wall_seconds'],
                            cpu_observation=attempt['cpu_observation'],
                            compiler={k: v for k, v in execution.items()
                                      if k not in ('stdout', 'stderr')},
                            sessions=[], errors=attempt['broker']['errors'])
            for stream in ('stdout', 'stderr'):
                rendered['compiler'][stream] = self.raw(directory, 'compiler.' + stream,
                                                       execution[stream])
            for row in attempt['broker']['sessions']:
                result = {k: v for k, v in row.items()
                          if k not in ('claims', 'input', 'output', '_binding', '_stored_streams')}
                prefix = 'session-' + str(row['ordinal'])
                for field in ('input', 'output'):
                    if field in row.get('_stored_streams', {}):
                        result[field] = row['_stored_streams'][field]
                        verify_retained(result[field])
                    elif field in row:
                        suffix = '.unspilled-' if '_stored_streams' in row else '.'
                        result[field] = self.raw(directory, prefix + suffix + field, bytes(row[field]))
                result['children'] = []
                for index, claim in enumerate(row.get('claims', [])):
                    entry = claim.entry
                    child = dict(errors=list(claim.errors), completed=claim.completed,
                                 ready_at_snapshot=claim.snapshot_ready,
                                 readiness_boundary=self.readiness_boundary,
                                 readiness_snapshot_time=claim.snapshot_time)
                    child['inputs'] = dict(getattr(claim, 'retained_inputs', {}))
                    for field in ('input', 'child_input'):
                        if field not in child['inputs']:
                            data = getattr(claim, field)
                            name = prefix + '-' + str(index) + '.unspilled-' + field
                            child['inputs'][field] = (self.raw_input_file(directory, name, data)
                                if isinstance(data, LiteralInputFile)
                                else self.raw(directory, name, bytes(data)))
                    for retained in child['inputs'].values():
                        verify_retained(retained)
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
                              role=pool.edit_workflow_role,
                              solver_sha256=pool.identity, context=pool.binding,
                              accounting=accounting, errors=errors,
                              failure=getattr(entry, 'failure', None),
                              accounting_error=getattr(entry, 'kernel_accounting_error', None),
                              loader_context=(entry.loader.record if getattr(entry, 'loader', None) is not None else None))
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
            rows = donor['broker']['sessions']
            if self.donor_context is None:
                row = rows[0]
            else:
                matching = [r for r in rows if r['_binding']['environment'].get(CONTEXT_ENV) == self.donor_context]
                if len(matching) != 1:
                    raise ProtocolRefusal('unique frozen historical donor context required')
                row = matching[0]
            result['donor_selection'] = dict(context=self.donor_context,
                ordinal=row['ordinal'], frozen_prefix_batches=self.donor_prefix_batches)
            binding = row['_binding']
            parser = Commands()
            donor_input = self.retained(row['_stored_streams']['input'])
            batches = parser.feed(donor_input)
            parser.finish()
            prefix = donor_prefix(batches, frozen_batch_count=self.donor_prefix_batches)
            reply_parser = Replies()
            replies = reply_parser.feed(self.retained(row['_stored_streams']['output']))
            reply_parser.finish()
            count = len(Commands().feed(prefix))
            if len(replies) != len(batches):
                raise ProtocolRefusal('donor literal batch/reply cardinality differs')
            pool = self.pool(binding, 'background-preparation')
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
            edited_started = time.monotonic()
            edited_cpu_started = cpu_snapshot()
            result['source_edit_monotonic'] = edited_started
            path.write_bytes(edited_bytes)
            time.sleep(max(0, launch_at - time.monotonic()))
            # Neither boundary waits for readiness or examines future input.
            # The prospective claim boundary overlaps only real compiler work;
            # ordinal0 is assigned at owned accept before its request is read.
            if self.readiness_boundary == 'edit-start':
                key = prepared.get('key')
                if key is not None:
                    snapshot = StreamingClaim(pool, key)
                result['preparation'] = dict(boundary='edit-start',
                    ready_at_edit_start=bool(snapshot and snapshot.snapshot_ready),
                    scheduled_launch=launch_at, snapshot=time.monotonic())
                selected = snapshot if snapshot and snapshot.snapshot_ready else None
                edited = self.attempt('edited', selected, binding)
            else:
                result['preparation'] = dict(boundary='first-session-claim',
                    scheduled_launch=launch_at, edited_attempt_start=time.monotonic(),
                    ready_at_session_claim=None, claim_observations=[])
                def at_first_claim(ordinal):
                    nonlocal snapshot
                    observed = dict(snapshot=time.monotonic(), ordinal=ordinal)
                    result['preparation']['claim_observations'].append(observed)
                    key = prepared.get('key')
                    if key is not None:
                        snapshot = StreamingClaim(pool, key)
                        observed.update(snapshot=snapshot.snapshot_time,
                            ready=snapshot.snapshot_ready, errors=list(snapshot.errors))
                    else:
                        observed.update(ready=False, errors=['preparation key unavailable at claim'])
                    result['preparation']['ready_at_session_claim'] = observed['ready']
                    return snapshot if snapshot and snapshot.snapshot_ready else None
                edited = self.attempt('edited', selected_binding=binding,
                                      selected_factory=at_first_claim)
            if edited['compiler']['errors']:
                # A timed-out/failed compiler owner cannot certify descendant
                # completion. The native caller must remove its owned Docker
                # container; do not start another compiler in that namespace.
                result['external_descendant_cleanup_required'] = True
                raise ProtocolRefusal('compiler I/O/owner incomplete; owned container cleanup required')
            # Cancel unused background work after the real edited attempt,
            # before either a retry or the independent fresh timing control.
            # This completion wait cannot promote edit-start readiness.
            thread.join(timeout=3)
            if thread.is_alive():
                raise ProtocolRefusal('preparation startup incomplete after edited attempt')
            pool.close()
            if edited['gate']['driver_exit'] != 0:
                # Whole new compiler process, new broker and all new children.
                # The failed attempt's replies/output/status are never reused.
                self.attempt('fresh-retry')
            final = self.attempts[-1]
            if final['compiler']['errors']:
                result['external_descendant_cleanup_required'] = True
                raise ProtocolRefusal('fresh retry compiler incomplete; owned container cleanup required')
            result['edited_to_result_wall_seconds'] = time.monotonic() - edited_started
            result['edited_to_result_cpu_observation'] = cpu_delta(edited_cpu_started)
            # Always execute a separate full fresh edited control, even after
            # a successful continuation. Never choose whether to measure it
            # from the observed gain or prefix match.
            control = self.attempt('fresh-control')
            same_output = all(final['compiler'][name] == control['compiler'][name]
                              for name in ('stdout', 'stderr'))
            result['literal_compiler_streams_equal'] = same_output
            result['driver_exit'] = (0 if final['gate']['driver_exit'] == 0 and
                                     control['gate']['driver_exit'] == 0 and same_output else 1)
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
                if self.persistence_errors:
                    result['errors'].append('retained stream verification: ' + repr(self.persistence_errors))
                result['owned_children'] = self.pool_records()
                if any(row['errors'] for row in result['owned_children']):
                    result['errors'].append('complete accounting for every created child refused')
                if hashlib.sha256(Path(self.solver).read_bytes()).hexdigest() != self.sha256:
                    result['errors'].append('final solver bytes differ')
            except (OSError, ValueError, RuntimeError) as error:
                result['errors'].append('bounded evidence/final identity: ' + str(error))
            for stored_input in self.input_files:
                try:
                    stored_input.close()
                except (OSError, ValueError) as error:
                    result['errors'].append('owned child input file cleanup: ' + str(error))
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
