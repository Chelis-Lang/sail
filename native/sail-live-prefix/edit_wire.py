"""Bounded single-connection research IPC for the disabled edit driver.

The enclosing driver must own a private authenticated connection, the compiler
attempt and all resources. This module's completion is not a verification
verdict. A compiler ignoring a late child exit still needs the outer gate.
"""
import hashlib
import json
import struct

from edit_stream import Commands, MAX_BYTES, ProtocolRefusal


def send(connection, kind, raw):
    """Send one bounded literal frame; the tag is not a solver-output byte."""
    if kind not in b'HIEOXD' or not isinstance(raw, bytes) or len(raw) > MAX_BYTES:
        raise ProtocolRefusal('unsupported IPC frame')
    connection.sendall(bytes([kind]) + struct.pack('!I', len(raw)) + raw)


def receive(connection, on_body=None):
    """Receive a complete bounded frame or refuse EOF/partial/unknown data."""
    def exact(size, kind=None):
        result = bytearray()
        while len(result) < size:
            block = connection.recv(min(65536, size - len(result)))
            if not block:
                raise ProtocolRefusal('IPC EOF before complete frame')
            if on_body is not None and kind is not None:
                on_body(kind, block)
            result.extend(block)
        return bytes(result)
    header = exact(5)
    size = struct.unpack('!I', header[1:])[0]
    if header[0] not in b'HIEOXD' or size > MAX_BYTES:
        raise ProtocolRefusal('unsupported IPC tag/size')
    return header[0], exact(size, header[0])


def attempt_status(compiler_exit, sessions):
    """Require broker completion independently of compiler exit; never verify."""
    valid = (type(compiler_exit) is int and compiler_exit == 0 and
             isinstance(sessions, list) and bool(sessions) and
             all(isinstance(row, dict) and row.get('passed') is True and row.get('complete') is True and
                 not row.get('error') for row in sessions))
    return {'compiler_exit': compiler_exit, 'driver_exit': 0 if valid else 1,
            'broker_gate': valid, 'qualified': False, 'selected': False,
            'whole_fresh_retry_required': not valid}


