"""Private research connection registry; not an exhaustive exec/spawn census.

The outer driver still owns the compiler and must join every start/finish record
and broker completion. Failure before a wrapper's first record is an unresolved
coverage boundary until actual-Verus/source-bound qualification. Never claim a
connection count proves whole-pipeline coverage or a verification verdict.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import socket
import stat
import struct
import threading
import time

from edit_stream import ProtocolRefusal
from edit_wire import interpose, receive, serve

MAX_SESSIONS = 32
MAX_ACTIVE = 16


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def create_record(path, value):
    """Create one ordinary private record, never replace a prior session fact."""
    raw = canonical(value) + b'\n'
    if len(raw) > 65536:
        raise ProtocolRefusal('private record budget')
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(raw)


def read_record(path):
    fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        if (not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid() or
                stat.S_IMODE(before.st_mode) != 0o600 or before.st_size > 65536):
            raise ProtocolRefusal('private ordinary record owner/mode/size')
        raw = stream.read(65537)
        after = os.fstat(stream.fileno())
        if ((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) !=
                (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns) or
                len(raw) != before.st_size):
            raise ProtocolRefusal('private record changed')
    return json.loads(raw)


def peer(connection):
    """Linux native identity; local portable controls explicitly double this API."""
    if not hasattr(socket, 'SO_PEERCRED'):
        raise ProtocolRefusal('native Linux peer credentials unavailable')
    pid, uid, gid = struct.unpack('3i', connection.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
    if pid <= 0 or uid != os.getuid():
        raise ProtocolRefusal('foreign private socket peer')
    return dict(pid=pid, uid=uid, gid=gid)


def context(solver, arguments):
    """Actual caller context, including loader/research variables unchanged."""
    if list(arguments) != ['-smt2', '-in']:
        raise ProtocolRefusal('only audited Verus Z3 invocation supported')
    return {'argv': [str(solver), *arguments], 'cwd': os.getcwd(),
            'environment': dict(os.environ)}


def wrapper(config_path, arguments, source, sink):
    """One explicit VERUS_Z3_PATH interposer; never identify it as pinned Z3."""
    config = read_record(config_path)
    directory = Path(config['directory'])
    session = secrets.token_hex(16)
    identity = dict(attempt=config['attempt'], session=session, pid=os.getpid())
    result = dict(identity=identity, exit=1, error=None, context_sha256=None)
    connection = None
    started = False
    try:
        binding = context(config['solver'], arguments)
        result['context_sha256'] = digest(binding)
        create_record(directory / (session + '.start.json'),
                      dict(identity=identity, context_sha256=result['context_sha256']))
        started = True
        connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        connection.settimeout(config['deadline_unix'] - time.time())
        connection.connect(config['socket'])
        result['exit'] = interpose(connection, binding, config['token'], source, sink,
                                   identity=identity)
        connection = None
    except (OSError, ValueError, RuntimeError) as error:
        result['error'] = type(error).__name__ + ': ' + str(error)
    finally:
        if connection is not None:
            connection.close()
        if started:
            create_record(directory / (session + '.finish.json'), result)
    return result['exit']


class Broker:
    """Assign ordinals at owned accept, before inspecting any solver input.

    claim_factory(ordinal, actual_context) must return an actual owned claim and
    optional fresh factory. It cannot inspect future query input. The enclosing
    driver snapshots the selected preparation BEFORE compiler launch, not here.
    """
    def __init__(self, root, attempt, token, claim_factory, deadline_unix):
        if (not re.fullmatch('[a-z0-9-]{1,64}', attempt) or len(token) < 32 or
                not 0 < deadline_unix - time.time() <= 300):
            raise ProtocolRefusal('bounded private attempt required')
        self.root = Path(root)
        self.root.mkdir(mode=0o700, exist_ok=False)
        self.records = self.root / 'records'
        self.records.mkdir(mode=0o700)
        self.path = self.root / 'broker.sock'
        self.attempt, self.token = attempt, token
        self.factory, self.deadline = claim_factory, deadline_unix
        self.rows, self.workers, self.connections = [], [], []
        self.lock = threading.Lock()
        self.stopping = threading.Event()
        self.listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.listener.bind(str(self.path))
        self.path.chmod(0o600)
        self.socket_stamp = self.path.lstat()
        self.listener.listen(MAX_ACTIVE)
        self.listener.settimeout(.05)
        self.thread = threading.Thread(target=self._listen)
        self.thread.start()

    def _listen(self):
        while not self.stopping.is_set() and time.time() < self.deadline:
            try:
                connection, _ = self.listener.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            with self.lock:
                ordinal = len(self.rows)
                row = dict(ordinal=ordinal, identity=None, passed=False, complete=False, error=None)
                self.rows.append(row)
                if ordinal >= MAX_SESSIONS or sum(t.is_alive() for t in self.workers) >= MAX_ACTIVE:
                    row['error'] = 'session/concurrency budget refused'
                    connection.close()
                    self.stopping.set()
                    continue
                self.connections.append(connection)
                worker = threading.Thread(target=self._session, args=(connection, row))
                self.workers.append(worker)
                worker.start()

    def _session(self, connection, row):
        owned = None
        try:
            connection.settimeout(max(.001, self.deadline - time.time()))
            actual_peer = peer(connection)
            row['peer'] = actual_peer
            kind, raw = receive(connection)
            row['header_sha256'] = hashlib.sha256(raw).hexdigest()
            if kind != ord('H') or len(raw) > 65536:
                raise ProtocolRefusal('bounded initial header required')
            header = json.loads(raw)
            if set(header) != {'binding', 'token', 'identity'} or header['token'] != self.token:
                raise ProtocolRefusal('attempt authentication refused')
            identity = header['identity']
            if (set(identity) != {'attempt', 'session', 'pid'} or
                    identity['attempt'] != self.attempt or identity['pid'] != actual_peer['pid'] or
                    not re.fullmatch('[0-9a-f]{32}', identity['session'])):
                raise ProtocolRefusal('actual peer/attempt/session refused')
            row['identity'] = identity
            binding = header['binding']
            if (set(binding) != {'argv', 'cwd', 'environment'} or
                    not isinstance(binding['environment'], dict)):
                raise ProtocolRefusal('complete actual context required')
            start = read_record(self.records / (identity['session'] + '.start.json'))
            if start != dict(identity=identity, context_sha256=digest(binding)):
                raise ProtocolRefusal('actual start record differs')
            row['context_sha256'] = digest(binding)
            row['environment_names'] = sorted(binding['environment'])
            # Actual context is needed by the enclosing donor/preparation
            # owner. It stays in memory; evidence exposes only its digest.
            row['_binding'] = binding
            owned, fallback = self.factory(row['ordinal'], binding)
            outcome = serve(connection, owned, binding, self.token, fresh_factory=fallback,
                            identity=identity, initial_header=raw)
            # Keep actual claims/raw in memory for the enclosing evidence writer;
            # do not serialize private environment/token header values.
            row.update(outcome)
            row['identity'] = identity
        except (OSError, ValueError, RuntimeError, KeyError, TypeError) as error:
            row['error'] = type(error).__name__ + ': ' + str(error)
            row['passed'] = False
            if owned is not None:
                owned.abort(row['error'])
                owned.close()
        finally:
            connection.close()

    def finish(self):
        """Stop accepts, join all known starts/finishes/brokers, never verify."""
        self.stopping.set()
        self.thread.join(timeout=1)
        self.listener.close()
        for worker in self.workers:
            worker.join(timeout=max(0, self.deadline - time.time()))
        alive = [worker for worker in self.workers if worker.is_alive()]
        if alive:
            for connection in self.connections:
                try:
                    connection.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
                connection.close()
            for worker in alive:
                worker.join(timeout=3)
        errors = []
        socket_removed = False
        try:
            current = self.path.lstat()
            if (not stat.S_ISSOCK(current.st_mode) or current.st_uid != os.getuid() or
                    (current.st_dev, current.st_ino) !=
                    (self.socket_stamp.st_dev, self.socket_stamp.st_ino)):
                raise ProtocolRefusal('owned broker socket path changed')
            self.path.unlink()
            socket_removed = True
        except (OSError, ValueError) as error:
            errors.append('owned socket cleanup: ' + str(error))
        if self.thread.is_alive() or any(worker.is_alive() for worker in self.workers):
            errors.append('owned broker threads incomplete')
        try:
            starts, finishes = {}, {}
            for path in sorted(self.records.iterdir()):
                match = re.fullmatch(r'([0-9a-f]{32})\.(start|finish)\.json', path.name)
                if match is None:
                    raise ProtocolRefusal('unknown private session record')
                (starts if match[2] == 'start' else finishes)[match[1]] = read_record(path)
            sessions = [row['identity']['session'] for row in self.rows if row.get('identity')]
            if (not sessions or len(sessions) != len(set(sessions)) or
                    set(starts) != set(finishes) or set(starts) != set(sessions) or
                    len(sessions) != len(self.rows)):
                raise ProtocolRefusal('complete start/finish/broker join refused')
            for row in self.rows:
                identity = row['identity']
                end = finishes[identity['session']]
                if (end['identity'] != identity or end['exit'] != 0 or end['error'] or
                        end['context_sha256'] != row['context_sha256'] or
                        not row['passed'] or not row['complete'] or row['error']):
                    raise ProtocolRefusal('independent session completion refused')
        except (OSError, ValueError, RuntimeError, KeyError, TypeError) as error:
            errors.append(type(error).__name__ + ': ' + str(error))
        return dict(passed=not errors, complete=not errors, errors=errors,
                    sessions=self.rows, starts=starts, finishes=finishes,
                    qualified=False, selected=False,
                    socket_removed=socket_removed,
                    coverage='all registered starts/finishes/connections joined; pre-first-record/exec gap PENDING, not exhaustive spawn proof')
