//! Exact incremental v2 key framing, conditional only on narrow hash primitives.
//!
//! The production SHA adapter supplies fresh, append, copy and finalize effects.
//! These operations establish byte identity; a digest never supplies solver origin.
use crate::query::PrefixFeed;
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_trait! {
    /// Incremental hash primitives. Approved production implementations are source-bound.
    pub trait HashHost: Sized {
        spec {
            spec fn bytes(&self) -> Seq<u8>;
            spec fn digest(bytes: Seq<u8>) -> [u8; 32];
        }
        /// Appends every supplied byte in order to this hash state.
        fn append(&mut self, bytes: &[u8]) -> (out: ())
            requires (true) ensures (final(self).bytes() == old(self).bytes() + bytes@);
        /// Copies this exact incremental state without changing it.
        #[must_use]
        fn duplicate(&self) -> (out: Self)
            requires (true) ensures (out.bytes() == self.bytes());
        /// Creates an empty state for the same hash algorithm.
        #[must_use]
        fn empty(&self) -> (out: Self)
            requires (true) ensures (out.bytes() == Seq::<u8>::empty());
        /// Finalizes a copy of the exact current bytes using this algorithm.
        fn finish(self) -> (out: [u8; 32])
            requires (true) ensures (out == Self::digest(self.bytes()));
    }
}

sail_spec! {
    pub open spec fn field(bytes: Seq<u8>) -> Seq<u8> {
        vstd::bytes::spec_u64_to_le_bytes(bytes.len() as u64) + bytes
    }
    #[verifier::opaque]
    pub open spec fn header(name: Seq<u8>, version: Seq<u8>, binary: Seq<u8>, platform: Seq<u8>, mode: Option<Seq<u8>>) -> Seq<u8> {
        field(b"sail-key/prefix/v2"@)
            + (match mode { None => field(b"mode:default"@), Some(value) => field(b"mode:opt-in"@) + field(value) })
            + field(name) + field(version) + field(binary) + field(platform)
    }
    pub open spec fn end(material: Seq<u8>, length: u64) -> Seq<u8> {
        material + b"\x00sail-key/prefix/v2/end"@ + vstd::bytes::spec_u64_to_le_bytes(length)
    }
}

sail_verified! {
    /// Encodes the unchanged eight little-endian length bytes without allocation.
    #[allow(clippy::cast_possible_truncation, reason = "Each byte selects its checked little-endian octet.")]
    pub fn encode_word(word: u64) -> (encoded: [u8; 8])
        requires (true) ensures (encoded@ == vstd::bytes::spec_u64_to_le_bytes(word))
    {
        let encoded = [word as u8, (word >> 8) as u8, (word >> 16) as u8, (word >> 24) as u8,
            (word >> 32) as u8, (word >> 40) as u8, (word >> 48) as u8, (word >> 56) as u8];
        sail_proof! {
            vstd::bytes::spec_u64_to_le_bytes_to_open(word);
            assert(word as u8 == (word & 255u64) as u8) by (bit_vector);
            assert((word >> 8) as u8 == ((word >> 8) & 255u64) as u8) by (bit_vector);
            assert((word >> 16) as u8 == ((word >> 16) & 255u64) as u8) by (bit_vector);
            assert((word >> 24) as u8 == ((word >> 24) & 255u64) as u8) by (bit_vector);
            assert((word >> 32) as u8 == ((word >> 32) & 255u64) as u8) by (bit_vector);
            assert((word >> 40) as u8 == ((word >> 40) & 255u64) as u8) by (bit_vector);
            assert((word >> 48) as u8 == ((word >> 48) & 255u64) as u8) by (bit_vector);
            assert((word >> 56) as u8 == ((word >> 56) & 255u64) as u8) by (bit_vector);
            assert(encoded@ =~= vstd::bytes::spec_u64_to_le_bytes(word));
        }
        encoded
    }
}

