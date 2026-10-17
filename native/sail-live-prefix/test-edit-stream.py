#!/usr/bin/env python3
"""Actual byte framing/routing controls; no broker or native Z3 qualification."""
import json
import os
import selectors
import subprocess
import sys
import time

from edit_stream import Commands, Replies, PrefixReplay, ProtocolRefusal, donor_prefix, MAX_DEPTH


VERSION = b'; donor trivia with (echo "<<DONE>>")\n(get-info :version)\n(echo "<<DONE>>")\n'
CHECK = b'(assert (= |name(with)paren| "text ) ; ""(echo <<DONE>>)"))\n(check-sat)\n(echo "<<DONE>>")\n'
LATER = b'(get-info :all-statistics)\n(echo "<<DONE>>")\n'
VR = b'(:version "4.16.0")\n<<DONE>>\n'
CR = b'unsat\n<<DONE>>\n'
rows = []


def refuse(name, operation):
    try:
        operation()
    except ProtocolRefusal as error:
        rows.append({'name': name, 'refused': str(error)})
        return
    raise AssertionError(name + ' was accepted')


data = VERSION + CHECK + LATER
for cut in range(len(data) + 1):
    parser = Commands()
    batches = parser.feed(data[:cut]) + parser.feed(data[cut:])
    assert [b['raw'] for b in batches] == [VERSION, CHECK, LATER]
    assert parser.finish() == b''
    assert donor_prefix(batches) == VERSION + CHECK
