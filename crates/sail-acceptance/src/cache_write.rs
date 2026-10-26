//! Exact record-write sequencing; a completed write is data, not solver-origin authority.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// Owned write image and stamp after write/metadata/close and a successful rename call.
    /// This type does not assert that the reply was established by a trusted solver.
    pub struct WriteCommit<Stamp> {
        image: Vec<u8>,
        header_len: usize,
        stamp: Stamp,
    }
}

sail_trait! {
    /// Narrow file and serialization operations for the actual cache transaction.
    /// Ghost projections record actual calls and their owned data, not destination
    /// contents, durability, globally unique stamps or historical solver origins.
    /// Generic implementations do not confer production evidence authority.
    pub trait PersistenceHost<Handle, Stamp, Failure> {
        spec {
            spec fn failure_stage(&self) -> Option<u8>;
            spec fn errors(&self) -> Seq<Failure>;
            spec fn steps(&self) -> Seq<u8>;
            spec fn header_bytes(&self) -> Seq<u8>;
            spec fn image(handle: &Handle) -> Seq<u8>;
            spec fn position(handle: &Handle) -> nat;
            spec fn closed_image(&self) -> Option<Seq<u8>>;
            spec fn rename_requests(&self) -> Seq<Seq<u8>>;
        }
        /// Exclusively creates a new temporary file, refusing every existing name.
        /// # Errors
        /// Returns the actual operation failure.
        fn create(&mut self) -> (result: Result<Handle, Failure>)
            requires (true)
            ensures (final(self).errors() == match result { Ok(_) => old(self).errors(), Err(error) => old(self).errors().push(error) },
                final(self).failure_stage() == old(self).failure_stage(),
                match old(self).failure_stage() { Some(stage) => result.is_ok() == (stage != 1u8), None => true },
                final(self).header_bytes() == old(self).header_bytes(),
                final(self).steps() == old(self).steps().push(if result.is_ok() { 0u8 } else { 1u8 }),
                final(self).closed_image() == old(self).closed_image(),
                final(self).rename_requests() == old(self).rename_requests(),
                match result { Ok(handle) => Self::image(&handle) == Seq::<u8>::empty() && Self::position(&handle) == 0, Err(_) => true });
        /// Serializes the original metadata header.
        /// # Errors
        /// Returns the actual operation failure.
        fn header(&mut self) -> (result: Result<Vec<u8>, Failure>)
            requires (true)
            ensures (final(self).errors() == match result { Ok(_) => old(self).errors(), Err(error) => old(self).errors().push(error) },
                final(self).failure_stage() == old(self).failure_stage(),
                match old(self).failure_stage() { Some(stage) => result.is_ok() == (stage != 3u8), None => true },
                final(self).header_bytes() == old(self).header_bytes(),
                final(self).steps() == old(self).steps().push(if result.is_ok() { 2u8 } else { 3u8 }),
                final(self).closed_image() == old(self).closed_image(),
                final(self).rename_requests() == old(self).rename_requests(),
                match result { Ok(bytes) => bytes@ == old(self).header_bytes(), Err(_) => true });
        /// Writes the complete image to this owned file.
        /// # Errors
        /// Returns the actual operation failure.
        fn write_all(&mut self, file: &mut Handle, bytes: &[u8]) -> (result: Result<(), Failure>)
            requires (Self::image(&*old(file)) == Seq::<u8>::empty(), Self::position(&*old(file)) == 0)
            ensures (final(self).errors() == match result { Ok(_) => old(self).errors(), Err(error) => old(self).errors().push(error) },
                final(self).failure_stage() == old(self).failure_stage(),
                match old(self).failure_stage() { Some(stage) => result.is_ok() == (stage != 5u8), None => true },
                final(self).header_bytes() == old(self).header_bytes(),
                final(self).steps() == old(self).steps().push(if result.is_ok() { 4u8 } else { 5u8 }),
                final(self).closed_image() == old(self).closed_image(),
                final(self).rename_requests() == old(self).rename_requests(),
                match result { Ok(_) => Self::image(&*final(file)) == bytes@ && Self::position(&*final(file)) == bytes.len(), Err(_) => exists|n: int| 0 <= n <= bytes.len() && Self::image(&*final(file)) == bytes@.take(n) && Self::position(&*final(file)) == n });
        /// Observes metadata on the same owned descriptor; stamps are not content hashes.
        /// # Errors
        /// Returns the actual operation failure.
        fn stamp(&mut self, file: &Handle) -> (result: Result<Stamp, Failure>)
            requires (true)
            ensures (final(self).errors() == match result { Ok(_) => old(self).errors(), Err(error) => old(self).errors().push(error) },
                final(self).failure_stage() == old(self).failure_stage(),
                match old(self).failure_stage() { Some(stage) => result.is_ok() == (stage != 7u8), None => true },
                final(self).header_bytes() == old(self).header_bytes(),
                final(self).steps() == old(self).steps().push(if result.is_ok() { 6u8 } else { 7u8 }),
                final(self).closed_image() == old(self).closed_image(),
                final(self).rename_requests() == old(self).rename_requests());
        /// Closes this handle before any rename.
        fn close(&mut self, file: Handle) -> (result: ())
            requires (true)
            ensures (final(self).errors() == old(self).errors(),
                final(self).failure_stage() == old(self).failure_stage(),
                final(self).header_bytes() == old(self).header_bytes(),
                final(self).steps() == old(self).steps().push(8u8),
                final(self).rename_requests() == old(self).rename_requests(),
                final(self).closed_image() == Some(Self::image(&file)));
        /// Calls rename after closing the owned handle. The associated image in
        /// the call history does not assert which object the pathname denotes.
        /// # Errors
        /// Returns the actual operation failure.
        fn rename(&mut self) -> (result: Result<(), Failure>)
            requires (self.closed_image().is_some())
            ensures (final(self).errors() == match result { Ok(_) => old(self).errors(), Err(error) => old(self).errors().push(error) },
                final(self).failure_stage() == old(self).failure_stage(),
                match old(self).failure_stage() { Some(stage) => result.is_ok() == (stage != 10u8), None => true },
                final(self).header_bytes() == old(self).header_bytes(),
                final(self).steps() == old(self).steps().push(if result.is_ok() { 9u8 } else { 10u8 }),
                final(self).closed_image() == old(self).closed_image(),
                match result { Ok(_) => final(self).rename_requests() == old(self).rename_requests().push(old(self).closed_image().unwrap()), Err(_) => final(self).rename_requests() == old(self).rename_requests() });
        /// Attempts cleanup of an unrequests temporary name.
        fn remove(&mut self) -> (result: ())
            requires (true)
            ensures (final(self).errors() == old(self).errors(),
                final(self).failure_stage() == old(self).failure_stage(),
                final(self).header_bytes() == old(self).header_bytes(),
                final(self).steps() == old(self).steps().push(11u8),
                final(self).closed_image() == old(self).closed_image(),
                final(self).rename_requests() == old(self).rename_requests());
        /// Constructs the existing malformed-header error without changing any I/O state.
        fn invalid_header(&mut self) -> (result: Failure)
            requires (true)
            ensures (final(self).failure_stage() == old(self).failure_stage(),
                final(self).errors() == old(self).errors().push(result),
                final(self).steps() == old(self).steps(),
                final(self).header_bytes() == old(self).header_bytes(),
                final(self).closed_image() == old(self).closed_image(),
                final(self).rename_requests() == old(self).rename_requests());
    }
}

