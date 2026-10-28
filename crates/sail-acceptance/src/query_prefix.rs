//! Same-owner raw input and incremental key state; external feed mixing is impossible.
use crate::key_prefix::{HashHost, Prefix};
use crate::query_raw::{RawTranscript, framed_command};
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// One immutable command observation from the actual owned raw/hash transition.
    pub struct FramedCommand<'a, 'b, Host> {
        head: &'a [u8],
        endpoint: usize,
        prefix: &'b Prefix<Host>,
    }
    /// Joint ownership of the only raw transcript and the key state it drives.
    pub struct QueryPrefix<Host> {
        raw: RawTranscript,
        prefix: Prefix<Host>,
    }
}

sail_code! {
    impl<'a, Host: HashHost> FramedCommand<'a, '_, Host> {
        sail_impl_spec! {
            pub closed spec fn head_view(&self) -> Seq<u8> { self.head@ }
            pub closed spec fn endpoint(&self) -> nat { self.endpoint as nat }
            pub closed spec fn key_view(&self) -> [u8; 32] { Host::digest(crate::key_prefix::end(self.prefix.material(),self.endpoint as u64)) }
            pub closed spec fn coherent(&self) -> bool { self.prefix.length() == self.endpoint }
        }
        sail_method! {
            /// Borrows the exact accepted command head.
            pub fn head(&self) -> (out: &'a [u8])
                requires (true) ensures (out@ == self.head_view())
            { self.head }
        }
        sail_method! {
            /// Computes output metadata from this accepted command's own exact head.
            pub fn policy(&self) -> (out: crate::query_policy::ResponsePolicy)
                requires (true)
                ensures ((out is One) == crate::query_policy::one_head(self.head_view()),
                    (out is None) == (!crate::query_policy::one_head(self.head_view()) && crate::query_policy::silent_head(self.head_view())),
                    (out is Unknown) == (!crate::query_policy::one_head(self.head_view()) && !crate::query_policy::silent_head(self.head_view())))
            { crate::query_policy::response_policy(self.head) }
        }
        sail_method! {
            /// The exact raw endpoint whose state produced this key.
            pub fn end(&self) -> (out: usize)
                requires (true) ensures (out == self.endpoint())
            { self.endpoint }
        }
        sail_method! {
            /// Copies the digest produced at this exact transition.
            pub fn key(&self) -> (out: [u8; 32])
                requires (self.coherent()) ensures (out == self.key_view())
            { self.prefix.key() }
        }
    }
    impl<Host: HashHost> QueryPrefix<Host> {
        sail_impl_spec! {
            pub closed spec fn view(&self) -> Seq<u8> { self.raw.view() }
            pub closed spec fn fed(&self) -> nat { self.raw.fed_view() }
            pub closed spec fn material(&self) -> Seq<u8> { self.prefix.material() }
            pub closed spec fn active(&self) -> bool { self.raw.active() && self.prefix.exact() }
            pub closed spec fn coherent(&self) -> bool {
                self.fed() <= self.view().len() && self.prefix.exact() && self.prefix.length() == self.fed()
            }
            pub open spec fn matches(&self, binding: (Seq<u8>, Seq<u8>, Seq<u8>, Seq<u8>, Option<Seq<u8>>)) -> bool {
                self.coherent() && self.material() == crate::key_prefix::header(binding.0,binding.1,binding.2,binding.3,binding.4)
                    + self.view().take(self.fed() as int)
            }
        }
        sail_method! {
            /// Starts an empty raw stream around a freshly initialized key state.
            pub fn new(prefix: Prefix<Host>) -> (out: Self)
                requires (prefix.exact(), prefix.length() == 0)
                ensures (out.coherent(), out.active(), out.view().len() == 0, out.fed() == 0,
                    out.material() == prefix.material(),
                    forall|binding: (Seq<u8>,Seq<u8>,Seq<u8>,Seq<u8>,Option<Seq<u8>>)|
                        prefix.material() == crate::key_prefix::header(binding.0,binding.1,binding.2,binding.3,binding.4) ==> out.matches(binding))
            { Self { raw: RawTranscript::empty(), prefix } }
        }
        sail_method! {
            /// Appends every original byte without granting any command/key interpretation.
            pub fn append(&mut self, bytes: &[u8]) -> (out: ())
                requires (old(self).coherent())
                ensures (final(self).coherent(), final(self).active() == old(self).active(),
                    final(self).view() == old(self).view() + bytes@, final(self).fed() == old(self).fed(),
                    final(self).material() == old(self).material(),
                    forall|binding: (Seq<u8>,Seq<u8>,Seq<u8>,Seq<u8>,Option<Seq<u8>>)|
                        old(self).matches(binding) ==> final(self).matches(binding))
            {
                self.raw.append(bytes);
                sail_proof! {
                    assert(self.view().take(self.fed() as int) =~= old(self).view().take(old(self).fed() as int));
                }
            }
        }
        sail_method! {
            /// Exposes only this owner's exact replay bytes.
            pub fn bytes(&self) -> (out: &[u8])
                requires (self.coherent()) ensures (out@ == self.view())
            { self.raw.bytes() }
        }
        sail_method! {
            /// Refuses interpretation permanently, retaining the complete original input.
            pub fn refuse(&mut self) -> (out: ())
                requires (old(self).coherent())
                ensures (final(self).coherent(), !final(self).active(),
                    final(self).view() == old(self).view(), final(self).fed() == old(self).fed(),
                    final(self).material() == old(self).material(),
                    forall|binding: (Seq<u8>,Seq<u8>,Seq<u8>,Seq<u8>,Option<Seq<u8>>)|
                        old(self).matches(binding) ==> final(self).matches(binding))
            { self.raw.refuse(); }
        }
        sail_method! {
            /// Whether same-owner cache interpretation has never refused.
            pub fn valid(&self) -> (out: bool)
                requires (self.coherent()) ensures (out == self.active())
            { self.raw.valid() && self.prefix.is_exact() }
        }
        sail_method! {
            /// Derives a command key only from this object's own validated contiguous raw interval.
            pub fn command<'a, 'b>(&'b mut self, offset: usize, command: &'a [u8]) -> (out: Option<FramedCommand<'a, 'b, Host>>)
                requires (old(self).coherent())
                ensures (final(self).coherent(), final(self).view() == old(self).view(),
                    forall|binding: (Seq<u8>,Seq<u8>,Seq<u8>,Seq<u8>,Option<Seq<u8>>)|
                        old(self).matches(binding) ==> final(self).matches(binding),
                    out.is_some() ==> out.unwrap().coherent() && final(self).active() && crate::query_raw::single_list(command@, true)
                        && out.unwrap().endpoint() == final(self).fed()
                        && final(self).fed() == offset as nat + command@.len()
                        && final(self).view().subrange(offset as int, final(self).fed() as int) == command@
                        && out.unwrap().key_view() == Host::digest(crate::key_prefix::end(final(self).material(), final(self).fed() as u64))
                        && exists|start: int, end: int| crate::query_raw::head_span(command@,start,end)
                            && out.unwrap().head_view() == command@.subrange(start,end),
                    out.is_none() ==> !final(self).active() && final(self).fed() == old(self).fed(),
                    old(self).active() && (exists|start: int,end: int| crate::query_raw::head_span(command@,start,end))
                        && old(self).fed() <= offset && offset as nat + command@.len() <= old(self).view().len()
                        && old(self).view().subrange(offset as int,offset as int + command@.len() as int) == command@
                        && crate::query_raw::trivia_after(old(self).view().subrange(old(self).fed() as int,offset as int),false) == Some(false)
                        ==> out.is_some(),
                    old(self).active() && crate::query_raw::single_list(command@,true) && crate::query_raw::symbol_first(command@[1])
                        && old(self).fed() <= offset && offset as nat + command@.len() <= old(self).view().len()
                        && old(self).view().subrange(offset as int,offset as int + command@.len() as int) == command@
                        && crate::query_raw::trivia_after(old(self).view().subrange(old(self).fed() as int,offset as int),false) == Some(false)
                        ==> out.is_some())
            {
                if !self.prefix.is_exact() || self.prefix.len() != self.raw.fed() as u64 {
                    self.raw.refuse(); return None;
                }
                let accepted = framed_command(&mut self.raw, offset, command);
                match accepted {
                    Some((feed, head)) => {
                        let endpoint = feed.end();
                        if !self.prefix.feed(&feed) { return None; }
                        if self.prefix.len() != endpoint as u64 { self.raw.refuse(); return None; }
                        sail_proof! {
                            assert(old(self).view().take(old(self).fed() as int)
                                + old(self).view().subrange(old(self).fed() as int, endpoint as int)
                                =~= old(self).view().take(endpoint as int));
                            assert forall|binding: (Seq<u8>,Seq<u8>,Seq<u8>,Seq<u8>,Option<Seq<u8>>)|
                                old(self).matches(binding) implies self.matches(binding) by {
                                assert((crate::key_prefix::header(binding.0,binding.1,binding.2,binding.3,binding.4)
                                    + old(self).view().take(old(self).fed() as int))
                                    + old(self).view().subrange(old(self).fed() as int, endpoint as int)
                                    =~= crate::key_prefix::header(binding.0,binding.1,binding.2,binding.3,binding.4)
                                    + self.view().take(endpoint as int));
                            }
                        }
                        Some(FramedCommand { head, endpoint, prefix: &self.prefix })
                    }
                    None => None,
                }
            }
        }
    }
}

