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
