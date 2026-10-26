//! Source-shared queue and replay operations used by the cached solver shim.
//!
//! The private queues own the actual expectation and reply payloads. The
//! verifier checks these bodies; it does not replace them with trusted specs.

sail_spec! { use vstd::prelude::*; }

sail_code! {
    /// A FIFO represented by two stacks, with amortized constant-time removal.
    pub struct Fifo<T> {
        front: Vec<T>,
        back: Vec<T>,
    }
}

sail_code! {
    impl<T> Fifo<T> {
        sail_impl_spec! {
            /// Ordered payloads still waiting in this queue.
            pub closed spec fn contents(&self) -> Seq<T> {
                self.front@.reverse() + self.back@
            }
        }

        sail_method! {
            /// Creates an empty queue.
            pub fn empty() -> (result: Self)
                requires (true)
                ensures (result.contents() == Seq::<T>::empty())
            {
                Self { front: Vec::new(), back: Vec::new() }
            }
        }

        sail_method! {
            /// Adds a payload at the end of the queue.
            pub fn push(&mut self, item: T) -> (result: ())
                requires (true)
                ensures (final(self).contents() == old(self).contents().push(item))
            {
                self.back.push(item);
            }
        }

        sail_method! {
            /// Whether there are no waiting payloads.
            pub fn is_empty(&self) -> (result: bool)
                requires (true)
                ensures (result == (self.contents().len() == 0))
            {
                self.front.is_empty() && self.back.is_empty()
            }
        }

        sail_method! {
            /// Removes exactly the oldest payload, leaving an empty queue unchanged.
            pub fn pop(&mut self) -> (result: Option<T>)
                requires (true)
                ensures (
                    result.is_some() == (old(self).contents().len() > 0),
                    result.is_some() ==> result.unwrap() == old(self).contents().first(),
                    final(self).contents() == if old(self).contents().len() == 0 {
                        old(self).contents()
                    } else {
                        old(self).contents().drop_first()
                    }
                )
            {
                if self.front.is_empty() {
                    sail_while! {
                        (!self.back.is_empty())
                        invariant (self.back@ + self.front@.reverse() == old(self).contents())
                        decreases (self.back.len())
                        {
                            let item = self.back.pop();
                            if let Some(item) = item {
                                self.front.push(item);
                                sail_proof! {
                                    assert(self.back@ + self.front@.reverse() =~= old(self).contents());
                                }
                            }
                        }
                    }
                }
                self.front.pop()
            }
        }
    }
}

sail_code! {
    /// A pair removed together from the oldest waiting expectation and reply.
    pub struct BoundPair<E, R> {
        expectation: E,
        reply: R,
    }
}

sail_code! {
    impl<E, R> BoundPair<E, R> {
        sail_impl_spec! {
            /// Exact expectation payload associated with this reply.
            pub closed spec fn expected(&self) -> E { self.expectation }
            /// Exact reply payload associated with this expectation.
            pub closed spec fn response(&self) -> R { self.reply }
        }

        sail_method! {
            /// Borrows the original expectation while retaining its paired reply.
            pub fn expectation(&self) -> (result: &E)
                requires (true)
                ensures (*result == self.expected())
            { &self.expectation }
        }

        sail_method! {
            /// Borrows the original reply while retaining its paired expectation.
            pub fn reply(&self) -> (result: &R)
                requires (true)
                ensures (*result == self.response())
            { &self.reply }
        }

        sail_method! {
            /// Consumes the pair without changing either owned payload.
            pub fn into_parts(self) -> (result: (E, R))
                requires (true)
                ensures (result.0 == self.expected(), result.1 == self.response())
            { (self.expectation, self.reply) }
        }
    }
}

sail_code! {
    /// Correlates two asynchronous streams by their complete arrival order.
    ///
    /// A partial arrival never consumes its counterpart. Stopping is permanent.
    pub struct CorrelatedQueue<E, R> {
        expects: Fifo<E>,
        replies: Fifo<R>,
        stopped: bool,
    }
}

sail_code! {
    impl<E, R> CorrelatedQueue<E, R> {
        sail_impl_spec! {
            /// Outstanding expectations in arrival order.
            pub closed spec fn expectations(&self) -> Seq<E> { self.expects.contents() }
            /// Outstanding replies in arrival order.
            pub closed spec fn responses(&self) -> Seq<R> { self.replies.contents() }
            /// Whether correlation has been permanently disabled.
            pub closed spec fn halted(&self) -> bool { self.stopped }
        }

        sail_method! {
            /// Creates an empty active correlator.
            pub fn empty() -> (result: Self)
                requires (true)
                ensures (
                    result.expectations() == Seq::<E>::empty(),
                    result.responses() == Seq::<R>::empty(),
                    !result.halted()
                )
            { Self { expects: Fifo::empty(), replies: Fifo::empty(), stopped: false } }
        }

        sail_method! {
            /// Queues the actual expectation payload.
            pub fn expect(&mut self, expectation: E) -> (result: ())
                requires (true)
                ensures (
                    final(self).expectations() == old(self).expectations().push(expectation),
                    final(self).responses() == old(self).responses(),
                    final(self).halted() == old(self).halted()
                )
            { self.expects.push(expectation); }
        }

        sail_method! {
            /// Queues the actual reply payload.
            pub fn reply(&mut self, reply: R) -> (result: ())
                requires (true)
                ensures (
                    final(self).responses() == old(self).responses().push(reply),
                    final(self).expectations() == old(self).expectations(),
                    final(self).halted() == old(self).halted()
                )
            { self.replies.push(reply); }
        }

        sail_method! {
            /// Permanently disables pairing without changing the queued payloads.
            pub fn stop(&mut self) -> (result: ())
                requires (true)
                ensures (
                    final(self).halted(),
                    final(self).expectations() == old(self).expectations(),
                    final(self).responses() == old(self).responses()
                )
            { self.stopped = true; }
        }

        sail_method! {
            /// Whether pairing has been permanently disabled.
            pub fn is_stopped(&self) -> (result: bool)
                requires (true)
                ensures (result == self.halted())
            { self.stopped }
        }

        sail_method! {
            /// Whether both streams have no unmatched payloads.
            pub fn is_empty(&self) -> (result: bool)
                requires (true)
                ensures (result == (self.expectations().len() == 0 && self.responses().len() == 0))
            { self.expects.is_empty() && self.replies.is_empty() }
        }

        sail_method! {
            /// Removes both oldest payloads together, or leaves both queues unchanged.
            pub fn take_pair(&mut self) -> (result: Option<BoundPair<E, R>>)
                requires (true)
                ensures (
                    result.is_some() == (!old(self).halted()
                        && old(self).expectations().len() > 0 && old(self).responses().len() > 0),
                    result.is_some() ==> (
                        result.unwrap().expected() == old(self).expectations().first()
                        && result.unwrap().response() == old(self).responses().first()
                        && final(self).expectations() == old(self).expectations().drop_first()
                        && final(self).responses() == old(self).responses().drop_first()
                    ),
                    result.is_none() ==> (
                        final(self).expectations() == old(self).expectations()
                        && final(self).responses() == old(self).responses()
                    ),
                    final(self).halted() == old(self).halted()
                )
            {
                if self.stopped || self.expects.is_empty() || self.replies.is_empty() {
                    return None;
                }
                let expectation = self.expects.pop();
                let reply = self.replies.pop();
                match (expectation, reply) {
                    (Some(expectation), Some(reply)) => Some(BoundPair { expectation, reply }),
                    _ => None,
                }
            }
        }
    }
}