sail_verified! {
    /// Emits the existing little-endian length and exact field bytes, without normalization.
    #[allow(clippy::cast_possible_truncation, reason = "Supported usize widths are at most 64 bits; the checked framing preserves this existing disk/key format.")]
    pub fn append_field<Host>(host: &mut Host, bytes: &[u8]) -> (out: ())
        where (Host: HashHost)
        requires (bytes.len() <= u64::MAX)
        ensures (final(host).bytes() == old(host).bytes() + field(bytes@))
    {
        host.append(&encode_word(bytes.len() as u64));
        host.append(bytes);
    }
}

sail_code! {
    /// Owned incremental hash state and checked raw-prefix byte counter.
    pub struct Prefix<Host> {
        host: Host,
        length: u64,
        exact: bool,
    }

    impl<Host: HashHost> Prefix<Host> {
        sail_impl_spec! {
            pub closed spec fn material(&self) -> Seq<u8> { self.host.bytes() }
            pub closed spec fn length(&self) -> u64 { self.length }
            pub closed spec fn exact(&self) -> bool { self.exact }
        }
        sail_method! {
            /// Initializes the actual v2 identity/mode header through the hash primitive.
            pub fn new(mut host: Host, name: &[u8], version: &[u8], binary: &[u8], platform: &[u8], mode: Option<&[u8]>) -> (out: Self)
                requires (host.bytes().len() == 0, name.len() <= u64::MAX, version.len() <= u64::MAX,
                    binary.len() <= u64::MAX, platform.len() <= u64::MAX,
                    mode.is_some() ==> mode.unwrap().len() <= u64::MAX)
                ensures (out.material() == header(name@, version@, binary@, platform@, (match mode { None => None, Some(value) => Some(value@) })),
                    out.length() == 0, out.exact())
            {
                sail_proof! { reveal(header); }
                append_field(&mut host, b"sail-key/prefix/v2");
                match mode {
                    None => append_field(&mut host, b"mode:default"),
                    Some(value) => {
                        append_field(&mut host, b"mode:opt-in");
                        append_field(&mut host, value);
                    }
                }
                append_field(&mut host, name);
                append_field(&mut host, version);
                append_field(&mut host, binary);
                append_field(&mut host, platform);
                Self { host, length: 0, exact: true }
            }
        }
        sail_method! {
            /// Appends original bytes and permanently refuses exact-length authority on overflow.
            #[allow(clippy::cast_possible_truncation, reason = "Supported usize widths are at most 64 bits; overflow of the cumulative counter is checked.")]
            pub fn update(&mut self, bytes: &[u8]) -> (out: ())
                requires (bytes.len() <= u64::MAX)
                ensures (final(self).material() == old(self).material() + bytes@,
                    final(self).exact() == (old(self).exact() && old(self).length() as int + bytes.len() <= u64::MAX),
                    old(self).length() as int + bytes.len() <= u64::MAX ==> final(self).length() == old(self).length() + bytes.len(),
                    old(self).length() as int + bytes.len() > u64::MAX ==> final(self).length() == old(self).length())
            {
                self.host.append(bytes);
                match self.length.checked_add(bytes.len() as u64) {
                    Some(length) => self.length = length,
                    None => self.exact = false,
                }
            }
        }
        sail_method! {
            /// Consumes only the next contiguous owned raw interval; refusal is permanent.
            #[allow(clippy::cast_possible_truncation, reason = "Supported usize widths are at most 64 bits; token endpoints are checked before hashing.")]
            pub fn feed(&mut self, feed: &PrefixFeed) -> (accepted: bool)
                requires (feed.from() <= u64::MAX, feed.to() <= u64::MAX)
                ensures (accepted == (old(self).exact() && feed.from() <= feed.to()
                    && old(self).length() == feed.from() && feed.view().len() == feed.to() - feed.from()),
                    accepted ==> final(self).material() == old(self).material() + feed.view()
                        && final(self).length() == feed.to() && final(self).exact(),
                    !accepted ==> final(self).material() == old(self).material()
                        && final(self).length() == old(self).length() && !final(self).exact())
            {
                let start = feed.start(); let end = feed.end(); let bytes = feed.bytes();
                if !self.exact || start > end || self.length != start as u64 || bytes.len() != end - start {
                    self.exact = false;
                    return false;
                }
                self.update(bytes);
                true
            }
        }
        sail_method! {
            /// Copies the exact incremental state and sticky counter validity.
            #[must_use]
            pub fn duplicate(&self) -> (out: Self)
                requires (true)
                ensures (out.material() == self.material(), out.length() == self.length(), out.exact() == self.exact())
            { Self { host: self.host.duplicate(), length: self.length, exact: self.exact } }
        }
        sail_method! {
            /// Returns the checked cumulative input-byte counter.
            pub fn len(&self) -> (out: u64)
                requires (true) ensures (out == self.length())
            { self.length }
        }
        sail_method! {
            /// Whether no input byte has been counted.
            pub fn is_empty(&self) -> (out: bool)
                requires (true) ensures (out == (self.length() == 0))
            { self.length == 0 }
        }
        sail_method! {
            /// Whether the counter still exactly represents every supplied input byte.
            pub fn is_exact(&self) -> (out: bool)
                requires (true) ensures (out == self.exact())
            { self.exact }
        }
        sail_method! {
            /// Finalizes only a duplicate after appending the unchanged v2 end marker and length.
            pub fn key(&self) -> (out: [u8; 32])
                requires (true) ensures (out == Host::digest(end(self.material(), self.length())))
            {
                let mut hash = self.host.duplicate();
                hash.append(b"\x00sail-key/prefix/v2/end");
                hash.append(&encode_word(self.length));
                hash.finish()
            }
        }
        sail_method! {
            /// Domain-separates complete-session silence using the exact existing nested v2 key.
            pub fn silence_key(&self) -> (out: [u8; 32])
                requires (true)
                ensures (out == Host::digest(field(b"sail-key/session-silence/v1"@)
                    + field(Host::digest(end(self.material(), self.length()))@)))
            {
                let mut hash = self.host.empty();
                append_field(&mut hash, b"sail-key/session-silence/v1");
                append_field(&mut hash, &self.key());
                hash.finish()
            }
        }
    }
}

