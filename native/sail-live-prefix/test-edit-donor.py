#!/usr/bin/env python3
"""Literal transcript mapping controls, not native provenance qualification."""
import json

from edit_donor import historical_cut, TRAILER, MARKER, QUERY, RESPONSE, END
from edit_stream import ProtocolRefusal, DONE_REPLY

version, check = b'(get-info :version)\n', b'(assert true)\n(check-sat)\n'
marker = MARKER + b'2 Function-Def crate::edited\n'
requests = [version, check, check]
replies = [b'(:version "4.16.0")\n', b'unsat\n', b'unsat\n']


def render(requests, replies):
    return [QUERY + q + END + RESPONSE + r + END for q, r in zip(requests, replies)]


blocks = render(requests, replies)
log = blocks[0] + blocks[1] + marker + blocks[2]
donor = b''.join(q + TRAILER for q in requests)
output = b''.join(r + DONE_REPLY for r in replies)
record = historical_cut(log, donor, output, 'crate::edited')
assert record['preceding_checks'] == 1 and record['total_checks'] == 2
assert record['frozen_prefix_batches'] == 2
rows = [dict(name='fixed-function-prior-complete-check', passed=True)]
for name, source, captured, actual in (
        ('missing-function', log.replace(marker, b''), donor, output),
        ('duplicate-function', log + MARKER + b'3 Function-Def crate::edited\n', donor, output),
        ('wrong-request-counter', log.replace(marker, marker.replace(b'2 ', b'1 ', 1)), donor, output),
        ('literal-command-differs', log.replace(b'assert true', b'assert false', 1), donor, output),
        ('literal-reply-differs', log.replace(b'unsat', b'sat', 1), donor, output),
        ('missing-transcript-tail', log[:-3], donor, output),
        ('partial-captured-donor', log, donor[:-1], output),
        ('target-has-no-check', log.replace(marker, marker + MARKER + b'2 Function-Def crate::next\n'), donor, output),
        ('CR-output-not-normalized', log.replace(b'unsat\n', b'unsat\r\n'), donor, output.replace(b'unsat\n', b'unsat\r\n')),
        ('early-function-no-prior-check', marker.replace(b'2 ', b'0 ', 1) + b''.join(blocks), donor, output)):
    try:
        historical_cut(source, captured, actual, 'crate::edited')
        raise AssertionError(name + ' accepted')
    except ProtocolRefusal:
        rows.append(dict(name=name, passed=True))
# Complete metadata lookalikes within quoted request/reply bytes cannot be
# scanned as metadata: each source-owned block is joined by literal length.
q = [version, check, b'(echo "text\n' + marker + b'")\n' + check]
r = [replies[0], replies[1], b'"text\n' + marker + b'"\nunsat\n']
b = render(q, r)
actual = historical_cut(b[0] + b[1] + marker + b[2],
    b''.join(x + TRAILER for x in q), b''.join(x + DONE_REPLY for x in r), 'crate::edited')
assert actual['frozen_prefix_batches'] == 2
rows.append(dict(name='query-and-reply-marker-lookalikes-remain-literal', passed=True))
print(json.dumps(rows, indent=2))