sail_code! {
    /// Counts the already-served prefix still to suppress during solver replay.
    pub struct ReplayCursor {
        remaining: u64,
    }
}

sail_code! {
    impl ReplayCursor {
        sail_impl_spec! {
            /// Number of leading solver responses still to suppress.
            pub closed spec fn left(&self) -> nat { self.remaining as nat }
        }

        sail_method! {
            /// Suppresses the first `served` complete response frames.
            pub fn new(served: u64) -> (result: Self)
                requires (true)
                ensures (result.left() == served as nat)
            { Self { remaining: served } }
        }

        sail_method! {
            /// Whether the next response belongs to the fresh suffix.
            pub fn forwarding(&self) -> (result: bool)
                requires (true)
                ensures (result == (self.left() == 0))
            { self.remaining == 0 }
        }

        sail_method! {
            /// Consumes one complete response frame and returns its forwarding decision.
            /// The counter never wraps, even for an arbitrarily long fresh suffix.
            pub fn advance(&mut self) -> (result: bool)
                requires (true)
                ensures (
                    result == (old(self).left() == 0),
                    final(self).left() == if old(self).left() == 0 { 0nat }
                        else { (old(self).left() - 1) as nat }
                )
            {
                if self.remaining == 0 {
                    true
                } else {
                    self.remaining -= 1;
                    false
                }
            }
        }
    }
}

sail_code! {
    /// Observations required before a complete solver session may publish entries.
    ///
    /// These are observations made by the adapter; the core does not establish
    /// operating-system facts such as pipe EOF or process exit identity.
    // Independent observations form a conjunction, rather than alternative states.
    #[allow(clippy::struct_excessive_bools)]
    pub struct PublicationGate {
        /// The input producer closed its stream and queued its expectations.
        pub input_closed: bool,
        /// No permanent correlation fault occurred.
        pub pairing_active: bool,
        /// Both expectation and response queues have been fully consumed.
        pub queues_empty: bool,
        /// The solver exited normally with a successful status.
        pub normal_exit: bool,
        /// Both diagnostic EOF and absence of diagnostic bytes were observed.
        pub quiet: bool,
        /// Forwarding and response input reads completed without error.
        pub forwarding_ok: bool,
    }
}

sail_code! {
    impl PublicationGate {
        sail_impl_spec! {
            /// Complete clean publication observations supplied by the adapter.
            pub open spec fn complete(&self) -> bool {
                self.input_closed && self.pairing_active && self.queues_empty
                    && self.normal_exit && self.quiet && self.forwarding_ok
            }
        }

        sail_method! {
            /// Admits publication only when every required observation holds.
            pub fn allows(&self) -> (result: bool)
                requires (true)
                ensures (result == self.complete())
            {
                self.input_closed && self.pairing_active && self.queues_empty
                    && self.normal_exit && self.quiet && self.forwarding_ok
            }
        }
    }
}

sail_code! {
    /// Owns unpublished payloads until the producer's complete-session gate opens.
    pub struct PublicationBatch<T> {
        entries: Vec<T>,
        bytes: usize,
    }
}

sail_code! {
    impl<T> PublicationBatch<T> {
        sail_impl_spec! {
            /// Exact unpublished payloads in insertion order.
            pub closed spec fn contents(&self) -> Seq<T> { self.entries@ }
            /// Sum of the adapter-supplied byte costs for accepted payloads.
            pub closed spec fn cost(&self) -> nat { self.bytes as nat }
        }

        sail_method! {
            /// Creates an empty unpublished batch.
            pub fn empty() -> (result: Self)
                requires (true)
                ensures (result.contents() == Seq::<T>::empty(), result.cost() == 0)
            { Self { entries: Vec::new(), bytes: 0 } }
        }

        sail_method! {
            /// Number of exact buffered payloads, for a complete ordering certificate.
            pub fn entries_count(&self) -> (count: usize)
                requires (true)
                ensures (count == self.contents().len())
            { self.entries.len() }
        }

        sail_method! {
            /// Borrows the exact original batch for checks before owned release.
            pub fn entries(&self) -> (entries: &[T])
                requires (true) ensures (entries@ == self.contents())
            { &self.entries }
        }

        sail_method! {
            /// Appends a payload only when its entire byte cost fits the bound.
            /// Overflow and oversized payloads leave the batch unchanged.
            pub fn push(&mut self, entry: T, cost: usize, limit: usize) -> (result: bool)
                requires (true)
                ensures (
                    result == (old(self).cost() + cost as nat <= limit as nat),
                    result ==> (final(self).contents() == old(self).contents().push(entry)
                        && final(self).cost() == old(self).cost() + cost as nat),
                    !result ==> (final(self).contents() == old(self).contents()
                        && final(self).cost() == old(self).cost())
                )
            {
                let Some(bytes) = self.bytes.checked_add(cost) else { return false; };
                if bytes > limit { return false; }
                self.entries.push(entry);
                self.bytes = bytes;
                true
            }
        }

        sail_method! {
            /// Releases exactly the buffered payloads, or releases no payloads.
            pub fn release(self, allowed: bool) -> (result: Option<Vec<T>>)
                requires (true)
                ensures (
                    result.is_some() == allowed,
                    result.is_some() ==> result.unwrap()@ == self.contents()
                )
            {
                if allowed { Some(self.entries) } else { None }
            }
        }
    }
}