sail_spec! {
    #[verifier::opaque]
    pub open spec fn encode_fields(values: Seq<Seq<u8>>) -> Seq<u8>
        decreases values.len()
    {
        if values.len() == 0 { Seq::empty() }
        else { field(values.first()) + encode_fields(values.subrange(1, values.len() as int)) }
    }

    pub proof fn field_prefix_injective(a: Seq<u8>, a_tail: Seq<u8>, b: Seq<u8>, b_tail: Seq<u8>)
        requires a.len() <= u64::MAX, b.len() <= u64::MAX,
            field(a) + a_tail == field(b) + b_tail
        ensures a == b, a_tail == b_tail
    {
        vstd::bytes::spec_u64_to_le_bytes_to_open(a.len() as u64);
        vstd::bytes::spec_u64_to_le_bytes_to_open(b.len() as u64);
        let left = field(a) + a_tail;
        let right = field(b) + b_tail;
        assert(left.take(8) == vstd::bytes::spec_u64_to_le_bytes(a.len() as u64));
        assert(right.take(8) == vstd::bytes::spec_u64_to_le_bytes(b.len() as u64));
        vstd::bytes::lemma_auto_spec_u64_to_from_le_bytes();
        assert(a.len() == b.len());
        assert(left.subrange(8, 8 + a.len() as int) == a);
        assert(right.subrange(8, 8 + b.len() as int) == b);
        assert(left.subrange(8 + a.len() as int, left.len() as int) == a_tail);
        assert(right.subrange(8 + b.len() as int, right.len() as int) == b_tail);
    }

    pub proof fn encoded_fields_injective(a: Seq<Seq<u8>>, b: Seq<Seq<u8>>)
        requires forall|i: int| 0 <= i < a.len() ==> #[trigger] a[i].len() <= u64::MAX,
            forall|i: int| 0 <= i < b.len() ==> #[trigger] b[i].len() <= u64::MAX,
            encode_fields(a) == encode_fields(b)
        ensures a == b
        decreases a.len() + b.len()
    {
        reveal_with_fuel(encode_fields, 1);
        if a.len() == 0 || b.len() == 0 {
            if a.len() > 0 { vstd::bytes::spec_u64_to_le_bytes_to_open(a.first().len() as u64); assert(field(a.first()).len() >= 8); assert(encode_fields(a).len() >= 8); }
            if b.len() > 0 { vstd::bytes::spec_u64_to_le_bytes_to_open(b.first().len() as u64); assert(field(b.first()).len() >= 8); assert(encode_fields(b).len() >= 8); }
            assert(a.len() == 0 && b.len() == 0);
        } else {
            let at = a.subrange(1, a.len() as int);
            let bt = b.subrange(1, b.len() as int);
            field_prefix_injective(a.first(), encode_fields(at), b.first(), encode_fields(bt));
            encoded_fields_injective(at, bt);
            assert(a == seq![a.first()] + at);
            assert(b == seq![b.first()] + bt);
        }
    }

    #[verifier::opaque]
    pub open spec fn identity_fields(name: Seq<u8>, version: Seq<u8>, binary: Seq<u8>, platform: Seq<u8>, mode: Option<Seq<u8>>) -> Seq<Seq<u8>> {
        seq![b"sail-key/prefix/v2"@]
            + (match mode { None => seq![b"mode:default"@], Some(value) => seq![b"mode:opt-in"@, value] })
            + seq![name, version, binary, platform]
    }

    pub proof fn header_fields(name: Seq<u8>, version: Seq<u8>, binary: Seq<u8>, platform: Seq<u8>, mode: Option<Seq<u8>>)
        ensures header(name, version, binary, platform, mode) == encode_fields(identity_fields(name, version, binary, platform, mode))
    {
        reveal(header); reveal(identity_fields);
        reveal_with_fuel(encode_fields, 8);
    }

    pub proof fn header_identity_injective(a: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>),
        b: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>))
        requires a.0.len() <= u64::MAX, a.1.len() <= u64::MAX, a.2.len() <= u64::MAX, a.3.len() <= u64::MAX,
            b.0.len() <= u64::MAX, b.1.len() <= u64::MAX, b.2.len() <= u64::MAX, b.3.len() <= u64::MAX,
            a.4.is_some() ==> a.4.unwrap().len() <= u64::MAX,
            b.4.is_some() ==> b.4.unwrap().len() <= u64::MAX,
            header(a.0, a.1, a.2, a.3, a.4) == header(b.0, b.1, b.2, b.3, b.4)
        ensures a == b
    {
        header_fields(a.0, a.1, a.2, a.3, a.4);
        header_fields(b.0, b.1, b.2, b.3, b.4);
        let af = identity_fields(a.0, a.1, a.2, a.3, a.4);
        let bf = identity_fields(b.0, b.1, b.2, b.3, b.4);
        identity_fields_bounded(a); identity_fields_bounded(b);
        encoded_fields_injective(af, bf);
        identity_fields_tuple_injective(a, b);
    }
}

