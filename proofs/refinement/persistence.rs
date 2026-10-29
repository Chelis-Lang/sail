//! Complete cache record sequencing root, conditional on its explicit host contract.
use vstd::prelude::*;

verus! {
    /// The actual transaction writes a nonempty header and reply in order.
    pub fn persistence_constructive_control() -> (passed: bool)
        ensures passed,
    { crate::cache_write::successful_write_control() }
/// A first-newline partition is unique even when the reply contains newlines.
/// This relates actual byte framing; it grants no meaning to header metadata.
pub proof fn record_partition_unique(header: Seq<u8>, reply: Seq<u8>,
    decoded_header: Seq<u8>, decoded_reply: Seq<u8>)
    requires crate::cache_record::record_header(header),
        crate::cache_record::record_header(decoded_header),
        header + seq![b'\n'] + reply == decoded_header + seq![b'\n'] + decoded_reply,
    ensures decoded_header == header, decoded_reply == reply,
{
    let whole = header + seq![b'\n'] + reply;
    if header.len() < decoded_header.len() {
        assert(whole[header.len() as int] == b'\n');
        assert(whole[header.len() as int] == decoded_header[header.len() as int]);
    } else if decoded_header.len() < header.len() {
        assert(whole[decoded_header.len() as int] == b'\n');
        assert(whole[decoded_header.len() as int] == header[decoded_header.len() as int]);
    }
    assert(header.len() == decoded_header.len());
    assert(whole.take(header.len() as int) =~= header);
    assert(whole.take(decoded_header.len() as int) =~= decoded_header);
    assert(header == decoded_header);
    assert(whole.subrange(header.len() as int + 1, whole.len() as int) =~= reply);
    assert(whole.subrange(decoded_header.len() as int + 1, whole.len() as int) =~= decoded_reply);
    assert(reply == decoded_reply);
}

/// The actual writer encoding and actual reader split preserve arbitrary payload
/// bytes. Metadata interpretation, digest computation and OS writes stay outside.
pub fn record_bytes_roundtrip(header: Vec<u8>, reply: &[u8])
    -> (decoded: Option<(Vec<u8>, Vec<u8>)>)
    ensures decoded.is_some() == crate::cache_record::record_header(header@),
        decoded.is_some() ==> decoded.unwrap().0@ == header@
            && decoded.unwrap().1@ == reply@,
{
    let ghost original_header = header@;
    let encoded = crate::cache_record::encode_record(header, reply);
    let encoded = match encoded { Some(encoded) => encoded, None => return None };
    let parts = crate::cache_record::split_record(&encoded);
    proof { assert(!crate::cache_record::record_header(encoded@)) by {
        assert(encoded@[original_header.len() as int] == b'\n');
    } }
    let (header, decoded_reply) = match parts { Some(parts) => parts, None => return None };
    proof { record_partition_unique(original_header, reply@, header@, decoded_reply@); }
    let mut header_copy = Vec::new();
    header_copy.extend_from_slice(header);
    let mut reply_copy = Vec::new();
    reply_copy.extend_from_slice(decoded_reply);
    Some((header_copy, reply_copy))
}

/// Nonempty multiline/binary reply survives exact framing; an embedded newline
/// in a purported serialized header is refused before it can relabel payloads.
pub fn record_payload_and_header_refusal_witness() {
    let header = vec![b'{', b'}'];
    let reply = vec![b'u', b'n', b's', b'a', b't', b'\n', 0u8, 255u8];
    let decoded = record_bytes_roundtrip(header, &reply);
    proof {
        assert(decoded.is_some());
        assert(decoded.unwrap().0@ == seq![b'{', b'}']);
        assert(decoded.unwrap().1@ == reply@);
    }
    let malformed_header = vec![b'{', b'\n', b'}'];
    let refused = crate::cache_record::encode_record(malformed_header, &reply);
    proof { assert(refused.is_none()); }
    let unterminated = vec![b'{', b'}'];
    let refused = crate::cache_record::split_record(&unterminated);
    proof { assert(refused.is_none()); }
}


}