sail_code! {
    /// The first failed disk-envelope integrity check.
    pub enum RecoveryRejection {
        /// The entry names another storage format.
        WrongVersion,
        /// The entry names a different content-addressed key.
        KeyMismatch,
        /// The recorded payload digest differs from the digest just computed.
        HashMismatch,
    }

    /// Owns a decoded envelope and its exact payload during disk admission.
    ///
    /// Decoding and digest computation happen outside this core. Passing these
    /// checks establishes integrity and key binding, not a historical solver origin.
    pub struct RecoveryRecord<M> {
        metadata: M,
        reply: Vec<u8>,
        version: u32,
        key: Vec<u8>,
        digest: Vec<u8>,
    }
}

sail_code! {
    impl<M> RecoveryRecord<M> {
        sail_impl_spec! {
            /// Metadata owned by this decoded envelope.
            pub closed spec fn metadata(&self) -> M { self.metadata }
            /// Exact reply payload owned by this envelope.
            pub closed spec fn payload(&self) -> Seq<u8> { self.reply@ }
            /// Decoded storage version supplied by the adapter.
            pub closed spec fn version(&self) -> u32 { self.version }
            /// Decoded key bytes supplied by the adapter.
            pub closed spec fn key(&self) -> Seq<u8> { self.key@ }
            /// Decoded digest bytes supplied by the adapter.
            pub closed spec fn digest(&self) -> Seq<u8> { self.digest@ }
        }

        sail_method! {
            /// Captures the actual decoded record and reply before admission.
            pub fn new(metadata: M, reply: Vec<u8>, version: u32, key: Vec<u8>, digest: Vec<u8>)
                -> (result: Self)
                requires (true)
                ensures (
                    result.metadata() == metadata, result.payload() == reply@,
                    result.version() == version, result.key() == key@, result.digest() == digest@
                )
            { Self { metadata, reply, version, key, digest } }
        }

        sail_method! {
            /// Admits exactly the original metadata and bytes after all checks.
            /// The supplied digest must be computed from this record's payload.
            ///
            /// # Errors
            /// Returns the first mismatching version, key or payload digest.
            pub fn recover(self, expected_version: u32, expected_key: &[u8], computed_digest: &[u8])
                -> (result: Result<(M, Vec<u8>), RecoveryRejection>)
                requires (true)
                ensures (
                    result.is_ok() == (self.version() == expected_version
                        && self.key() == expected_key@ && self.digest() == computed_digest@),
                    match result { Ok((metadata, reply)) => metadata == self.metadata()
                        && reply@ == self.payload(), Err(_) => true },
                    self.version() != expected_version ==> result == Err(RecoveryRejection::WrongVersion),
                    self.version() == expected_version && self.key() != expected_key@
                        ==> result == Err(RecoveryRejection::KeyMismatch),
                    self.version() == expected_version && self.key() == expected_key@
                        && self.digest() != computed_digest@ ==> result == Err(RecoveryRejection::HashMismatch)
                )
            {
                if self.version != expected_version {
                    return Err(RecoveryRejection::WrongVersion);
                }
                let recorded_key = self.key.as_slice();
                if recorded_key != expected_key {
                    return Err(RecoveryRejection::KeyMismatch);
                }
                sail_proof! { assert(self.key@ == expected_key@); }
                let recorded_digest = self.digest.as_slice();
                if recorded_digest != computed_digest {
                    return Err(RecoveryRejection::HashMismatch);
                }
                sail_proof! { assert(self.digest@ == computed_digest@); }
                Ok((self.metadata, self.reply))
            }
        }
    }
}

sail_spec! {
    /// Lexical state after a prefix containing only whitespace and line comments.
    /// None means that a nontrivia byte was encountered outside a comment.
    pub open spec fn trivia_after(bytes: Seq<u8>, comment: bool) -> Option<bool>
        decreases bytes.len(),
    {
        if bytes.len() == 0 { Some(comment) }
        else if comment {
            trivia_after(bytes.drop_first(), bytes[0] != b'\n' && bytes[0] != b'\r')
        } else if bytes[0] == b' ' || bytes[0] == b'\t' || bytes[0] == b'\n' || bytes[0] == b'\r' {
            trivia_after(bytes.drop_first(), false)
        } else if bytes[0] == b';' { trivia_after(bytes.drop_first(), true) }
        else { None }
    }
}