sail_code! {
    /// Executable recording primitive for conformance controls; not a cryptographic hash.
    pub struct RecordingHash {
        bytes: Vec<u8>,
    }
    impl HashHost for RecordingHash {
        sail_trait_impl_spec! {
            closed spec fn bytes(&self) -> Seq<u8> { self.bytes@ }
            open spec fn digest(bytes: Seq<u8>) -> [u8; 32] { let value = bytes.len() as u8; [value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value] }
        }
        fn append(&mut self, bytes: &[u8]) { self.bytes.extend_from_slice(bytes); }
        fn duplicate(&self) -> Self {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&self.bytes);
            Self { bytes }
        }
        fn empty(&self) -> Self { Self { bytes: Vec::new() } }
        #[allow(clippy::cast_possible_truncation, reason = "This noncryptographic recording fixture intentionally returns its length modulo 256.")]
        fn finish(self) -> [u8; 32] { [self.bytes.len() as u8; 32] }
    }
}

sail_verified! {
    /// Exercises exact nonempty fragments, distinct mode/identity fields, cloned state and finalization.
    pub fn nonempty_framing_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        sail_proof! { reveal(Prefix::material); reveal(Prefix::length); reveal(Prefix::exact); reveal(<RecordingHash as HashHost>::bytes); }
        let mut prefix = Prefix::new(RecordingHash { bytes: Vec::new() }, b"z3", b"4.16.0", b"binary", b"linux-x86_64", Some(b"named-mode"));
        let first = b"; untouched comment\n(assert (str.contains \"a;()\" \"a\"))\n";
        let second = b"(check-sat)";
        prefix.update(first);
        let mut copied = prefix.duplicate();
        prefix.update(second);
        copied.update(b"(get-info :version)");
        let _key = prefix.key();
        let _silence = prefix.silence_key();
        sail_proof! {
            assert(prefix.material() == header(b"z3"@, b"4.16.0"@, b"binary"@, b"linux-x86_64"@, Some(b"named-mode"@)) + first@ + second@);
            assert(prefix.length() == first.len() + second.len());
            assert(_key == RecordingHash::digest(end(prefix.material(), prefix.length())));
            assert(_silence == RecordingHash::digest(field(b"sail-key/session-silence/v1"@) + field(_key@)));
            assert(copied.material() == header(b"z3"@, b"4.16.0"@, b"binary"@, b"linux-x86_64"@, Some(b"named-mode"@)) + first@ + b"(get-info :version)"@);
        }
        prefix.is_exact() && copied.is_exact() && prefix.len() == (first.len() + second.len()) as u64
    }
}

