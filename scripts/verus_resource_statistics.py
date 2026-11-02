"""Lossless, bounded classification of the three ADR 0097 consumption fields."""
import json
import re

MAX_BYTES = 16 * 1024 * 1024
MAX_TOKENS = 200_000
MAX_DEPTH = 64
TOKEN = re.compile(r'[ \t\r\n]+|"(?:[^"\\\x00-\x1f]|\\["\\/bfnrt]|\\u[0-9a-fA-F]{4})*"|-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?|true|false|null|[{}\[\]:,]')
UINT = re.compile(r'(?:0|[1-9][0-9]*)\Z')
BASE = ('times-ms', 'smt')
MODULES = BASE + ('smt-run-module-times',)


def counter_path(path):
    return (path == BASE + ('rlimit-run',)
            or len(path) == 5 and path[:3] == MODULES
            and type(path[3]) is int and path[4] == 'rlimit'
            or len(path) == 7 and path[:3] == MODULES
            and type(path[3]) is int and path[4] == 'function-breakdown'
            and type(path[5]) is int and path[6] == 'rlimit')


def expected_container(path):
    if path in (('times-ms',), BASE):
        return '{'
    if path == MODULES:
        return '['
    if len(path) == 4 and path[:3] == MODULES and type(path[3]) is int:
        return '{'
    if len(path) == 5 and path[:3] == MODULES and type(path[3]) is int and path[4] == 'function-breakdown':
        return '['
    if len(path) == 6 and path[:3] == MODULES and type(path[3]) is int and path[4] == 'function-breakdown' and type(path[5]) is int:
        return '{'
    return None


class Lossless:
    def __init__(self, text):
        if len(text.encode('utf-8')) > MAX_BYTES:
            raise ValueError('JSON byte limit')
        self.text = text
        self.tokens = []
        end = 0
        while end < len(text):
            # Anchoring avoids repeated scans of an unterminated escaped string.
            match = TOKEN.match(text, end)
            if match is None:
                raise ValueError('unrecognized JSON byte')
            end = match.end()
            if not match.group().isspace():
                self.tokens.append((match.group(), match.start(), match.end()))
                if len(self.tokens) > MAX_TOKENS:
                    raise ValueError('JSON token limit')
        if end != len(text):
            raise ValueError('unrecognized JSON tail')
        self.index = 0
        self.spans = []

    def peek(self):
        return self.tokens[self.index][0] if self.index < len(self.tokens) else None

    def take(self, expected=None):
        if self.index >= len(self.tokens):
            raise ValueError('truncated JSON')
        token = self.tokens[self.index]
        self.index += 1
        if expected is not None and token[0] != expected:
            raise ValueError('unexpected JSON token')
        return token

    def value(self, path=(), depth=0):
        if depth > MAX_DEPTH:
            raise ValueError('JSON depth limit')
        token, start, end = self.take()
        container = expected_container(path)
        if container is not None and token != container:
            raise ValueError('unrecognized statistics container')
        if counter_path(path):
            if not UINT.fullmatch(token) or len(token) > 20 or int(token) > 2**64 - 1:
                raise ValueError('consumption is not uint64')
            self.spans.append((start, end, path, int(token)))
            return
        if token == '{':
            keys = set()
            if self.peek() == '}':
                self.take('}')
                return
            while True:
                key, _, _ = self.take()
                if not key.startswith('"'):
                    raise ValueError('object key is not a string')
                key = json.loads(key)
                if key in keys:
                    raise ValueError('duplicate JSON key')
                keys.add(key)
                self.take(':')
                self.value(path + (key,), depth + 1)
                tail = self.take()[0]
                if tail == '}':
                    return
                if tail != ',':
                    raise ValueError('missing object separator')
        elif token == '[':
            if self.peek() == ']':
                self.take(']')
                return
            index = 0
            while True:
                self.value(path + (index,), depth + 1)
                index += 1
                tail = self.take()[0]
                if tail == ']':
                    return
                if tail != ',':
                    raise ValueError('missing array separator')
        elif not (token.startswith('"') or token in ('true', 'false', 'null') or re.fullmatch(r'-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?', token)):
            raise ValueError('invalid JSON value')

    def masked(self):
        self.value()
        if self.index != len(self.tokens):
            raise ValueError('trailing JSON token')
        parts, previous = [], 0
        for start, end, _, _ in self.spans:
            parts.extend((self.text[previous:start], '0'))
            previous = end
        parts.append(self.text[previous:])
        return ''.join(parts), {path: value for _, _, path, value in self.spans}


def classify_stdout(left, right):
    """Inputs already have the unchanged strict tool's wall-time handling."""
    if left == right:
        return {'classification': 'strict-same', 'differences': []}
    try:
        left_mask, left_values = Lossless(left).masked()
        right_mask, right_values = Lossless(right).masked()
    except (ValueError, RecursionError, UnicodeError) as error:
        return {'classification': 'strict-different', 'reason': str(error)}
    if left_mask != right_mask or left_values.keys() != right_values.keys():
        return {'classification': 'strict-different', 'reason': 'another byte, ordering or shape differs'}
    differences = [{'path': list(path), 'reference': left_values[path], 'candidate': right_values[path]}
                   for path in left_values if left_values[path] != right_values[path]]
    if not differences:
        return {'classification': 'strict-different', 'reason': 'no recognized consumption difference'}
    return {'classification': 'consumed-resource-only', 'differences': differences}