sail_verified! {
    /// Checks actual inter-item bytes, including complete semicolon comments.
    /// At EOF an unfinished line comment is permitted; a command cannot begin in one.
    pub fn trivia(bytes: &[u8], allow_open_comment: bool) -> (accepted: bool)
        requires (true)
        ensures (accepted == (trivia_after(bytes@, false) == Some(false)
            || allow_open_comment && trivia_after(bytes@, false) == Some(true)))
    {
        let mut i = 0;
        let mut comment = false;
        sail_proof! { assert(bytes@.subrange(0, bytes.len() as int) =~= bytes@); }
        sail_while! { (i < bytes.len())
            invariant (i <= bytes.len(),
                trivia_after(bytes@, false) == trivia_after(bytes@.subrange(i as int, bytes.len() as int), comment))
            decreases (bytes.len() - i)
            {
                sail_proof! {
                    assert(bytes@.subrange(i as int, bytes.len() as int).drop_first()
                        =~= bytes@.subrange(i as int + 1, bytes.len() as int));
                    reveal_with_fuel(trivia_after, 1);
                }
                let byte = bytes[i];
                if comment {
                    comment = byte != b'\n' && byte != b'\r';
                } else if byte == b';' {
                    comment = true;
                } else if byte != b' ' && byte != b'\t' && byte != b'\n' && byte != b'\r' {
                    return false;
                }
                i += 1;
            }
        }
        allow_open_comment || !comment
    }
}

sail_code! {
    /// Exact append-only solver input and the portion already fed to key derivation.
    /// Parser output is a candidate interval, never authority over different bytes.
    pub struct RawTranscript {
        bytes: Vec<u8>,
        fed: usize,
        valid: bool,
    }

    /// Borrow of the actual next contiguous bytes supplied to prefix hashing.
    pub struct PrefixFeed<'a> {
        bytes: &'a [u8],
        start: usize,
        end: usize,
    }
}

sail_code! {
    impl<'a> PrefixFeed<'a> {
        sail_impl_spec! {
            /// Exact original raw bytes of this feed interval.
            pub closed spec fn view(&self) -> Seq<u8> { self.bytes@ }
            /// First raw-byte position in this feed.
            pub closed spec fn from(&self) -> nat { self.start as nat }
            /// Exclusive raw-byte endpoint of this feed.
            pub closed spec fn to(&self) -> nat { self.end as nat }
        }
        sail_method! {
            /// Borrows the unmodified feed bytes.
            pub fn bytes(&self) -> (bytes: &'a [u8])
                requires (true)
                ensures (bytes@ == self.view())
            { self.bytes }
        }
        sail_method! {
            /// First raw-byte position in this feed.
            pub fn start(&self) -> (start: usize)
                requires (true)
                ensures (start == self.from())
            { self.start }
        }
        sail_method! {
            /// Exclusive raw-byte endpoint in this feed.
            pub fn end(&self) -> (end: usize)
                requires (true)
                ensures (end == self.to())
            { self.end }
        }
    }
}

sail_code! {
    impl RawTranscript {
        sail_impl_spec! {
            /// Complete received input bytes, including comments and separators.
            pub closed spec fn view(&self) -> Seq<u8> { self.bytes@ }
            /// Number of bytes already returned in contiguous prefix feeds.
            pub closed spec fn fed_view(&self) -> nat { self.fed as nat }
            /// Whether interval interpretation has never failed.
            pub closed spec fn active(&self) -> bool { self.valid }
        }
        sail_method! {
            /// Creates an empty valid transcript.
            pub fn empty() -> (transcript: Self)
                requires (true)
                ensures (transcript.view() == Seq::<u8>::empty(), transcript.fed_view() == 0,
                    transcript.active())
            { Self { bytes: Vec::new(), fed: 0, valid: true } }
        }
        sail_method! {
            /// Appends every received byte exactly once without moving the feed endpoint.
            pub fn append(&mut self, bytes: &[u8]) -> (result: ())
                requires (true)
                ensures (final(self).view() == old(self).view() + bytes@,
                    final(self).fed_view() == old(self).fed_view(),
                    final(self).active() == old(self).active())
            { self.bytes.extend_from_slice(bytes); }
        }
        sail_method! {
            /// Borrows the complete original input for solver replay.
            pub fn bytes(&self) -> (bytes: &[u8])
                requires (true)
                ensures (bytes@ == self.view())
            { self.bytes.as_slice() }
        }
        sail_method! {
            /// Number of raw bytes returned for prefix hashing.
            pub fn fed(&self) -> (fed: usize)
                requires (true)
                ensures (fed == self.fed_view())
            { self.fed }
        }
        sail_method! {
            /// Whether every interpreted interval has passed its byte checks.
            pub fn valid(&self) -> (valid: bool)
                requires (true)
                ensures (valid == self.active())
            { self.valid }
        }
        sail_method! {
            /// Permanently refuses further cache interpretation, preserving all raw bytes.
            pub fn refuse(&mut self) -> (result: ())
                requires (true)
                ensures (!final(self).active(), final(self).view() == old(self).view(),
                    final(self).fed_view() == old(self).fed_view())
            { self.valid = false; }
        }
        sail_method! {
            /// Whether the still-unfed suffix is only whitespace or line comments.
            pub fn trailing_trivia(&self) -> (valid: bool)
                requires (true)
                ensures (valid == (self.fed_view() <= self.view().len()
                    && (trivia_after(self.view().subrange(self.fed_view() as int, self.view().len() as int), false) == Some(false)
                    || trivia_after(self.view().subrange(self.fed_view() as int, self.view().len() as int), false) == Some(true))))
            {
                if self.fed > self.bytes.len() { return false; }
                trivia(&self.bytes[self.fed..], true)
            }
        }
        sail_method! {
            /// Validates one parser interval and returns every raw byte since the last feed.
            /// A changed, overlapping, omitted or out-of-range item permanently refuses caching.
            pub fn command<'a>(&'a mut self, offset: usize, command: &[u8]) -> (feed: Option<PrefixFeed<'a>>)
                requires (true)
                ensures (final(self).view() == old(self).view(),
                    feed.is_some() == (old(self).active() && command@.len() > 0
                        && old(self).fed_view() <= offset
                        && offset as nat + command@.len() <= old(self).view().len()
                        && old(self).view().subrange(offset as int, offset as int + command@.len() as int) == command@
                        && trivia_after(old(self).view().subrange(old(self).fed_view() as int, offset as int), false) == Some(false)),
                    feed.is_some() ==> (
                        old(self).active() && final(self).active()
                        && old(self).fed_view() <= offset
                        && offset < final(self).fed_view() <= old(self).view().len()
                        && final(self).fed_view() == offset as nat + command@.len()
                        && old(self).view().subrange(offset as int, final(self).fed_view() as int) == command@
                        && feed.unwrap().from() == old(self).fed_view()
                        && feed.unwrap().to() == final(self).fed_view()
                        && feed.unwrap().view() == old(self).view().subrange(
                            old(self).fed_view() as int, final(self).fed_view() as int)
                        && trivia_after(old(self).view().subrange(old(self).fed_view() as int, offset as int), false) == Some(false)
                    ),
                    feed.is_none() ==> (!final(self).active()
                        && final(self).fed_view() == old(self).fed_view()))
            {
                if !self.valid || command.is_empty() || self.fed > offset {
                    self.valid = false;
                    return None;
                }
                let raw_length = self.bytes.len();
                let Some(end) = offset.checked_add(command.len()) else {
                    self.valid = false;
                    return None;
                };
                if end > raw_length {
                    self.valid = false;
                    return None;
                }
                if !super::inputs::byte_eq(&self.bytes[offset..end], command)
                    || !trivia(&self.bytes[self.fed..offset], false)
                {
                    self.valid = false;
                    return None;
                }
                let start = self.fed;
                self.fed = end;
                Some(PrefixFeed { bytes: &self.bytes[start..end], start, end })
            }
        }
        sail_method! {
            /// Returns the entire remaining raw suffix without interpreting its syntax.
            /// EOF/parser or explicit-exit completion is checked separately by the adapter.
            pub fn tail<'a>(&'a mut self) -> (feed: Option<PrefixFeed<'a>>)
                requires (true)
                ensures (final(self).view() == old(self).view(),
                    feed.is_some() == (old(self).active() && old(self).fed_view() <= old(self).view().len()),
                    feed.is_some() ==> (old(self).active() && final(self).active()
                        && old(self).fed_view() <= old(self).view().len()
                        && final(self).fed_view() == old(self).view().len()
                        && feed.unwrap().from() == old(self).fed_view()
                        && feed.unwrap().to() == old(self).view().len()
                        && feed.unwrap().view() == old(self).view().subrange(
                            old(self).fed_view() as int, old(self).view().len() as int)),
                    feed.is_none() ==> (!final(self).active()
                        && final(self).fed_view() == old(self).fed_view()))
            {
                if !self.valid || self.fed > self.bytes.len() {
                    self.valid = false;
                    return None;
                }
                let start = self.fed;
                let end = self.bytes.len();
                self.fed = end;
                Some(PrefixFeed { bytes: &self.bytes[start..end], start, end })
            }
        }
    }
}

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

sail_code! {
    /// Lexical contexts that hide structural parentheses in one solver form.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FrameLex {
        /// Parentheses delimit lists.
        Plain,
        /// Inside a double-quoted string.
        String,
        /// A quote may close the string or begin a doubled-quote escape.
        Quote,
        /// Inside a bar-quoted symbol.
        Symbol,
        /// Inside an input semicolon comment.
        Comment,
    }
}