sail_verified! {
    /// An actual counter overflow is sticky while every original byte still reaches the primitive.
    pub fn overflow_framing_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        sail_proof! { reveal(Prefix::material); reveal(Prefix::length); reveal(Prefix::exact); reveal(<RecordingHash as HashHost>::bytes); }
        let mut prefix = Prefix { host: RecordingHash { bytes: Vec::new() }, length: u64::MAX, exact: true };
        prefix.update(b"x");
        prefix.update(b"");
        sail_proof! { assert(prefix.material() == b"x"@); }
        !prefix.is_exact() && prefix.len() == u64::MAX
    }
}

sail_spec! {
    pub proof fn fields_prefix_injective(a: Seq<Seq<u8>>, a_tail: Seq<u8>, b: Seq<Seq<u8>>, b_tail: Seq<u8>)
        requires a.len() == b.len(),
            forall|i: int| 0 <= i < a.len() ==> #[trigger] a[i].len() <= u64::MAX,
            forall|i: int| 0 <= i < b.len() ==> #[trigger] b[i].len() <= u64::MAX,
            encode_fields(a) + a_tail == encode_fields(b) + b_tail
        ensures a == b, a_tail == b_tail
        decreases a.len()
    {
        reveal_with_fuel(encode_fields, 1);
        if a.len() > 0 {
            let at = a.subrange(1, a.len() as int);
            let bt = b.subrange(1, b.len() as int);
            assert(field(a.first()) + (encode_fields(at) + a_tail) =~= encode_fields(a) + a_tail);
            assert(field(b.first()) + (encode_fields(bt) + b_tail) =~= encode_fields(b) + b_tail);
            field_prefix_injective(a.first(), encode_fields(at) + a_tail, b.first(), encode_fields(bt) + b_tail);
            fields_prefix_injective(at, a_tail, bt, b_tail);
            assert(a == seq![a.first()] + at);
            assert(b == seq![b.first()] + bt);
        }
    }

    pub proof fn identity_prefix_injective(a: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>), a_tail: Seq<u8>,
        b: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>), b_tail: Seq<u8>)
        requires a.0.len() <= u64::MAX, a.1.len() <= u64::MAX, a.2.len() <= u64::MAX, a.3.len() <= u64::MAX,
            b.0.len() <= u64::MAX, b.1.len() <= u64::MAX, b.2.len() <= u64::MAX, b.3.len() <= u64::MAX,
            a.4.is_some() ==> a.4.unwrap().len() <= u64::MAX,
            b.4.is_some() ==> b.4.unwrap().len() <= u64::MAX,
            header(a.0, a.1, a.2, a.3, a.4) + a_tail == header(b.0, b.1, b.2, b.3, b.4) + b_tail
        ensures a == b, a_tail == b_tail
    {
        header_fields(a.0, a.1, a.2, a.3, a.4);
        header_fields(b.0, b.1, b.2, b.3, b.4);
        let af = identity_fields(a.0, a.1, a.2, a.3, a.4);
        let bf = identity_fields(b.0, b.1, b.2, b.3, b.4);
        identity_fields_shape(a); identity_fields_shape(b);
        reveal_with_fuel(encode_fields, 2);
        assert(field(af.first()) + (encode_fields(af.subrange(1, af.len() as int)) + a_tail) =~= encode_fields(af) + a_tail);
        assert(field(bf.first()) + (encode_fields(bf.subrange(1, bf.len() as int)) + b_tail) =~= encode_fields(bf) + b_tail);
        field_prefix_injective(af.first(), encode_fields(af.subrange(1, af.len() as int)) + a_tail,
            bf.first(), encode_fields(bf.subrange(1, bf.len() as int)) + b_tail);
        assert(af.subrange(1, af.len() as int).subrange(1, af.len() as int - 1) =~= af.subrange(2, af.len() as int));
        assert(bf.subrange(1, bf.len() as int).subrange(1, bf.len() as int - 1) =~= bf.subrange(2, bf.len() as int));
        assert(field(af[1]) + (encode_fields(af.subrange(2, af.len() as int)) + a_tail) =~= encode_fields(af.subrange(1, af.len() as int)) + a_tail);
        assert(field(bf[1]) + (encode_fields(bf.subrange(2, bf.len() as int)) + b_tail) =~= encode_fields(bf.subrange(1, bf.len() as int)) + b_tail);
        field_prefix_injective(af[1], encode_fields(af.subrange(2, af.len() as int)) + a_tail,
            bf[1], encode_fields(bf.subrange(2, bf.len() as int)) + b_tail);
        assert(a.4.is_some() == b.4.is_some());
        identity_fields_bounded(a); identity_fields_bounded(b);
        fields_prefix_injective(af, a_tail, bf, b_tail);
        identity_fields_tuple_injective(a, b);
    }

    /// Equal actual framed hash inputs imply exact solver/mode/raw-prefix equality.
    /// This is a byte-codec theorem, not an assumption that SHA-256 has no collisions.
    pub proof fn query_material_injective(a: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>), a_raw: Seq<u8>,
        b: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>), b_raw: Seq<u8>)
        requires a.0.len() <= u64::MAX, a.1.len() <= u64::MAX, a.2.len() <= u64::MAX, a.3.len() <= u64::MAX,
            b.0.len() <= u64::MAX, b.1.len() <= u64::MAX, b.2.len() <= u64::MAX, b.3.len() <= u64::MAX,
            a.4.is_some() ==> a.4.unwrap().len() <= u64::MAX,
            b.4.is_some() ==> b.4.unwrap().len() <= u64::MAX,
            a_raw.len() <= u64::MAX, b_raw.len() <= u64::MAX,
            end(header(a.0, a.1, a.2, a.3, a.4) + a_raw, a_raw.len() as u64)
                == end(header(b.0, b.1, b.2, b.3, b.4) + b_raw, b_raw.len() as u64)
        ensures a == b, a_raw == b_raw
    {
        let at = a_raw + b"\x00sail-key/prefix/v2/end"@ + vstd::bytes::spec_u64_to_le_bytes(a_raw.len() as u64);
        let bt = b_raw + b"\x00sail-key/prefix/v2/end"@ + vstd::bytes::spec_u64_to_le_bytes(b_raw.len() as u64);
        assert(header(a.0, a.1, a.2, a.3, a.4) + at =~= end(header(a.0, a.1, a.2, a.3, a.4) + a_raw, a_raw.len() as u64));
        assert(header(b.0, b.1, b.2, b.3, b.4) + bt =~= end(header(b.0, b.1, b.2, b.3, b.4) + b_raw, b_raw.len() as u64));
        identity_prefix_injective(a, at, b, bt);
        vstd::bytes::spec_u64_to_le_bytes_to_open(a_raw.len() as u64);
        vstd::bytes::spec_u64_to_le_bytes_to_open(b_raw.len() as u64);
        assert(a_raw.len() == b_raw.len());
        assert(at.take(a_raw.len() as int) == a_raw);
        assert(bt.take(b_raw.len() as int) == b_raw);
    }
}

