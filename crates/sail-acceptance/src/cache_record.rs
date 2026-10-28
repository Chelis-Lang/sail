//! Exact cache header/payload byte framing; no solver-origin authority.
sail_spec! { use vstd::prelude::*; }

sail_spec! {
    /// Whether these bytes contain no record-line separator.
    pub open spec fn record_header(bytes: Seq<u8>) -> bool {
        forall|i: int| 0 <= i < bytes.len() ==> bytes[i] != b'\n'
    }
}

sail_verified! {
    /// Checks that serialized header bytes cannot move the payload separator.
    pub fn header_line(bytes: &[u8]) -> (single_line: bool)
        requires (true)
        ensures (single_line == record_header(bytes@))
    {
        let mut i = 0;
        sail_while! { (i < bytes.len())
            invariant (i <= bytes.len(),
                forall|j: int| 0 <= j < i ==> bytes@[j] != b'\n')
            decreases (bytes.len() - i)
            {
                if bytes[i] == b'\n' { return false; }
                i += 1;
            }
        }
        true
    }
}

sail_verified! {
    /// Encodes exactly one header line, one newline, and the unchanged reply.
    /// A newline-bearing header is refused; this establishes framing, not origin.
    pub fn encode_record(mut header: Vec<u8>, reply: &[u8]) -> (record: Option<Vec<u8>>)
        requires (true)
        ensures (record.is_some() == record_header(header@),
            record.is_some() ==> record.unwrap()@ == header@ + seq![b'\n'] + reply@)
    {
        if !header_line(&header) { return None; }
        header.push(b'\n');
        header.extend_from_slice(reply);
        Some(header)
    }
}

sail_verified! {
    /// Splits at the first actual newline without copying or interpreting payloads.
    /// Missing separators are refused; the returned slices partition the input.
    pub fn split_record(data: &[u8]) -> (parts: Option<(&[u8], &[u8])>)
        requires (true)
        ensures (parts.is_some() == !record_header(data@),
            parts.is_some() ==> record_header(parts.unwrap().0@)
                && data@ == parts.unwrap().0@ + seq![b'\n'] + parts.unwrap().1@)
    {
        let mut i = 0;
        sail_while! { (i < data.len())
            invariant (i <= data.len(),
                forall|j: int| 0 <= j < i ==> data@[j] != b'\n')
            decreases (data.len() - i)
            {
                if data[i] == b'\n' {
                    let parts = (&data[..i], &data[i + 1..]);
                    sail_proof! {
                        assert(data@ =~= parts.0@ + seq![b'\n'] + parts.1@);
                    }
                    return Some(parts);
                }
                i += 1;
            }
        }
        None
    }
}