def serve(connection, claim, binding, token, *, fresh_factory=None, identity=None, initial_header=None):
    """Serve one already assigned claim, with at most one pre-reply fallback.

    `claim` and fresh_factory own actual bound children; no received metadata
    can fabricate process accounting. Header values are checked, never copied
    to raw evidence: they may contain private environment binding material.
    SMT reads and forwarded replies are retained before routing/writing.
    """
    result = {'passed': False, 'complete': False, 'error': None,
              'streaming_policy': 'donor-first-check-stream-v1',
              'input': bytearray(), 'output': bytearray(), 'claims': [claim],
              'fallback': False, 'header_sha256': None}
    current = claim
    emitted = False
    parser = Commands()
    def capture(kind, raw):
        if kind == ord('I'):
            available = max(0, MAX_BYTES - len(result['input']))
            result['input'].extend(raw[:available])
            if len(raw) > available:
                raise ProtocolRefusal('whole IPC input budget; prefix retained')
    try:
        kind, raw = receive(connection) if initial_header is None else (ord('H'), initial_header)
        result['header_sha256'] = hashlib.sha256(raw).hexdigest()
        if kind != ord('H') or len(raw) > 65536:
            raise ProtocolRefusal('bounded initial IPC header required')
        header = json.loads(raw)
        expected_keys = {'binding', 'token'} | ({'identity'} if identity is not None else set())
        if set(header) != expected_keys or header['token'] != token:
            raise ProtocolRefusal('IPC authentication refused')
        if identity is not None and header['identity'] != identity:
            raise ProtocolRefusal('owned attempt/session identity refused')
        if header['binding'] != binding:
            raise ProtocolRefusal('exact caller context binding refused')
        while True:
            kind, raw = receive(connection, capture)
            if kind == ord('I'):
                # Keep exact batch bytes, releasing parsed command copies
                # before blocking on a child. The full framing checks ran.
                batches = tuple(batch['raw'] for batch in parser.feed(raw))
                for batch in batches:
                    decision = current.batch(batch)
                    if decision['action'] == 'fresh-before-reply' and not emitted and fresh_factory is not None:
                        current.close()
                        result['fallback'] = True
                        current = fresh_factory()
                        result['claims'].append(current)
                        fresh_factory = None
                        decision = current.batch(batch)
                    if decision['action'] not in ('prepared-reply', 'original-child-reply'):
                        raise ProtocolRefusal(decision['reason'])
                    reply = decision['reply']
                    if len(reply) > MAX_BYTES - len(result['output']):
                        raise ProtocolRefusal('whole IPC reply budget')
                    result['output'].extend(reply)
                    emitted = True
                    send(connection, ord('O'), reply)
            elif kind == ord('E'):
                if raw or not current.finish(parser.finish()):
                    raise ProtocolRefusal('final owned child/I/O/accounting gate refused')
                result.update(passed=True, complete=True)
                send(connection, ord('D'), b'{"exit":0}')
                break
            elif kind == ord('X'):
                raise ProtocolRefusal('caller input/output/IPC refusal')
            else:
                raise ProtocolRefusal('unexpected IPC request')
    except (OSError, ValueError, RuntimeError) as error:
        result['error'] = type(error).__name__ + ': ' + str(error)
        result['passed'] = False
        if current is not None:
            current.abort(result['error'])
        try:
            send(connection, ord('X'), b'research attempt refused; outer full attempt must fail')
        except OSError:
            pass
    finally:
        for owned in result['claims']:
            owned.close()
            if owned.errors and owned is current:
                result['passed'] = False
                result['error'] = result['error'] or 'owned cleanup refused'
            entry = owned.entry
            if entry is None or entry.kernel_accounting is None or entry.cleanup_errors:
                result['passed'] = False
                result['error'] = result['error'] or 'complete preparation/fresh cost or cleanup unavailable'
            elif not 0 < entry.kernel_accounting['accounting']['peak_rss_bytes'] <= owned.pool.bytes_per_slot:
                result['passed'] = False
                result['error'] = result['error'] or 'preparation/fresh kernel peak bound refused'
        connection.close()
        result['input'] = bytes(result['input'])
        result['output'] = bytes(result['output'])
        result['emitted_reply'] = emitted
    return result


def interpose(connection, binding, token, source, sink, *, identity=None):
    """Sequential Verus writer protocol over an already private connection.

    No complete-session buffering: version/DONE is forwarded before asking the
    compiler for another batch. The outer driver must inspect serve()'s actual
    result, even when a caller ignores this function's nonzero exit status.
    """
    parser = Commands()
    try:
        metadata = {'binding': binding, 'token': token}
        if identity is not None:
            metadata['identity'] = identity
        header = json.dumps(metadata, sort_keys=True).encode()
        if len(header) > 65536:
            raise ProtocolRefusal('IPC header budget')
        send(connection, ord('H'), header)
        while raw := source.read1(65536):
            # The receiver journals every raw chunk before either side parses
            # it, including a partial batch or a later protocol refusal.
            send(connection, ord('I'), raw)
            for batch in parser.feed(raw):
                kind, reply = receive(connection)
                if kind != ord('O'):
                    raise ProtocolRefusal('broker refused batch')
                if sink.write(reply) != len(reply):
                    raise ProtocolRefusal('incomplete literal reply forwarding')
                sink.flush()
        parser.finish()
        send(connection, ord('E'), b'')
        kind, raw = receive(connection)
        if kind != ord('D') or raw != b'{"exit":0}':
            raise ProtocolRefusal('broker refused final completion')
        return 0
    except (OSError, ValueError, RuntimeError):
        try:
            send(connection, ord('X'), b'caller refused protocol or output')
        except OSError:
            pass
        return 1
    finally:
        connection.close()
