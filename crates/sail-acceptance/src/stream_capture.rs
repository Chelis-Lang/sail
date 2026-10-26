//! Exact read-prefix and EOF capture for an owned compiler pipe.
//!
//! Native endpoint attribution and std/OS read semantics remain explicit
//! primitives. Neither a byte count nor child exit manufactures EOF.

#[cfg(sail_verus)]
use vstd::prelude::*;

use crate::inputs::OwnerIdentity;

sail_code! {
    /// Open read transaction; only actual reads can establish its EOF state.
    pub struct PipeCapture<Owner> {
        owner: Owner,
        kind: u8,
        bytes: Vec<u8>,
        eof: bool,
        failed: bool,
    }

    /// Immutable bytes and endpoint identity sealed after an actual EOF read.
    pub struct SealedPipe<Owner> {
        owner: Owner,
        kind: u8,
        bytes: Vec<u8>,
    }

    /// Actual bytes remain available when observation cannot supply an EOF token.
    pub enum PipeReadback<Owner> {
        /// Private EOF token from the checked read transaction.
        Observed(SealedPipe<Owner>),
        /// Fresh bytes without stream authority; never accepted as complete.
        Unavailable(Vec<u8>),
    }

    /// Actual primitive error or a bounded capture-storage refusal.
    pub enum PipeReadFailure<Failure> {
        /// Preserve the exact original read error for the native caller.
        Read(Failure),
        /// Addressable byte storage would overflow; no EOF token can be sealed.
        Capacity,
    }
}

sail_spec! {
    /// Optional concrete witness for one primitive read only, never a whole output.
    /// Native reads have no predicted outcome; finite fixture bodies compute theirs.
    pub enum PipePrimitiveOutcome<Failure> {
        Bytes(Seq<u8>), Eof, EmptyRead, Failure(Failure),
    }

    pub open spec fn primitive_read_matches<Failure>(out: Result<usize, Failure>, buffer: Seq<u8>, expected: PipePrimitiveOutcome<Failure>) -> bool {
        match expected {
            PipePrimitiveOutcome::Bytes(bytes) => match out { Ok(count) => count > 0 && count == bytes.len()
                && buffer.take(count as int) == bytes, _ => false },
            PipePrimitiveOutcome::Eof => out == Ok(0) && buffer.len() > 0,
            PipePrimitiveOutcome::EmptyRead => out == Ok(0) && buffer.len() == 0,
            PipePrimitiveOutcome::Failure(error) => out == Err(error),
        }
    }

    /// One concrete primitive read outcome; a zero-size read is not EOF.
    pub enum PipeReadObservation<Owner> {
        Bytes { owner: Owner, kind: u8, bytes: Seq<u8> },
        Eof { owner: Owner, kind: u8 },
        EmptyRead { owner: Owner, kind: u8 },
        Failure { owner: Owner, kind: u8, fault: u8 },
    }
}

sail_trait! {
    /// Only actual endpoint metadata, read outcomes and error-kind classification.
    /// Production keeps this provider private around its original child pipe.
    pub trait CompilerPipeReadHost<Owner, Failure> {
        spec {
            spec fn owner_view(&self) -> Owner;
            spec fn kind_view(&self) -> u8;
            spec fn observations(&self) -> Seq<PipeReadObservation<Owner>>;
            spec fn fault_view(error: &Failure) -> u8;
            spec fn next_read(&self, capacity: nat) -> Option<PipePrimitiveOutcome<Failure>>;
        }
        /// Borrow the original actual endpoint's ownership lease.
        fn owner(&self) -> (owner: &Owner)
            requires (true) ensures (*owner == self.owner_view());
        /// Observe its original stream role: zero stdout, one stderr.
        fn kind(&self) -> (kind: u8)
            requires (true) ensures (kind == self.kind_view());
        /// Zero is fatal, one interrupted, two would-block; no success flag is accepted.
        fn fault_kind(&self, error: &Failure) -> (kind: u8)
            requires (true) ensures (kind <= 2 && kind == Self::fault_view(error));
        /// Read into this exact supplied buffer through the same owned endpoint.
        ///
        /// # Errors
        /// Returns the original primitive read failure without fabricated bytes.
        fn read(&mut self, buffer: &mut [u8]) -> (out: Result<usize, Failure>)
            requires (true)
            ensures (final(self).owner_view() == old(self).owner_view(),
                final(self).kind_view() == old(self).kind_view(), final(buffer).len() == old(buffer).len(),
                match out { Ok(count) => count <= final(buffer).len(), Err(_) => true },
                match old(self).next_read(old(buffer).len()) {
                    Some(expected) => primitive_read_matches(out, final(buffer)@, expected), None => true,
                },
                final(self).observations() == old(self).observations().push(match out {
                    Ok(count) => if count > 0 { PipeReadObservation::Bytes {
                        owner: old(self).owner_view(), kind: old(self).kind_view(), bytes: final(buffer)@.take(count as int) }
                    } else if final(buffer).len() > 0 { PipeReadObservation::Eof {
                        owner: old(self).owner_view(), kind: old(self).kind_view() }
                    } else { PipeReadObservation::EmptyRead {
                        owner: old(self).owner_view(), kind: old(self).kind_view() } },
                    Err(error) => PipeReadObservation::Failure {
                        owner: old(self).owner_view(), kind: old(self).kind_view(), fault: Self::fault_view(&error) },
                }));
    }
}