sail_code! {
    impl<Host: HashHost> QueryPrefix<Host> {
        sail_method! {
            /// Number of this owner's bytes already represented by its hash state.
            pub fn len(&self) -> (out: u64)
                requires (self.coherent()) ensures (out == self.fed())
            { self.prefix.len() }
        }
        sail_method! {
            /// Whether no raw byte has yet entered the incremental state.
            pub fn is_empty(&self) -> (out: bool)
                requires (self.coherent()) ensures (out == (self.fed() == 0))
            { self.prefix.is_empty() }
        }
        sail_method! {
            /// Whether the unfed original suffix consists only of trivia.
            pub fn trailing_trivia(&self) -> (out: bool)
                requires (self.coherent())
                ensures (out == (crate::query_raw::trivia_after(self.view().subrange(self.fed() as int,self.view().len() as int),false) == Some(false)
                    || crate::query_raw::trivia_after(self.view().subrange(self.fed() as int,self.view().len() as int),false) == Some(true)))
            { self.raw.trailing_trivia() }
        }
        sail_method! {
            /// Hashes this owner's remaining original bytes; this does not assert physical EOF or clean solver exit.
            pub fn hash_tail(&mut self) -> (accepted: bool)
                requires (old(self).coherent())
                ensures (final(self).coherent(), final(self).view() == old(self).view(),
                    accepted == old(self).active(), final(self).active() == old(self).active(),
                    accepted ==> final(self).fed() == final(self).view().len(),
                    !accepted ==> final(self).fed() == old(self).fed(),
                    forall|binding: (Seq<u8>,Seq<u8>,Seq<u8>,Seq<u8>,Option<Seq<u8>>)|
                        old(self).matches(binding) ==> final(self).matches(binding))
            {
                if !self.prefix.is_exact() || self.prefix.len() != self.raw.fed() as u64 {
                    self.raw.refuse(); return false;
                }
                let candidate = self.raw.tail();
                match candidate {
                    Some(feed) => {
                        let _endpoint = feed.end();
                        let accepted = self.prefix.feed(&feed);
                        sail_proof! {
                            assert(accepted);
                            assert(old(self).view().take(old(self).fed() as int)
                                + old(self).view().subrange(old(self).fed() as int, _endpoint as int)
                                =~= old(self).view().take(_endpoint as int));
                            assert forall|binding: (Seq<u8>,Seq<u8>,Seq<u8>,Seq<u8>,Option<Seq<u8>>)|
                                old(self).matches(binding) implies self.matches(binding) by {
                                assert((crate::key_prefix::header(binding.0,binding.1,binding.2,binding.3,binding.4)
                                    + old(self).view().take(old(self).fed() as int))
                                    + old(self).view().subrange(old(self).fed() as int, _endpoint as int)
                                    =~= crate::key_prefix::header(binding.0,binding.1,binding.2,binding.3,binding.4)
                                    + self.view().take(_endpoint as int));
                            }
                        }
                        accepted
                    }
                    None => false,
                }
            }
        }
        sail_method! {
            /// The current same-owner material's digest, without an origin or clean-run assertion.
            pub fn key(&self) -> (out: [u8; 32])
                requires (self.coherent())
                ensures (out == Host::digest(crate::key_prefix::end(self.material(),self.fed() as u64)))
            { self.prefix.key() }
        }
        sail_method! {
            /// The unchanged silence domain candidate; admission still requires complete clean publication.
            pub fn silence_key(&self) -> (out: [u8; 32])
                requires (self.coherent())
                ensures (out == Host::digest(crate::key_prefix::field(b"sail-key/session-silence/v1"@)
                    + crate::key_prefix::field(Host::digest(crate::key_prefix::end(self.material(),self.fed() as u64))@)))
            { self.prefix.silence_key() }
        }
    }
}

