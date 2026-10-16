"""Research-only numeric :time comparison; original response bytes stay intact.

This policy is never the default and never changes input or cache-key bytes.
Only a numeric value of one bare :time key in a complete top-level flat numeric
statistics list may differ. Every other byte, including spacing and framing,
must match. Models, strings, errors and other statistics are opaque exact bytes.
"""
import hashlib
import re

EXPERIMENT = 'numeric-time-only-experiment-v1'
NUMBER = re.compile(rb'(?:0|[1-9][0-9]*)(?:\.[0-9]+)?\Z')
KEYWORD = re.compile(rb':[A-Za-z][A-Za-z0-9_.-]*\Z')
MAX_BYTES = 16 * 1024 * 1024
MAX_TOKENS = 100000
MAX_DEPTH = 128


def project(data, *, partial=False):
    """Return exact non-time pieces plus retained tokens, or incomplete None.

    The sentinel is an internal comparison representation only, never emitted
    to a verifier, solver, cache or raw evidence stream. Embedded sentinel bytes
    are refused, avoiding a collision with the representation.
    """
    sentinel = b'\x00sail-experiment-numeric-time\x00'
    if not isinstance(data, bytes) or len(data) > MAX_BYTES or sentinel in data:
        raise ValueError('unsupported response bytes/budget')
    spans, children = [], []
    depth = count = pos = 0
    size = len(data)
    while pos < size:
        char = data[pos]
        if char in b' \t\r\n':
            pos += 1
            continue
        if char == ord(';'):
            end = data.find(b'\n', pos)
            if end < 0:
                if partial:
                    return None
                pos = size
            else:
                pos = end + 1
            continue
        if char == ord('('):
            if depth == 1:
                children.append(None)  # Nested responses are never statistics.
            depth += 1
            if depth > MAX_DEPTH:
                raise ValueError('response depth exceeds budget')
            if depth == 1:
                children = []
            pos += 1
            continue
        if char == ord(')'):
            if depth == 0:
                raise ValueError('unmatched response closing delimiter')
            if depth == 1 and children and all(x is not None for x in children):
                keys = [data[a:b] for a, b in children[::2]]
                values = children[1::2]
                if (len(children) % 2 == 0 and len(keys) == len(set(keys)) and
                        all(KEYWORD.fullmatch(key) for key in keys) and
                        all(NUMBER.fullmatch(data[a:b]) for a, b in values) and
                        b':time' in keys):
                    spans.append(values[keys.index(b':time')])
            depth -= 1
            pos += 1
            continue
        start = pos
        quoted = char in (ord('"'), ord('|'))
        if quoted:
            quote = char
            pos += 1
            while pos < size:
                if data[pos] == quote:
                    if quote == ord('"') and pos + 1 < size and data[pos + 1] == quote:
                        pos += 2
                        continue
                    pos += 1
                    break
                pos += 1
            else:
                if partial:
                    return None
                raise ValueError('unterminated response quotation')
        else:
            while pos < size and data[pos] not in b' \t\r\n();"|':
                pos += 1
            if pos == size and partial:
                return None  # A bare token may still be arriving.
        count += 1
        if count > MAX_TOKENS:
            raise ValueError('response token budget exceeded')
        if depth == 1:
            children.append(None if quoted else (start, pos))
    if depth:
        if partial:
            return None
        raise ValueError('incomplete response list')
    pieces, previous, tokens = [], 0, []
    for start, end in spans:
        pieces.extend((data[previous:start], sentinel))
        tokens.append({'offset': start, 'bytes': end - start,
                       'value': data[start:end].decode('ascii')})
        previous = end
    pieces.append(data[previous:])
    return b''.join(pieces), tokens


def compare(left, right):
    """Compare completed responses and expose every retained time-token value."""
    a, av = project(left)
    b, bv = project(right)
    return {'policy': EXPERIMENT, 'raw_equal': left == right,
            'equal_except_numeric_time': a == b,
            'left_sha256': hashlib.sha256(left).hexdigest(),
            'right_sha256': hashlib.sha256(right).hexdigest(),
            'left_time_tokens': av, 'right_time_tokens': bv}


def prefix_state(expected, actual):
    """Compare a streamed preparation prefix without guessing a partial token."""
    reference, _ = project(expected)
    observed = project(actual, partial=True)
    if observed is None:
        return 'incomplete'
    value, _ = observed
    if value == reference:
        return 'equal'
    return 'incomplete' if reference.startswith(value) else 'mismatch'


def compare_prefix(expected, complete):
    """Bind historical prefix replies to a complete fresh raw response stream."""
    reference, values = project(expected)
    whole, whole_values = project(complete)
    return {'policy': EXPERIMENT, 'raw_prefix_equal': complete.startswith(expected),
            'prefix_equal_except_numeric_time': whole.startswith(reference),
            'expected_time_tokens': values,
            'complete_time_tokens': whole_values,
            'expected_sha256': hashlib.sha256(expected).hexdigest(),
            'complete_sha256': hashlib.sha256(complete).hexdigest()}