impl<Owner> PipeCapture<Owner> {
    sail_impl_spec! {
        /// Original native lease retained across every primitive read.
        pub closed spec fn owner_view(&self) -> Owner { self.owner }
        /// Original stdout/stderr role, never relabeled at completion.
        pub closed spec fn kind_view(&self) -> u8 { self.kind }
        /// Exact successful primitive prefixes accumulated in order.
        pub closed spec fn bytes_view(&self) -> Seq<u8> { self.bytes@ }
        /// Only a nonempty-buffer zero result can set EOF.
        pub closed spec fn eof_view(&self) -> bool { self.eof }
        /// Identity, role, terminal, fatal-read and storage failures latch refusal.
        pub closed spec fn failed_view(&self) -> bool { self.failed }
    }
}

impl<Owner> SealedPipe<Owner> {
    sail_impl_spec! {
        /// Original actual endpoint lease.
        pub closed spec fn owner_view(&self) -> Owner { self.owner }
        /// Original actual endpoint role.
        pub closed spec fn kind_view(&self) -> u8 { self.kind }
        /// Exact returned read prefixes before the observed EOF.
        pub closed spec fn bytes_view(&self) -> Seq<u8> { self.bytes@ }
    }
}

sail_spec! {
    /// Raw bytes stay observable on both accepted and refused captures.
    pub open spec fn pipe_bytes<Owner>(readback: PipeReadback<Owner>) -> Seq<u8> {
        match readback { PipeReadback::Observed(pipe) => pipe.bytes_view(), PipeReadback::Unavailable(bytes) => bytes@ }
    }
    /// Only a private genuine-EOF token with matching lease/role can supply authority.
    pub open spec fn pipe_matches<Owner, Identity: OwnerIdentity<Owner>>(readback: PipeReadback<Owner>, owner: Owner, kind: u8) -> bool {
        match readback {
            PipeReadback::Observed(pipe) => kind <= 1 && pipe.kind_view() == kind
                && Identity::identity(&pipe.owner_view()) == Identity::identity(&owner),
            PipeReadback::Unavailable(_) => false,
        }
    }
}

impl<Owner> PipeReadback<Owner> {
    sail_method! {
        /// Borrow original bytes without promoting unavailable data to stream authority.
        pub fn bytes(&self) -> (bytes: &[u8])
            requires (true) ensures (bytes@ == pipe_bytes(*self))
        { match self { PipeReadback::Observed(pipe) => &pipe.bytes, PipeReadback::Unavailable(bytes) => bytes } }
    }
    sail_method! {
        /// Recover exact fresh bytes regardless of whether an EOF token exists.
        pub fn into_bytes(self) -> (bytes: Vec<u8>)
            requires (true) ensures (bytes@ == pipe_bytes(self))
        { match self { PipeReadback::Observed(pipe) => pipe.bytes, PipeReadback::Unavailable(bytes) => bytes } }
    }
}

sail_verified! {
    /// Begin without assuming that any read, EOF or successful I/O will occur.
    pub fn begin_pipe_capture<Owner>(owner: Owner, kind: u8) -> (out: PipeCapture<Owner>)
        requires (true)
        ensures (out.owner_view() == owner && out.kind_view() == kind && out.bytes_view() == Seq::<u8>::empty()
            && !out.eof_view() && out.failed_view() == (kind > 1))
    { PipeCapture { owner, kind, bytes: Vec::new(), eof: false, failed: kind > 1 } }
}

sail_verified! {
    /// Append only the actual successful read prefix, preserving original endpoint state.
    fn append_pipe_prefix<Owner>(capture: &mut PipeCapture<Owner>, prefix: &[u8]) -> (out: ())
        requires (old(capture).bytes_view().len() + prefix.len() <= usize::MAX)
        ensures (final(capture).owner_view() == old(capture).owner_view()
            && final(capture).kind_view() == old(capture).kind_view()
            && final(capture).eof_view() == old(capture).eof_view()
            && final(capture).failed_view() == old(capture).failed_view()
            && final(capture).bytes_view() == old(capture).bytes_view() + prefix@)
    {
        let mut i = 0;
        sail_while! { (i < prefix.len())
            invariant (i <= prefix.len(), capture.bytes@ == old(capture).bytes_view() + prefix@.take(i as int),
                capture.owner == old(capture).owner_view(), capture.kind == old(capture).kind_view(),
                capture.eof == old(capture).eof_view(), capture.failed == old(capture).failed_view(),
                old(capture).bytes_view().len() + prefix.len() <= usize::MAX)
            decreases (prefix.len() - i)
            { capture.bytes.push(prefix[i]); i += 1; }
        }
    }
}