# The application framer supports deeply nested literal solver commands without
# recursive parsing. The prospective cap is finite; the next level still refuses.
nested = b'(assert ' + b'(' * (MAX_DEPTH - 1) + b'x' + b')' * MAX_DEPTH + b'\n(echo "<<DONE>>")\n'
for cut in (0, 1, len(nested) // 2, len(nested)):
    bounded = Commands()
    framed = bounded.feed(nested[:cut]) + bounded.feed(nested[cut:])
    assert [batch['raw'] for batch in framed] == [nested]
    assert bounded.finish() == b''
refuse('next-application-depth-refuses', lambda: Commands().feed(b'(' * (MAX_DEPTH + 1)))
rows.append({'name': 'application-depth-exact-boundary', 'maximum_depth': MAX_DEPTH})
parser = Commands()
one_byte = [batch for byte in data for batch in parser.feed(bytes([byte]))]
assert b''.join(b['raw'] for b in one_byte) == data
assert parser.finish() == b''
rows.append({'name': 'commands-all-splits-and-one-byte', 'split_positions': len(data) + 1})

reply = b'(model (define-fun |<<DONE>>| () String "<<DONE>>"))\n<<DONE>>\n'
for cut in range(len(reply) + 1):
    parser = Replies()
    assert parser.feed(reply[:cut]) + parser.feed(reply[cut:]) == [reply]
    parser.finish()
rows.append({'name': 'reply-quoted-DONE-all-splits', 'split_positions': len(reply) + 1})

for name, value in [('partial-batch', VERSION[:-1]), ('partial-command', b'(assert true'),
                    ('partial-string', b'(echo "x'), ('partial-quoted', b'(declare-fun |x'),
                    ('missing-DONE', b'(check-sat)\n'), ('bare-atom', b'check-sat\n'),
                    ('unmatched-close', b')\n'), ('missing-writer-newline', VERSION[:-1] + b' '),
                    ('quoted-DONE-lookalike', b'(echo "\"<<DONE>>\"")\n')]:
    def incomplete(value=value):
        parser = Commands()
        parser.feed(value)
        parser.finish()
    refuse(name, incomplete)
refuse('reply-incomplete-after-DONE', lambda: (lambda p: (p.feed(VR + b'partial'), p.finish()))(Replies()))
refuse('unmatched-reply', lambda: (lambda p: (p.feed(b'unsat\n'), p.finish()))(Replies()))
refuse('future-prefix-expansion', lambda: PrefixReplay([VERSION, CHECK, LATER], [VR, CR, VR], ready_at_edit_start=True))
refuse('reply-count', lambda: PrefixReplay([VERSION, CHECK], [VR], ready_at_edit_start=True))

historical = [VERSION, CHECK, CHECK]
parsed = Commands().feed(b''.join(historical))
assert donor_prefix(parsed, frozen_batch_count=3) == b''.join(historical)
frozen = PrefixReplay(historical, [VR, CR, CR], ready_at_edit_start=True, frozen_batch_count=3)
for request, expected in zip(historical, [VR, CR, CR]):
    assert frozen.consume(request) == dict(action='prepared-reply', reply=expected)
refuse('historical-cut-not-complete-check', lambda: donor_prefix(Commands().feed(VERSION + CHECK + LATER), frozen_batch_count=3))
refuse('historical-cut-missing-batches', lambda: donor_prefix(parsed, frozen_batch_count=4))
refuse('historical-cut-zero', lambda: donor_prefix(parsed, frozen_batch_count=0))
refuse('historical-ledger-count-differs', lambda: PrefixReplay(historical, [VR, CR, CR], ready_at_edit_start=True, frozen_batch_count=2))
rows.append(dict(name='explicit-historical-cut-only-no-default-expansion', passed=True))

unready = PrefixReplay([VERSION, CHECK], [VR, CR], ready_at_edit_start=False)
assert unready.consume(VERSION)['action'] == 'fresh-before-reply'
refuse('late-ready-cannot-resurrect-attempt', lambda: unready.consume(VERSION))
early = PrefixReplay([VERSION, CHECK], [VR, CR], ready_at_edit_start=True)
assert early.consume(VERSION.replace(b':version', b':name'))['action'] == 'fresh-before-reply'
for reason in ['literal-divergence', 'child-failure', 'ipc-failure', 'accounting-failure']:
    replay = PrefixReplay([VERSION, CHECK], [VR, CR], ready_at_edit_start=True)
    assert replay.consume(VERSION) == {'action': 'prepared-reply', 'reply': VR}
    decision = replay.consume(CHECK.replace(b'check-sat', b'check-sat-assuming')) if reason == 'literal-divergence' else replay.fail(reason)
    assert decision['action'] == 'abort-whole-attempt'
    refuse(reason + '-cannot-continue-after-abort', lambda: replay.consume(CHECK))
rows.append({'name': 'readiness-and-before-after-reply-boundaries', 'passed': True})

# A real owned caller waits for version/DONE before writing its check batch.
# The test has no actual solver: the prepared-reply fixture origin is explicit.
child_source = r'''
import os,sys
sys.stdout.buffer.write(bytes.fromhex(sys.argv[1]));sys.stdout.buffer.flush()
actual=bytearray()
while not actual.endswith(b'<<DONE>>\n'):
 block=os.read(0,1)
 if not block:raise SystemExit(7)
 actual+=block
assert bytes(actual)==bytes.fromhex(sys.argv[3])
sys.stdout.buffer.write(bytes.fromhex(sys.argv[2]));sys.stdout.buffer.flush()
actual=bytearray()
while not actual.endswith(b'<<DONE>>\n'):
 block=os.read(0,1)
 if not block:raise SystemExit(8)
 actual+=block
assert bytes(actual)==b'unsat\n<<DONE>>\n'
'''
child = subprocess.Popen([sys.executable, '-B', '-c', child_source, VERSION.hex(), CHECK.hex(), VR.hex()],
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
selector = selectors.DefaultSelector()
parser = Commands()
replay = PrefixReplay([VERSION, CHECK], [VR, CR], ready_at_edit_start=True)
raw = bytearray()
try:
    selector.register(child.stdout, selectors.EVENT_READ)
    deadline = time.monotonic() + 5
    while selector.get_map():
        assert time.monotonic() < deadline, 'owned caller deadlock'
        for key, _ in selector.select(.05):
            block = os.read(key.fileobj.fileno(), 65536)
            if not block:
                selector.unregister(key.fileobj)
                continue
            raw.extend(block)  # retain reads BEFORE parsing/admission
            for batch in parser.feed(block):
                decision = replay.consume(batch['raw'])
                assert decision['action'] == 'prepared-reply'
                child.stdin.write(decision['reply'])
                child.stdin.flush()
    assert parser.finish() == b'' and bytes(raw) == VERSION + CHECK
    stdout, stderr = child.communicate(timeout=1)
    assert child.returncode == 0 and stdout == stderr == b''
    rows.append({'name': 'actual-owned-early-version-wait', 'exit': child.returncode,
                 'input_bytes': len(raw), 'solver_origin': 'explicit prepared-reply fixture only'})
finally:
    selector.close()
    if child.poll() is None:
        child.kill()
    child.communicate(timeout=1)

print(json.dumps({'passed': True, 'rows': rows, 'scope': __doc__}, indent=2))
