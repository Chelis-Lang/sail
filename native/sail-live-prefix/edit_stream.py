"""Bounded literal Verus batch framing for a source-only research interposer.

This module neither launches a solver nor authorizes a cached reply. It frames
the actual pinned Verus writer's exact DONE command. Original input/output
bytes are retained; framing never canonicalizes solver input or output.
The caller must retain raw reads before invoking a bounded parser, including
reads that make that parser refuse.
"""

DONE_COMMAND = b'(echo "<<DONE>>")'
DONE_REPLY = b'<<DONE>>\n'
MAX_BYTES = 16 * 1024 * 1024
MAX_DEPTH = 128
MAX_COMMANDS = 100000


class ProtocolRefusal(ValueError):
    """The bounded research protocol cannot safely frame this stream."""


class Commands:
    """Incrementally frame complete exact commands and Verus DONE batches.

    Parentheses in comments, strings and quoted identifiers have no structural
    meaning. A quote at a chunk boundary remains pending until its doubled
    quote/closing-quote meaning is known. Bare top-level atoms and variations
    of the writer's DONE command are unsupported research inputs.
    """

    def __init__(self):
        self.raw = bytearray()
        self.total = self.count = self.depth = 0
        self.state = 'normal'
        self.start = None
        self.await_newline = False
        self.commands = []
        self.failed = False

    def feed(self, data):
        """Return completed batches, retaining literal bytes and command spans."""
        if self.failed:
            raise ProtocolRefusal('framer already refused')
        batches = []
        try:
            if not isinstance(data, bytes) or len(data) > MAX_BYTES - self.total:
                raise ProtocolRefusal('command byte budget')
            self.total += len(data)
            for byte in data:
                self.raw.append(byte)
                if self.await_newline:
                    if byte != 10:
                        raise ProtocolRefusal('DONE command lacks exact writer newline')
                    batches.append({'raw': bytes(self.raw), 'commands': self.commands})
                    self.raw.clear()
                    self.commands = []
                    self.await_newline = False
                    continue
                if self.state == 'comment':
                    if byte == 10:
                        self.state = 'normal'
                    continue
                if self.state == 'string':
                    if byte == 34:
                        self.state = 'string-quote'
                    continue
                if self.state == 'string-quote':
                    if byte == 34:
                        self.state = 'string'
                        continue
                    self.state = 'normal'
                    # The current byte follows the closing quote; consume it
                    # below rather than losing a structural delimiter.
                if self.state == 'quoted':
                    if byte == 124:
                        self.state = 'normal'
                    continue
                if byte == 59:
                    self.state = 'comment'
                elif byte == 34:
                    if not self.depth:
                        raise ProtocolRefusal('top-level string is not a command')
                    self.state = 'string'
                elif byte == 124:
                    if not self.depth:
                        raise ProtocolRefusal('top-level identifier is not a command')
                    self.state = 'quoted'
                elif byte == 40:
                    if not self.depth:
                        self.start = len(self.raw) - 1
                    self.depth += 1
                    if self.depth > MAX_DEPTH:
                        raise ProtocolRefusal('command depth budget')
                elif byte == 41:
                    if not self.depth:
                        raise ProtocolRefusal('unmatched command close')
                    self.depth -= 1
                    if not self.depth:
                        self.count += 1
                        if self.count > MAX_COMMANDS:
                            raise ProtocolRefusal('command count budget')
                        command = bytes(self.raw[self.start:])
                        self.commands.append(command)
                        self.start = None
                        if command == DONE_COMMAND:
                            self.await_newline = True
                elif not self.depth and byte not in b' \t\r\n':
                    raise ProtocolRefusal('top-level atom is not a command')
        except BaseException:
            self.failed = True
            raise
        return batches

    def finish(self):
        """Return literal trailing trivia; refuse incomplete commands/batches."""
        if self.failed or self.depth or self.await_newline or self.commands:
            raise ProtocolRefusal('incomplete or refused command batch')
        if self.state not in ('normal', 'comment'):
            raise ProtocolRefusal('incomplete command quotation')
        return bytes(self.raw)