sail_verified! {
    /// The real owned raw token feeds once; replaying it refuses without hashing it again.
    pub fn owned_feed_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        sail_proof! { reveal(Prefix::material); reveal(<RecordingHash as HashHost>::bytes); }
        let mut raw = crate::query::RawTranscript::empty();
        raw.append(b"(check-sat)");
        let mut prefix = Prefix::new(RecordingHash { bytes: Vec::new() }, b"z3", b"4", b"binary", b"platform", None);
        sail_proof! {
            assert(crate::query::trivia_after(Seq::<u8>::empty(), false) == Some(false));
            assert(raw.view().subrange(0, 0) == Seq::<u8>::empty());
            assert(raw.active()); assert(raw.fed_view() == 0);
            assert(raw.view() == b"(check-sat)"@);
            assert(raw.view().subrange(0, b"(check-sat)"@.len() as int) == b"(check-sat)"@);
        }
        let proposal = raw.command(0, b"(check-sat)");
        sail_proof! { assert(proposal.is_some()); }
        match proposal {
            Some(feed) => {
                let accepted = prefix.feed(&feed);
                sail_proof! { assert(accepted); }
                let repeated = prefix.feed(&feed);
                sail_proof! { assert(!repeated); }
                sail_proof! {
                    assert(prefix.material() == header(b"z3"@, b"4"@, b"binary"@, b"platform"@, None) + b"(check-sat)"@);
                }
                accepted && !repeated && !prefix.is_exact()
            }
            None => false,
        }
    }
}