sail_spec! {
    pub open spec fn flat_byte(byte: u8) -> bool {
        byte != b'(' && byte != b')' && byte != b'"' && byte != b'|' && byte != b';'
    }
    pub proof fn flat_contents_prefix(contents: Seq<u8>)
        requires forall|i: int| 0 <= i < contents.len() ==> #[trigger] flat_byte(contents[i])
        ensures crate::query_raw::frame_scan(seq![b'('] + contents,true) == (1int,crate::query_raw::FrameLex::Plain)
        decreases contents.len()
    {
        let framed = seq![b'('] + contents;
        if contents.len() > 0 {
            flat_contents_prefix(contents.drop_last());
            assert(framed.drop_last() =~= seq![b'('] + contents.drop_last());
            assert(framed.last() == contents[contents.len() - 1]);
            assert(flat_byte(framed.last()));
            reveal_with_fuel(crate::query_raw::frame_scan,1);
        } else {
            assert(framed =~= seq![b'(']);
            reveal_with_fuel(crate::query_raw::frame_scan,2);
        }
    }
    pub proof fn flat_list_frame(contents: Seq<u8>)
        requires forall|i: int| 0 <= i < contents.len() ==> #[trigger] flat_byte(contents[i])
        ensures crate::query_raw::single_list(seq![b'('] + contents + seq![b')'],true)
    {
        let full = seq![b'('] + contents + seq![b')'];
        flat_contents_prefix(contents);
        assert(full.drop_last() =~= seq![b'('] + contents);
        reveal_with_fuel(crate::query_raw::frame_scan,1);
        assert forall|i: int| 1 <= i < full.len() implies crate::query_raw::frame_scan(full.take(i),true).0 > 0 by {
            let prefix = contents.take(i - 1);
            assert(full.take(i) =~= seq![b'('] + prefix);
            flat_contents_prefix(prefix);
        }
    }
}