sail_spec! {
    /// Parentheses count only outside strings, quoted symbols and comments.
    pub open spec fn structural(lex: FrameLex, byte: u8) -> bool {
        lex is Plain || lex is Quote && byte != b'"'
    }

    /// Lexical grammar transition for an actual byte; no token normalization.
    pub open spec fn frame_lex(lex: FrameLex, byte: u8, comments: bool) -> FrameLex {
        if lex is String { if byte == b'"' { FrameLex::Quote } else { FrameLex::String } }
        else if lex is Symbol { if byte == b'|' { FrameLex::Plain } else { FrameLex::Symbol } }
        else if lex is Comment {
            if byte == b'\n' || byte == b'\r' { FrameLex::Plain } else { FrameLex::Comment }
        } else if lex is Quote && byte == b'"' { FrameLex::String }
        else if byte == b'"' { FrameLex::String }
        else if byte == b'|' { FrameLex::Symbol }
        else if comments && byte == b';' { FrameLex::Comment }
        else { FrameLex::Plain }
    }

    /// Mathematical nesting of every exact prefix under the lexical grammar.
    /// Negative depths remain representable so malformed streams are not assumed away.
    pub open spec fn frame_scan(bytes: Seq<u8>, comments: bool) -> (int, FrameLex)
        decreases bytes.len(),
    {
        if bytes.len() == 0 { (0, FrameLex::Plain) }
        else {
            let previous = frame_scan(bytes.drop_last(), comments);
            let byte = bytes.last();
            (previous.0 + if structural(previous.1, byte) && byte == b'(' { 1int }
                else if structural(previous.1, byte) && byte == b')' { -1int } else { 0int },
                frame_lex(previous.1, byte, comments))
        }
    }

    /// One complete list, with no proper prefix already closing its root.
    pub open spec fn single_list(bytes: Seq<u8>, comments: bool) -> bool {
        bytes.len() >= 2 && bytes[0] == b'(' && bytes.last() == b')'
            && frame_scan(bytes, comments) == (0, FrameLex::Plain)
            && forall|i: int| 1 <= i < bytes.len() ==> frame_scan(bytes.take(i), comments).0 > 0
    }
}