sail_verified! {
    /// Drive the actual read primitive and capture its returned prefix and real EOF.
    /// The original read still runs after observation refusal; fresh I/O is preserved.
    ///
    /// # Errors
    /// Returns the actual primitive failure or refuses addressable capture overflow.
    pub fn capture_pipe_read<Owner, Failure, Host, Identity>(identity: &Identity, host: &mut Host,
        capture: &mut PipeCapture<Owner>, buffer: &mut [u8]) -> (out: Result<usize, PipeReadFailure<Failure>>)
        where (Host: CompilerPipeReadHost<Owner, Failure>, Identity: OwnerIdentity<Owner>)
        requires (true)
        ensures (final(capture).owner_view() == old(capture).owner_view()
            && final(capture).kind_view() == old(capture).kind_view(),
            final(host).owner_view() == old(host).owner_view()
                && final(host).kind_view() == old(host).kind_view() && final(buffer).len() == old(buffer).len(),
            final(capture).failed_view() == (old(capture).failed_view() || old(capture).eof_view()
                || Identity::identity(&old(capture).owner_view()) != Identity::identity(&old(host).owner_view())
                || old(capture).kind_view() != old(host).kind_view()
                || match out {
                    Err(PipeReadFailure::Capacity) => true,
                    Err(PipeReadFailure::Read(error)) => Host::fault_view(&error) != 1 && Host::fault_view(&error) != 2,
                    _ => false,
                }),
            final(capture).eof_view() == (old(capture).eof_view() || match out {
                Ok(count) => count == 0 && final(buffer).len() > 0, Err(_) => false }),
            old(capture).bytes_view().len() + old(buffer).len() <= usize::MAX
                ==> !(out is Err && out->Err_0 is Capacity),
            match old(host).next_read(old(buffer).len()) {
                Some(expected) => match out {
                    Ok(count) => primitive_read_matches(Ok(count), final(buffer)@, expected),
                    Err(PipeReadFailure::Read(error)) => primitive_read_matches(Err(error), final(buffer)@, expected),
                    Err(PipeReadFailure::Capacity) => true,
                }, None => true,
            },
            match out {
                Ok(count) => count <= final(buffer).len()
                    && final(capture).bytes_view() == old(capture).bytes_view() + final(buffer)@.take(count as int),
                Err(_) => final(capture).bytes_view() == old(capture).bytes_view(),
            })
    {
        if capture.eof || !identity.same_owner(&capture.owner, host.owner()) || capture.kind != host.kind() {
            capture.failed = true;
        }
        match host.read(buffer) {
            Ok(count) => {
                if count == 0 {
                    if !buffer.is_empty() { capture.eof = true; }
                } else {
                    if count > usize::MAX - capture.bytes.len() {
                        capture.failed = true;
                        return Err(PipeReadFailure::Capacity);
                    }
                    append_pipe_prefix(capture, &buffer[..count]);
                }
                Ok(count)
            }
            Err(error) => {
                let kind = host.fault_kind(&error);
                if kind != 1 && kind != 2 { capture.failed = true; }
                Err(PipeReadFailure::Read(error))
            }
        }
    }
}

sail_verified! {
    /// Seal only actual EOF; partial, failed and unsupported captures remain fresh bytes.
    pub fn finish_pipe_capture<Owner>(capture: PipeCapture<Owner>) -> (out: PipeReadback<Owner>)
        requires (true)
        ensures (pipe_bytes(out) == capture.bytes_view(),
            (out is Observed) == (capture.eof_view() && !capture.failed_view()),
            out is Observed ==> out->Observed_0.owner_view() == capture.owner_view()
                && out->Observed_0.kind_view() == capture.kind_view())
    {
        if capture.eof && !capture.failed {
            PipeReadback::Observed(SealedPipe { owner: capture.owner, kind: capture.kind, bytes: capture.bytes })
        } else { PipeReadback::Unavailable(capture.bytes) }
    }
}

sail_verified! {
    /// Check the original endpoint lease and exact stdout/stderr role before completion.
    pub fn pipe_readback_matches<Owner, Identity>(identity: &Identity, readback: &PipeReadback<Owner>, owner: &Owner, kind: u8) -> (matches: bool)
        where (Identity: OwnerIdentity<Owner>)
        requires (true) ensures (matches == pipe_matches::<Owner, Identity>(*readback, *owner, kind))
    {
        match readback {
            PipeReadback::Observed(pipe) => kind <= 1 && pipe.kind == kind && identity.same_owner(&pipe.owner, owner),
            PipeReadback::Unavailable(_) => false,
        }
    }
}
