"""Audit historical transcript metadata against literal donor input/output.

This never supplies replies or a serving key. Bind source/runtime and the
unique compiler-owned child context/log filename first. No edited input or
query/output normalization is accepted here.
"""
import hashlib

from edit_stream import Commands, Replies, DONE_REPLY, MAX_BYTES, ProtocolRefusal, donor_prefix

TRAILER = b'\n(echo "<<DONE>>")\n'
MARKER = b';;;>>> SAIL-FUNCTION '
QUERY, RESPONSE, END = b';;;>>> QUERY\n', b';;;>>> RESPONSE\n', b';;;<<<\n'


def historical_cut(transcript, donor_input, donor_output, function):
    """Freeze the last checked batch before one compiler function marker.

    Consume query/reply blocks by known literal captured byte lengths: marker
    lookalikes inside quoted commands/replies cannot become metadata. CR-bearing
    replies refuse because the pinned transcript writer removes CR; this audit
    never normalizes them. No actual prefix cost follows from a mapping alone.
    """
    if (not all(isinstance(x, bytes) for x in (transcript, donor_input, donor_output))
            or len(transcript) > MAX_BYTES * 3
            or max(len(donor_input), len(donor_output)) > MAX_BYTES
            or not isinstance(function, str) or not function
            or any(c in function for c in '\r\n\0')):
        raise ProtocolRefusal('bounded literal donor/transcript/function required')
    parser, reader = Commands(), Replies()
    batches, replies = parser.feed(donor_input), reader.feed(donor_output)
    if parser.finish() or reader.finish() or not batches or len(batches) != len(replies):
        raise ProtocolRefusal('complete literal donor batch/reply join required')
    offset = index = 0
    markers = []
    while offset < len(transcript):
        if transcript.startswith(MARKER, offset):
            end = transcript.find(b'\n', offset)
            if end < 0 or end - offset > 8192:
                raise ProtocolRefusal('bounded complete compiler function marker required')
            count, separator, name = transcript[offset + len(MARKER):end].partition(b' ')
            if not separator or count != str(index).encode() or not name.startswith(b'Function-Def '):
                raise ProtocolRefusal('compiler function batch counter differs')
            markers.append((index, name, offset))
            offset = end + 1
            continue
        if index >= len(batches) or not batches[index]['raw'].endswith(TRAILER):
            raise ProtocolRefusal('unexpected transcript or writer batch')
        request, reply = batches[index]['raw'][:-len(TRAILER)], replies[index]
        if not reply.endswith(DONE_REPLY) or b'\r' in reply:
            raise ProtocolRefusal('unsupported literal transcript reply')
        expected = QUERY + request + END + RESPONSE + reply[:-len(DONE_REPLY)] + END
        if not transcript.startswith(expected, offset):
            raise ProtocolRefusal('literal donor/transcript request or reply differs')
        offset += len(expected)
        index += 1
    if index != len(batches):
        raise ProtocolRefusal('transcript misses donor batches')
    target = b'Function-Def ' + function.encode()
    positions = [i for i, row in enumerate(markers) if row[1] == target]
    if len(positions) != 1:
        raise ProtocolRefusal('unique historical function marker required')
    position = positions[0]
    before, _, marker_offset = markers[position]
    after = markers[position + 1][0] if position + 1 < len(markers) else len(batches)
    checks = [b'(check-sat)' in batch['commands'] for batch in batches]
    if not any(checks[before:after]):
        raise ProtocolRefusal('historical function has no supported check')
    preceding = [i + 1 for i in range(before) if checks[i]]
    if not preceding:
        raise ProtocolRefusal('no checked historical prefix before edited function')
    count = preceding[-1]
    prefix = donor_prefix(batches, frozen_batch_count=count)
    return dict(function=function, marker_offset=marker_offset,
                marker_request_batch_count=before, function_end_batch_count=after,
                preceding_checks=sum(checks[:before]), total_checks=sum(checks),
                frozen_prefix_batches=count, prefix_bytes=len(prefix),
                prefix_sha256=hashlib.sha256(prefix).hexdigest(),
                donor_sha256=hashlib.sha256(donor_input).hexdigest(),
                donor_output_sha256=hashlib.sha256(donor_output).hexdigest(),
                compiler_transcript_sha256=hashlib.sha256(transcript).hexdigest(),
                cost_qualified=False, source_runtime_context_binding_required=True)
