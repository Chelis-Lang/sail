//! Owned solver-pipe delivery, separately checked against the parser-side key.
//!
//! This establishes full-write accounting, not a compiler input closure or the
//! identity of the executable the operating system actually launches.
use crate::key_prefix::{HashHost, Prefix};
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_trait! {
    /// Primitives on one owned input pipe. Implementations bind actual writes
    /// and closure; successful writes acknowledge every supplied byte in order.
    pub trait InputPipe {
        spec {
            spec fn sent(&self) -> Seq<u8>;
            spec fn closed(&self) -> bool;
            /// Only executable script controls predict their next primitive
            /// result. Native pipes use None and supply actual I/O outcomes.
            spec fn success_hint(&self) -> Option<bool>;
        }
        /// Writes and flushes the exact bytes through this pipe.
        /// A failure may have partially written; it never establishes authority.
        fn write(&mut self, bytes: &[u8]) -> (ok: bool)
            requires (!old(self).closed())
            ensures (final(self).closed() == old(self).closed(),
                old(self).success_hint().is_some() ==> ok == old(self).success_hint().unwrap(),
                final(self).success_hint() == match old(self).success_hint() { None => None, Some(_) => Some(true) },
                ok ==> final(self).sent() == old(self).sent() + bytes@);
        /// Closes this pipe's owned descriptor.
        fn close(&mut self) -> (out: ())
            requires (true)
            ensures (final(self).closed(), final(self).sent() == old(self).sent());
    }
}

sail_code! {
    /// Input pipe and independent hash of its successful complete deliveries.
    pub struct Delivery<Pipe, Host> {
        pipe: Pipe,
        #[allow(dead_code, reason = "The initial hash state is retained as the exact non-ghost representation witness checked by stock Verus.")]
        initial: Prefix<Host>,
        delivered: Prefix<Host>,
        healthy: bool,
        closed: bool,
    }

    impl<Pipe: InputPipe, Host: HashHost> Delivery<Pipe, Host> {
        sail_impl_spec! {
            pub closed spec fn material(&self) -> Seq<u8> { self.delivered.material() }
            pub closed spec fn initial_material(&self) -> Seq<u8> { self.initial.material() }
            pub closed spec fn sent(&self) -> Seq<u8> { self.pipe.sent() }
            pub closed spec fn length(&self) -> u64 { self.delivered.length() }
            pub closed spec fn healthy(&self) -> bool { self.healthy }
            pub closed spec fn closed(&self) -> bool { self.closed }
            pub closed spec fn success_hint(&self) -> Option<bool> { self.pipe.success_hint() }
            pub closed spec fn coherent(&self) -> bool {
                self.initial.length() == 0 && self.initial.exact()
                && self.closed == self.pipe.closed()
                && (self.healthy ==> self.delivered.exact()
                    && self.delivered.material() == self.initial.material() + self.pipe.sent()
                    && self.delivered.length() == self.pipe.sent().len())
            }
        }
        sail_method! {
            /// The caller transfers one fresh actual pipe and the header-only
            /// seed of the same parser session, before any delivery.
            pub fn new(pipe: Pipe, prefix: Prefix<Host>) -> (out: Self)
                requires (!pipe.closed(), pipe.sent().len() == 0,
                    prefix.length() == 0, prefix.exact())
                ensures (out.coherent(), out.healthy(), !out.closed(),
                    out.initial_material() == prefix.material(), out.material() == prefix.material(),
                    out.length() == 0, out.sent().len() == 0, out.success_hint() == pipe.success_hint())
            {
                let initial = prefix.duplicate();
                Self { pipe, initial, delivered: prefix, healthy: true, closed: false }
            }
        }
        sail_method! {
            /// Calls the actual full-write primitive before appending these
            /// bytes to the independent delivered-prefix state.
            pub fn write(&mut self, bytes: &[u8]) -> (ok: bool)
                requires (old(self).coherent(), bytes.len() <= u64::MAX)
                ensures (final(self).coherent(), final(self).closed() == old(self).closed(),
                    final(self).initial_material() == old(self).initial_material(),
                    ok ==> !old(self).closed()
                        && final(self).material() == old(self).material() + bytes@,
                    !old(self).closed() && old(self).success_hint().is_some() ==> ok == old(self).success_hint().unwrap(),
                    !old(self).closed() ==> final(self).success_hint() == match old(self).success_hint() { None => None, Some(_) => Some(true) },
                    ok && old(self).length() as int + bytes.len() <= u64::MAX ==> final(self).length() == old(self).length() + bytes.len(),
                    ok && old(self).healthy() && old(self).length() as int + bytes.len() <= u64::MAX ==> final(self).healthy(),
                    !ok ==> !final(self).healthy(),
                    !old(self).healthy() ==> !final(self).healthy())
            {
                if self.closed { self.healthy = false; return false; }
                if !self.pipe.write(bytes) { self.healthy = false; return false; }
                self.delivered.update(bytes);
                if !self.delivered.is_exact() { self.healthy = false; }
                sail_proof! {
                    reveal(Delivery::coherent);
                    if self.healthy {
                        assert(self.delivered.material() =~= self.initial.material() + self.pipe.sent());
                        assert(self.delivered.length() == self.pipe.sent().len());
                    }
                }
                true
            }
        }
        sail_method! {
            /// Whether writes remain possible; failed writes permanently end
            /// this delivery's publication eligibility.
            pub fn active(&self) -> (active: bool)
                requires (true) ensures (active == (self.healthy() && !self.closed()))
            { self.healthy && !self.closed }
        }
        sail_method! {
            /// Compares the independently acknowledged full input with the
            /// parser session's complete-silence key before terminal delivery.
            pub fn matches(&self, expected: [u8; 32], complete: bool) -> (accepted: bool)
                requires (self.coherent())
                ensures (accepted == (complete && self.healthy() && !self.closed()
                        && Host::digest(crate::key_prefix::field(b"sail-key/session-silence/v1"@)
                            + crate::key_prefix::field(Host::digest(crate::key_prefix::end(self.material(),self.length()))@)) == expected),
                    accepted ==> self.material() == self.initial_material() + self.sent()
                        && self.length() == self.sent().len())
            {
                if !complete || !self.healthy || self.closed { return false; }
                let key = self.delivered.silence_key();
                let accepted = crate::inputs::byte_eq(&key, &expected);
                sail_proof! { if accepted { assert(key =~= expected); } }
                accepted
            }
        }
        sail_method! {
            /// Closes the same owned descriptor after terminal expectations and
            /// their completion signal have been delivered to the output side.
            pub fn close(&mut self) -> (out: ())
                requires (old(self).coherent())
                ensures (final(self).coherent(), final(self).closed(),
                    final(self).initial_material() == old(self).initial_material(),
                    final(self).material() == old(self).material(), final(self).sent() == old(self).sent(),
                    final(self).healthy() == old(self).healthy())
            {
                self.pipe.close();
                self.closed = true;
            }
        }
    }
}