sail_code! {
    impl<Stamp> WriteCommit<Stamp> {
        sail_impl_spec! {
            pub closed spec fn bytes_view(&self) -> Seq<u8> { self.image@ }
            pub closed spec fn header_length(&self) -> nat { self.header_len as nat }
            pub closed spec fn stamp_view(&self) -> Stamp { self.stamp }
        }
        sail_method! {
            /// Borrows the exact completed image without implying solver provenance.
            pub fn bytes(&self) -> (out: &[u8])
                requires (true) ensures (out@ == self.bytes_view())
            { &self.image }
        }
        sail_method! {
            /// The serialized header length in the exact completed image.
            pub fn header_size(&self) -> (out: usize)
                requires (true) ensures (out == self.header_length())
            { self.header_len }
        }
        sail_method! {
            /// Consumes this receipt and returns only the originally observed file stamp.
            pub fn into_stamp(self) -> (out: Stamp)
                requires (true) ensures (out == self.stamp_view())
            { self.stamp }
        }
    }
}

sail_verified! {
    /// Executes the actual open/encode/write/stamp/close/rename transaction.
    /// Cleanup and failure do not grant a completed-write receipt.
    /// # Errors
    /// Returns the original operation failure, preserving its priority.
    #[allow(clippy::question_mark, reason = "The explicit create-failure branch is the reviewed boundary forbidding cleanup of an unowned existing name.")]
    pub fn write_record<Host, Handle, Stamp, Failure>(host: &mut Host, reply: &[u8])
        -> (result: Result<WriteCommit<Stamp>, Failure>)
        where (Host: PersistenceHost<Handle, Stamp, Failure>)
        requires (true)
        ensures (final(host).failure_stage() == old(host).failure_stage(),
            final(host).header_bytes() == old(host).header_bytes(),
            old(host).failure_stage() == Some(0u8) && crate::query::record_header(old(host).header_bytes()) ==> result.is_ok(),
            !crate::query::record_header(old(host).header_bytes()) ==> result.is_err(),
            final(host).errors() == match result { Ok(_) => old(host).errors(), Err(error) => old(host).errors().push(error) },
            matches!(old(host).failure_stage(),Some(1u8) | Some(3u8) | Some(5u8) | Some(7u8) | Some(10u8)) ==> result.is_err(),
            old(host).failure_stage() == Some(1u8) ==> final(host).steps() == old(host).steps().push(1u8),
            old(host).failure_stage().is_some() && old(host).failure_stage() != Some(1u8) && result.is_err()
                ==> final(host).steps().last() == 11u8,
            match result {
                Ok(done) => done.bytes_view() == old(host).header_bytes() + seq![b'\n'] + reply@
                    && done.header_length() == old(host).header_bytes().len()
                    && final(host).steps() == old(host).steps() + seq![0u8,2u8,4u8,6u8,8u8,9u8]
                    && final(host).rename_requests() == old(host).rename_requests().push(done.bytes_view()),
                Err(_) => final(host).steps().len() > old(host).steps().len()
                    && (final(host).steps() == old(host).steps().push(1u8) || final(host).steps().last() == 11u8)
                    && final(host).rename_requests() == old(host).rename_requests(),
            })
    {
        let mut file = match host.create() {
            Ok(file) => file,
            Err(error) => return Err(error),
        };
        let header = match host.header() {
            Ok(header) => header,
            Err(error) => { host.close(file); host.remove(); return Err(error); }
        };
        let header_len = header.len();
        let Some(image) = crate::query::encode_record(header,reply) else {
            let error = host.invalid_header();
            host.close(file); host.remove(); return Err(error);
        };
        if let Err(error) = host.write_all(&mut file,&image) {
            host.close(file); host.remove(); return Err(error);
        }
        let observed = host.stamp(&file);
        host.close(file);
        let stamp = match observed {
            Ok(stamp) => stamp,
            Err(error) => { host.remove(); return Err(error); }
        };
        if let Err(error) = host.rename() {
            host.remove(); return Err(error);
        }
        Ok(WriteCommit { image, header_len, stamp })
    }
}