sail_verified! {
    /// Advances actual lexical state and nesting using the framing grammar.
    pub fn frame_step(depth: usize, lex: FrameLex, byte: u8, comments: bool)
        -> (next: (usize, FrameLex))
        requires (0 < depth < usize::MAX)
        ensures (next.0 as int == depth as int
                + if structural(lex, byte) && byte == b'(' { 1int }
                else if structural(lex, byte) && byte == b')' { -1int } else { 0int },
            next.1 == frame_lex(lex, byte, comments),
            next.0 == 0 ==> byte == b')' && next.1 is Plain)
    {
        if matches!(lex, FrameLex::String) {
            return (depth, if byte == b'"' { FrameLex::Quote } else { FrameLex::String });
        }
        if matches!(lex, FrameLex::Symbol) {
            return (depth, if byte == b'|' { FrameLex::Plain } else { FrameLex::Symbol });
        }
        if matches!(lex, FrameLex::Comment) {
            return (depth, if byte == b'\n' || byte == b'\r' { FrameLex::Plain } else { FrameLex::Comment });
        }
        if matches!(lex, FrameLex::Quote) && byte == b'"' {
            return (depth, FrameLex::String);
        }
        let next = if byte == b'(' { depth + 1 }
            else if byte == b')' { depth - 1 } else { depth };
        let lex = if byte == b'"' { FrameLex::String }
            else if byte == b'|' { FrameLex::Symbol }
            else if comments && byte == b';' { FrameLex::Comment }
            else { FrameLex::Plain };
        (next, lex)
    }
}

sail_verified! {
    /// Accepts exactly one complete parenthesized frame, without trusting offsets.
    /// Strings use doubled quotes; symbols use bars; only input permits comments.
    pub fn complete_list(bytes: &[u8], comments: bool) -> (accepted: bool)
        requires (true)
        ensures (accepted == single_list(bytes@, comments))
    {
        if bytes.len() < 2 || bytes[0] != b'(' { return false; }
        let mut i = 1usize;
        let mut depth = 1usize;
        let mut lex = FrameLex::Plain;
        sail_proof! {
            assert(bytes@.take(1) =~= seq![b'(']);
            reveal_with_fuel(frame_scan, 2);
        }
        sail_while! { (i < bytes.len())
            invariant (1 <= i <= bytes.len(), 0 < depth <= i,
                bytes.len() >= 2, bytes@[0] == b'(',
                frame_scan(bytes@.take(i as int), comments) == (depth as int, lex),
                forall|j: int| 1 <= j <= i ==> frame_scan(bytes@.take(j), comments).0 > 0)
            decreases (bytes.len() - i)
            {
                let byte = bytes[i];
                let next = frame_step(depth, lex, byte, comments);
                sail_proof! {
                    assert(bytes@.take(i as int + 1).drop_last() =~= bytes@.take(i as int));
                    assert(bytes@.take(i as int + 1).last() == byte);
                    reveal_with_fuel(frame_scan, 1);
                }
                i += 1;
                depth = next.0;
                lex = next.1;
                if depth == 0 { return i == bytes.len(); }
                sail_proof! {
                    assert forall|j: int| 1 <= j <= i implies frame_scan(bytes@.take(j), comments).0 > 0 by {
                        if j == i {} else { assert(j <= i - 1); }
                    }
                }
            }
        }
        sail_proof! { assert(bytes@.take(bytes.len() as int) =~= bytes@); }
        false
    }
}

sail_spec! {
    /// Whitespace and lexical punctuation ending a simple SMT symbol.
    pub open spec fn word_delimiter(byte: u8) -> bool {
        byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n'
            || byte == b'(' || byte == b')' || byte == b'"' || byte == b'|' || byte == b';'
    }
    /// A native-compatible bare-head first byte, excluding numeral/keyword/bit prefixes.
    pub open spec fn symbol_first(byte: u8) -> bool {
        !word_delimiter(byte) && !(b'0' <= byte <= b'9') && byte != b'#' && byte != b':'
    }
    /// Exact first bare command-head token inside one framed list.
    pub open spec fn head_span(bytes: Seq<u8>, start: int, end: int) -> bool {
        single_list(bytes, true) && 1 <= start < end < bytes.len()
            && trivia_after(bytes.subrange(1, start), false) == Some(false)
            && symbol_first(bytes[start]) && word_delimiter(bytes[end])
            && forall|i: int| start <= i < end ==> !word_delimiter(bytes[i])
    }

    /// Trivia composition follows the actual lexical comment state, including
    /// a CR/LF boundary split across arbitrary received fragments.
    pub proof fn trivia_join(left: Seq<u8>, right: Seq<u8>, comment: bool)
        ensures trivia_after(left + right, comment) == match trivia_after(left, comment) {
            Some(next) => trivia_after(right, next), None => None,
        },
        decreases left.len(),
    {
        if left.len() > 0 {
            assert((left + right).drop_first() =~= left.drop_first() + right);
            if comment {
                trivia_join(left.drop_first(), right, left[0] != b'\n' && left[0] != b'\r');
            } else if left[0] == b' ' || left[0] == b'\t' || left[0] == b'\n' || left[0] == b'\r' {
                trivia_join(left.drop_first(), right, false);
            } else if left[0] == b';' {
                trivia_join(left.drop_first(), right, true);
            }
        } else { assert(left + right =~= right); }
    }
}

sail_verified! {
    /// Checks the same exact delimiter byte set used by the native SMT lexer.
    pub fn delimiter(byte: u8) -> (is_delimiter: bool)
        requires (true)
        ensures (is_delimiter == word_delimiter(byte))
    { matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b'(' | b')' | b'"' | b'|' | b';') }
}