class Replies:
    """Retain literal reply batches ending at the actual DONE response line."""

    def __init__(self):
        self.raw = bytearray()
        self.line = bytearray()
        self.total = 0
        self.failed = False

    def feed(self, data):
        """Return complete raw batches without modifying any response byte."""
        if self.failed:
            raise ProtocolRefusal('reply framer already refused')
        if not isinstance(data, bytes) or len(data) > MAX_BYTES - self.total:
            self.failed = True
            raise ProtocolRefusal('reply byte budget')
        self.total += len(data)
        batches = []
        for byte in data:
            self.raw.append(byte)
            self.line.append(byte)
            if byte == 10:
                if self.line == DONE_REPLY:
                    batches.append(bytes(self.raw))
                    self.raw.clear()
                self.line.clear()
        return batches

    def finish(self):
        """Require all output to belong to completed batches, even on failure."""
        if self.failed or self.raw:
            raise ProtocolRefusal('unmatched or incomplete reply bytes')


def donor_prefix(batches):
    """Choose the first complete check batch of fixed donor session ordinal0.

    No future input is accepted by this function. Unrecognized check spelling,
    absent checks or incomplete batches cannot produce a preparation prefix.
    """
    prefix = []
    for batch in batches:
        prefix.append(batch['raw'])
        if b'(check-sat)' in batch['commands']:
            return b''.join(prefix)
    raise ProtocolRefusal('donor has no complete supported first check batch')


class PrefixReplay:
    """Pure routing decisions, not a solver/provenance/resource authority.

    A future broker must supply actual prepared-child batches and their bound
    origin. This source-only state machine cannot itself make replies safe for
    a verifier. It freezes readiness once and makes the post-reply abort
    boundary explicit; there is no later readiness setter or child swap.
    """

    def __init__(self, commands, replies, *, ready_at_edit_start):
        if type(ready_at_edit_start) is not bool:
            raise ProtocolRefusal('explicit readiness snapshot required')
        if not commands or len(commands) != len(replies):
            raise ProtocolRefusal('prepared command/reply batch count')
        parsed = []
        command_bytes = reply_bytes = 0
        for command, reply in zip(commands, replies):
            command_bytes += len(command)
            reply_bytes += len(reply)
            if max(command_bytes, reply_bytes) > MAX_BYTES:
                raise ProtocolRefusal('prepared ledger byte budget')
            command_parser, reply_parser = Commands(), Replies()
            framed = command_parser.feed(command)
            if len(framed) != 1 or framed[0]['raw'] != command or command_parser.finish():
                raise ProtocolRefusal('prepared command boundary')
            if reply_parser.feed(reply) != [reply]:
                raise ProtocolRefusal('prepared reply boundary')
            reply_parser.finish()
            parsed.extend(framed)
        if donor_prefix(parsed) != b''.join(commands):
            raise ProtocolRefusal('prepared prefix exceeds fixed first check batch')
        self.commands = tuple(commands)
        self.replies = tuple(replies)
        self._ready_at_edit_start = ready_at_edit_start
        self.index = 0
        self.emitted = False
        self.terminal = False

    def consume(self, batch):
        """Return a literal reply, original-child continuation, or refusal."""
        if self.terminal:
            raise ProtocolRefusal('attempt already refused')
        if not self._ready_at_edit_start:
            self.terminal = True
            return {'action': 'fresh-before-reply', 'reason': 'not-ready-at-edit-start'}
        if self.index == len(self.commands):
            return {'action': 'continue-original-child', 'input': batch}
        if batch != self.commands[self.index]:
            self.terminal = True
            return {'action': 'abort-whole-attempt' if self.emitted else 'fresh-before-reply',
                    'reason': 'literal-input-divergence'}
        reply = self.replies[self.index]
        self.index += 1
        self.emitted |= bool(reply)
        return {'action': 'prepared-reply', 'reply': reply}

    def fail(self, reason):
        """Child/IPC/accounting failure cannot become a successful child swap."""
        self.terminal = True
        return {'action': 'abort-whole-attempt' if self.emitted else 'fresh-before-reply',
                'reason': reason}