sail_code! {
    /// Executable primitive provider for controls; never selected for disk I/O.
    pub struct WriteScript {
        header: Vec<u8>,
        errors: Vec<u8>,
        steps: Vec<u8>,
        closed: Option<Vec<u8>>,
        requests: Vec<Vec<u8>>,
        fail_at: u8,
    }
    impl PersistenceHost<Vec<u8>, u64, u8> for WriteScript {
        sail_trait_impl_spec! {
            closed spec fn failure_stage(&self) -> Option<u8> { Some(self.fail_at) }
            closed spec fn errors(&self) -> Seq<u8> { self.errors@ }
            closed spec fn steps(&self) -> Seq<u8> { self.steps@ }
            closed spec fn header_bytes(&self) -> Seq<u8> { self.header@ }
            open spec fn image(handle: &Vec<u8>) -> Seq<u8> { handle@ }
            open spec fn position(handle: &Vec<u8>) -> nat { handle@.len() }
            closed spec fn closed_image(&self) -> Option<Seq<u8>> {
                match &self.closed { Some(image) => Some(image@), None => None }
            }
            closed spec fn rename_requests(&self) -> Seq<Seq<u8>> {
                Seq::new(self.requests@.len(), |i: int| self.requests@[i]@)
            }
        }
        #[allow(clippy::bool_to_int_with_if, reason = "Explicit branches expose the scripted primitive's exact event codes to stock verification.")]
        fn create(&mut self) -> Result<Vec<u8>,u8> {
            let failed = self.fail_at == 1;
            self.steps.push(if failed { 1 } else { 0 });
            if failed { self.errors.push(1); Err(1) } else { Ok(Vec::new()) }
        }
        fn header(&mut self) -> Result<Vec<u8>,u8> {
            let failed = self.fail_at == 3;
            self.steps.push(if failed { 3 } else { 2 });
            if failed { self.errors.push(3); Err(3) } else { Ok(self.header.clone()) }
        }
        fn write_all(&mut self, file: &mut Vec<u8>, bytes: &[u8]) -> Result<(),u8> {
            let failed = self.fail_at == 5;
            self.steps.push(if failed { 5 } else { 4 });
            if failed {
                let amount = bytes.len() / 2;
                file.extend_from_slice(&bytes[..amount]);
                self.errors.push(5);
                Err(5)
            } else {
                file.extend_from_slice(bytes);
                Ok(())
            }
        }
        fn stamp(&mut self, file: &Vec<u8>) -> Result<u64,u8> {
            let failed = self.fail_at == 7;
            self.steps.push(if failed { 7 } else { 6 });
            if failed { self.errors.push(7); Err(7) } else { Ok(file.len() as u64) }
        }
        fn close(&mut self, file: Vec<u8>) {
            self.steps.push(8);
            self.closed = Some(file);
        }
        fn rename(&mut self) -> Result<(),u8> {
            let failed = self.fail_at == 10;
            self.steps.push(if failed { 10 } else { 9 });
            if failed { self.errors.push(10); Err(10) } else {
                let image = match &self.closed { Some(image) => image.clone(), None => Vec::new() };
                self.requests.push(image);
                Ok(())
            }
        }
        fn remove(&mut self) { self.steps.push(11); }
        fn invalid_header(&mut self) -> u8 { self.errors.push(254); 254 }
    }
}