sail_verified! {
    /// Extracts the actual first bare-head name only after full framing.
    /// This does not validate the command's arity, types or solver semantics.
    pub fn command_head(bytes: &[u8]) -> (head: Option<&[u8]>)
        requires (true)
        ensures (head.is_some() ==> exists|start: int, end: int|
            head_span(bytes@, start, end) && head.unwrap()@ == bytes@.subrange(start, end),
            (exists|start: int, end: int| head_span(bytes@,start,end)) ==> head.is_some(),
            forall|start: int,end: int| head_span(bytes@,start,end) ==> head.is_some() && head.unwrap()@ == bytes@.subrange(start,end),
            single_list(bytes@, true) && symbol_first(bytes@[1]) ==> head.is_some(),
            single_list(bytes@, true) && !frame_space(bytes@[1]) && bytes@[1] != b';'
                && !symbol_first(bytes@[1]) ==> head.is_none())
    {
        if !complete_list(bytes, true) { return None; }
        let mut i = 1usize;
        let mut comment = false;
        sail_proof! { assert(bytes@.subrange(1, 1) =~= Seq::<u8>::empty()); }
        sail_while! { (i < bytes.len() && (comment || bytes[i] == b';'
                || bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r'))
            invariant (1 <= i <= bytes.len(), single_list(bytes@, true),
                trivia_after(bytes@.subrange(1, i as int), false) == Some(comment),
                forall|start: int,end: int| head_span(bytes@,start,end) ==> i <= start,
                !frame_space(bytes@[1]) && bytes@[1] != b';' ==> i == 1 && !comment)
            decreases (bytes.len() - i)
            {
                let byte = bytes[i];
                sail_proof! {
                    assert forall|start: int,end: int| head_span(bytes@,start,end) implies i < start by {
                        if i == start {
                            assert(!comment);
                            assert(symbol_first(byte));
                            assert(false);
                        }
                    }
                    trivia_join(bytes@.subrange(1, i as int), seq![byte], false);
                    reveal_with_fuel(trivia_after, 2);
                    assert(bytes@.subrange(1, i as int + 1) =~= bytes@.subrange(1, i as int) + seq![byte]);
                }
                if comment { comment = byte != b'\n' && byte != b'\r'; }
                else if byte == b';' { comment = true; }
                i += 1;
            }
        }
        sail_proof! {
            assert forall|start: int,end: int| head_span(bytes@,start,end) implies i == start by {
                if i < start {
                    assert(!comment);
                    trivia_join(bytes@.subrange(1,i as int),bytes@.subrange(i as int,start),false);
                    assert(bytes@.subrange(1,start) =~= bytes@.subrange(1,i as int) + bytes@.subrange(i as int,start));
                    assert(bytes@.subrange(i as int,start)[0] == bytes@[i as int]);
                    reveal_with_fuel(trivia_after,1);
                    assert(false);
                }
            }
        }
        if i == bytes.len() || delimiter(bytes[i]) || bytes[i] == b'#' || bytes[i] == b':'
            || (b'0' <= bytes[i] && bytes[i] <= b'9') { return None; }
        let start = i;
        sail_while! { (i < bytes.len() && !delimiter(bytes[i]))
            invariant (1 <= start <= i <= bytes.len(), start < bytes.len(),
                single_list(bytes@, true), symbol_first(bytes@[start as int]),
                forall|s: int,e: int| head_span(bytes@,s,e) ==> start == s && i <= e,
                trivia_after(bytes@.subrange(1, start as int), false) == Some(false),
                forall|j: int| start <= j < i ==> !word_delimiter(bytes@[j]))
            decreases (bytes.len() - i)
            {
                sail_proof! {
                    assert forall|s: int,e: int| head_span(bytes@,s,e) implies i < e by {
                        if i == e { assert(word_delimiter(bytes@[i as int])); }
                    }
                }
                i += 1;
            }
        }
        if i == bytes.len() { return None; }
        let head = &bytes[start..i];
        sail_proof! { assert(head_span(bytes@, start as int, i as int)); }
        Some(head)
    }
}

sail_spec! {
    /// SMT stream whitespace, without changing or stripping the actual payload.
    pub open spec fn frame_space(byte: u8) -> bool {
        byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n'
    }
    /// One output frame: a complete list, or one non-list line, with surrounding whitespace.
    pub open spec fn response_trim(bytes: Seq<u8>, start: int, end: int) -> bool {
        0 <= start < end <= bytes.len()
            && (forall|j: int| 0 <= j < start ==> frame_space(bytes[j]))
            && (forall|j: int| end <= j < bytes.len() ==> frame_space(bytes[j]))
            && !frame_space(bytes[start]) && !frame_space(bytes[end - 1])
    }
    /// One precise nonempty response range inside its raw transport whitespace.
    pub open spec fn response_span(bytes: Seq<u8>, start: int, end: int) -> bool {
        response_trim(bytes, start, end) && (if bytes[start] == b'(' { single_list(bytes.subrange(start, end), false) }
                else { (forall|j: int| start <= j < end ==> bytes[j] != b'\n')
                    && exists|j: int| end <= j < bytes.len() && bytes[j] == b'\n' })
    }
    /// One complete response according to the supported output framing grammar.
    pub open spec fn response_shape(bytes: Seq<u8>) -> bool {
        exists|start: int, end: int| response_span(bytes, start, end)
    }
    /// Byte-observable first/last non-whitespace bounds are unique.
    pub proof fn response_trim_unique(bytes: Seq<u8>, start: int, end: int, a: int, b: int)
        requires response_trim(bytes, start, end), response_trim(bytes, a, b),
        ensures start == a, end == b,
    {
        if start < a { assert(frame_space(bytes[start])); }
        if a < start { assert(frame_space(bytes[a])); }
        if end < b { assert(frame_space(bytes[b - 1])); }
        if b < end { assert(frame_space(bytes[end - 1])); }
    }
    /// Choosing a trim range cannot hide another supported response partition.
    pub proof fn response_at_trim(bytes: Seq<u8>, start: int, end: int)
        requires response_trim(bytes, start, end),
        ensures response_shape(bytes) == response_span(bytes, start, end),
    {
        if response_shape(bytes) {
            let (a, b) = choose|a: int, b: int| response_span(bytes, a, b);
            response_trim_unique(bytes, start, end, a, b);
        } else if response_span(bytes, start, end) { assert(response_shape(bytes)); }
    }
}

sail_verified! {
    /// Checks stream whitespace without interpreting locale or Unicode.
    pub fn stream_space(byte: u8) -> (space: bool)
        requires (true)
        ensures (space == frame_space(byte))
    { matches!(byte, b' ' | b'\t' | b'\r' | b'\n') }
}

sail_verified! {
    /// Refuses coalesced, partial or empty reply payloads before cache admission.
    /// Echo text remains command-dependent; this check grants no solver origin.
    pub fn response_frame(bytes: &[u8]) -> (accepted: bool)
        requires (true)
        ensures (accepted == response_shape(bytes@))
    {
        let mut start = 0usize;
        sail_while! { (start < bytes.len() && stream_space(bytes[start]))
            invariant (start <= bytes.len(), forall|j: int| 0 <= j < start ==> frame_space(bytes@[j]))
            decreases (bytes.len() - start)
            { start += 1; }
        }
        if start == bytes.len() {
            sail_proof! {
                if response_shape(bytes@) {
                    let (a, b) = choose|a: int, b: int| response_span(bytes@, a, b);
                    assert(frame_space(bytes@[a]));
                }
            }
            return false;
        }
        let mut end = bytes.len();
        let mut newline = false;
        sail_while! { (end > start && stream_space(bytes[end - 1]))
            invariant (start <= end <= bytes.len(), start < bytes.len(), !frame_space(bytes@[start as int]),
                forall|j: int| 0 <= j < start ==> frame_space(bytes@[j]),
                forall|j: int| end <= j < bytes.len() ==> frame_space(bytes@[j]),
                newline == (exists|j: int| end <= j < bytes.len() && bytes@[j] == b'\n'))
            decreases (end - start)
            {
                if bytes[end - 1] == b'\n' { newline = true; }
                end -= 1;
            }
        }
        if end == start { return false; }
        sail_proof! {
            assert(response_trim(bytes@, start as int, end as int));
            response_at_trim(bytes@, start as int, end as int);
        }
        if bytes[start] == b'(' {
            let accepted = complete_list(&bytes[start..end], false);
            sail_proof! {
                if accepted {
                    assert(response_span(bytes@, start as int, end as int));
                    assert(response_shape(bytes@));
                }
            }
            return accepted;
        }
        if !newline { return false; }
        let mut i = start;
        sail_while! { (i < end)
            invariant (start <= i <= end <= bytes.len(), start < end, bytes@[start as int] != b'(',
                response_shape(bytes@) == response_span(bytes@, start as int, end as int),
                exists|j: int| end <= j < bytes.len() && bytes@[j] == b'\n',
                !frame_space(bytes@[start as int]), !frame_space(bytes@[end as int - 1]),
                forall|j: int| 0 <= j < start ==> frame_space(bytes@[j]),
                forall|j: int| end <= j < bytes.len() ==> frame_space(bytes@[j]),
                forall|j: int| start <= j < i ==> bytes@[j] != b'\n')
            decreases (end - i)
            {
                if bytes[i] == b'\n' { return false; }
                i += 1;
            }
        }
        sail_proof! {
            assert(response_span(bytes@, start as int, end as int));
            assert(response_shape(bytes@));
        }
        true
    }
}

sail_verified! {
    /// Checks command framing and exact first-head bytes before deriving its prefix binding.
    /// Any refusal permanently disables cache interpretation while preserving raw input.
    pub fn framed_command<'a, 'b>(raw: &'a mut RawTranscript, offset: usize, command: &'b [u8])
        -> (accepted: Option<(PrefixFeed<'a>, &'b [u8])>)
        requires (true)
        ensures (final(raw).view() == old(raw).view(),
            accepted.is_some() ==> (
                final(raw).active() && old(raw).active() && single_list(command@, true)
                && exists|start: int, end: int| head_span(command@, start, end)
                    && accepted.unwrap().1@ == command@.subrange(start, end)),
            accepted.is_some() ==> (
                old(raw).fed_view() <= offset
                && final(raw).fed_view() == offset as nat + command@.len()
                && final(raw).fed_view() <= old(raw).view().len()
                && old(raw).view().subrange(offset as int, final(raw).fed_view() as int) == command@
                && accepted.unwrap().0.from() == old(raw).fed_view()
                && accepted.unwrap().0.to() == final(raw).fed_view()
                && accepted.unwrap().0.view() == old(raw).view().subrange(old(raw).fed_view() as int, final(raw).fed_view() as int)
                && trivia_after(old(raw).view().subrange(old(raw).fed_view() as int, offset as int), false) == Some(false)),
            accepted.is_none() ==> (!final(raw).active() && final(raw).fed_view() == old(raw).fed_view()),
            old(raw).active() && (exists|start: int,end: int| head_span(command@,start,end))
                && old(raw).fed_view() <= offset
                && offset as nat + command@.len() <= old(raw).view().len()
                && old(raw).view().subrange(offset as int, offset as int + command@.len() as int) == command@
                && trivia_after(old(raw).view().subrange(old(raw).fed_view() as int, offset as int), false) == Some(false)
                ==> accepted.is_some(),
            old(raw).active() && single_list(command@, true) && symbol_first(command@[1])
                && old(raw).fed_view() <= offset
                && offset as nat + command@.len() <= old(raw).view().len()
                && old(raw).view().subrange(offset as int, offset as int + command@.len() as int) == command@
                && trivia_after(old(raw).view().subrange(old(raw).fed_view() as int, offset as int), false) == Some(false)
                ==> accepted.is_some())
    {
        let head = command_head(command);
        let Some(head) = head else { raw.refuse(); return None; };
        let feed = raw.command(offset, command);
        match feed { Some(feed) => Some((feed, head)), None => None }
    }
}

sail_verified! {
    /// A nonempty command head following whitespace and a completed comment is accepted exactly.
    #[allow(clippy::byte_char_slices, reason = "Explicit byte values expose the constructive grammar witness to stock verification.")]
    pub fn commented_head_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let bytes = [b'(',b' ',b';',b'x',b'\n',b'c',b')'];
        sail_proof! {
            reveal_with_fuel(frame_scan,8);
            reveal_with_fuel(trivia_after,6);
            assert(head_span(bytes@,5,6));
        }
        let expected = [b'c'];
        match command_head(&bytes) {
            Some(head) => {
                sail_proof! { assert(head@ == bytes@.subrange(5,6)); assert(head@ =~= expected@); }
                crate::inputs::byte_eq(head,&expected)
            }
            None => false,
        }
    }
}