sail_spec! {
    pub proof fn identity_fields_bounded(a: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>))
        requires a.0.len() <= u64::MAX, a.1.len() <= u64::MAX, a.2.len() <= u64::MAX, a.3.len() <= u64::MAX,
            a.4.is_some() ==> a.4.unwrap().len() <= u64::MAX
        ensures forall|i: int| 0 <= i < identity_fields(a.0,a.1,a.2,a.3,a.4).len()
            ==> #[trigger] identity_fields(a.0,a.1,a.2,a.3,a.4)[i].len() <= u64::MAX
    {
        reveal(identity_fields);
        let af = identity_fields(a.0,a.1,a.2,a.3,a.4);
        assert forall|i: int| 0 <= i < af.len() implies #[trigger] af[i].len() <= u64::MAX by {
            if a.4.is_some() { assert(af.len() == 7); } else { assert(af.len() == 6); }
        }
    }
    pub proof fn identity_fields_tuple_injective(a: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>),
        b: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>))
        requires identity_fields(a.0,a.1,a.2,a.3,a.4) == identity_fields(b.0,b.1,b.2,b.3,b.4)
        ensures a == b
    {
        reveal(identity_fields);
        let af = identity_fields(a.0,a.1,a.2,a.3,a.4);
        let bf = identity_fields(b.0,b.1,b.2,b.3,b.4);
        if a.4.is_some() { assert(af.len() == 7); } else { assert(af.len() == 6); }
        if b.4.is_some() { assert(bf.len() == 7); } else { assert(bf.len() == 6); }
        assert(a.4.is_some() == b.4.is_some());
        if a.4.is_some() {
            assert(af[2] == a.4.unwrap() && bf[2] == b.4.unwrap());
            assert(af[3] == a.0 && af[4] == a.1 && af[5] == a.2 && af[6] == a.3);
            assert(bf[3] == b.0 && bf[4] == b.1 && bf[5] == b.2 && bf[6] == b.3);
        } else {
            assert(af[2] == a.0 && af[3] == a.1 && af[4] == a.2 && af[5] == a.3);
            assert(bf[2] == b.0 && bf[3] == b.1 && bf[4] == b.2 && bf[5] == b.3);
        }
    }
}

sail_spec! {
    pub proof fn identity_fields_shape(a: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>))
        ensures identity_fields(a.0,a.1,a.2,a.3,a.4).len() == (if a.4.is_some() { 7int } else { 6int }),
            identity_fields(a.0,a.1,a.2,a.3,a.4)[0] == b"sail-key/prefix/v2"@,
            identity_fields(a.0,a.1,a.2,a.3,a.4)[1] == (if a.4.is_some() { b"mode:opt-in"@ } else { b"mode:default"@ })
    { reveal(identity_fields); }
}