sail_verified! {
    /// One real record image is fully written and renamed by the checked transaction.
    #[allow(clippy::byte_char_slices, clippy::vec_init_then_push, reason = "Explicit byte and push effects construct the checked nonempty record witness.")]
    pub fn successful_write_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let mut header = Vec::new(); header.push(b'{'); header.push(b'}');
        let reply = [b'u',b'n',b's',b'a',b't',b'\n'];
        let mut host = WriteScript { header, errors: Vec::new(), steps: Vec::new(), closed: None, requests: Vec::new(), fail_at: 0 };
        sail_proof! { assert(crate::query::record_header(host.header@)); }
        match write_record::<_, Vec<u8>, u64, u8>(&mut host,&reply) {
            Ok(commit) => {
                let expected = [b'{',b'}',b'\n',b'u',b'n',b's',b'a',b't',b'\n'];
                sail_proof! { assert(commit.bytes_view() =~= expected@); }
                crate::inputs::byte_eq(commit.bytes(),&expected)
            }
            Err(_) => false,
        }
    }
}

sail_verified! {
    /// Failures clean only an owned temporary file, without a completed rename call.
    pub fn failed_write_control(stage: u8) -> (passed: bool)
        requires (matches!(stage,1u8 | 3u8 | 5u8 | 7u8 | 10u8))
        ensures (passed)
    {
        let mut host = WriteScript { header: Vec::new(), errors: Vec::new(), steps: Vec::new(), closed: None, requests: Vec::new(), fail_at: stage };
        sail_proof! {
            reveal(<WriteScript as PersistenceHost<Vec<u8>,u64,u8>>::rename_requests);
            assert(host.rename_requests() == Seq::<Seq<u8>>::empty());
        }
        let outcome = write_record::<_, Vec<u8>, u64, u8>(&mut host,b"unsat\n");
        if outcome.is_ok() { return false; }
        sail_proof! { assert(host.steps@.len() > 0); assert(stage == 1u8 || host.steps@.last() == 11u8); assert(host.rename_requests().len() == host.requests@.len()); assert(host.requests@.len() == 0); }
        let Some(step) = host.steps.last() else { return false; };
        host.requests.is_empty() && *step == (if stage == 1 { 1 } else { 11 })
    }
}

sail_verified! {
    /// A malformed serialized header refuses writing and cleans the newly owned file.
    #[allow(clippy::vec_init_then_push, reason = "The explicit push exposes the malformed separator as a constructive verifier witness.")]
    pub fn malformed_write_header_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let mut header = Vec::new(); header.push(b'\n');
        let mut host = WriteScript { header, errors: Vec::new(), steps: Vec::new(), closed: None, requests: Vec::new(), fail_at: 0 };
        sail_proof! {
            reveal(<WriteScript as PersistenceHost<Vec<u8>,u64,u8>>::rename_requests);
            assert(host.header@[0] == b'\n');
            if crate::query::record_header(host.header@) { assert(host.header@[0] != b'\n'); }
            assert(!crate::query::record_header(host.header@));
            assert(host.rename_requests() == Seq::<Seq<u8>>::empty());
        }
        let outcome = write_record::<_,Vec<u8>,u64,u8>(&mut host,b"unsat\n");
        if outcome.is_ok() { return false; }
        sail_proof! { assert(host.rename_requests().len() == host.requests@.len()); }
        host.requests.is_empty()
    }
}