sail_code! {
    /// Executable narrow-pipe control, including a partial write on failure.
    /// This is a constructive witness, never an operating-system observation.
    pub struct ScriptPipe {
        bytes: Vec<u8>,
        closed: bool,
        fail_next: bool,
    }
    impl InputPipe for ScriptPipe {
        sail_trait_impl_spec! {
            closed spec fn sent(&self) -> Seq<u8> { self.bytes@ }
            closed spec fn closed(&self) -> bool { self.closed }
            closed spec fn success_hint(&self) -> Option<bool> { Some(!self.fail_next) }
        }
        fn write(&mut self, bytes: &[u8]) -> bool {
            if self.fail_next {
                self.fail_next = false;
                if !bytes.is_empty() { self.bytes.push(bytes[0]); }
                return false;
            }
            self.bytes.extend_from_slice(bytes);
            true
        }
        fn close(&mut self) { self.closed = true; }
    }
}

sail_verified! {
    /// Nonempty exact delivery split into two successful physical calls.
    pub fn complete_delivery_control<Host>(host: Host) -> (passed: bool)
        where (Host: HashHost)
        requires (host.bytes().len() == 0)
        ensures (passed)
    {
        let prefix = Prefix::new(host, b"z3", b"pinned", b"binary", b"platform", None);
        let mut expected = prefix.duplicate();
        expected.update(&[1u8,2u8]);
        let terminal = expected.silence_key();
        let mut delivery = Delivery::new(ScriptPipe { bytes: Vec::new(), closed: false, fail_next: false }, prefix);
        let a = delivery.write(&[1u8]);
        let b = delivery.write(&[2u8]);
        sail_proof! { assert(seq![1u8] + seq![2u8] =~= seq![1u8,2u8]); }
        sail_proof! {
            assert(a); assert(b); assert(delivery.healthy());
            assert(delivery.length() == expected.length());
            assert(delivery.material() =~= expected.material());
        }
        let accepted = delivery.matches(terminal, true);
        sail_proof! { assert(accepted); }
        let was_active = delivery.active();
        delivery.close();
        let after_close = delivery.matches(terminal, true);
        a && b && accepted && was_active && !after_close && !delivery.active()
    }
}

sail_verified! {
    /// A later successful write cannot heal a partially delivered failed call.
    pub fn failed_delivery_control<Host>(host: Host) -> (passed: bool)
        where (Host: HashHost)
        requires (host.bytes().len() == 0)
        ensures (passed)
    {
        let prefix = Prefix::new(host, b"z3", b"pinned", b"binary", b"platform", None);
        let terminal = prefix.silence_key();
        let mut delivery = Delivery::new(ScriptPipe { bytes: Vec::new(), closed: false, fail_next: true }, prefix);
        let first = delivery.write(b"bad");
        let second = delivery.write(b"later");
        let accepted = delivery.matches(terminal, true);
        delivery.close();
        !first && second && !accepted && !delivery.active()
    }
}
