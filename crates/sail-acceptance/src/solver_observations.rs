//! Source-shared launch inventory; the host owns only low-level process and I/O primitives.
//!
//! No report count, stream correctness bit or purported historical solver origin
//! constructs this inventory. Every launch call appends its actual command first.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// Actual executable/argument bytes about to be passed to spawn.
    #[derive(Debug)]
    pub struct LaunchSpec {
        /// Exact executable bytes passed to the native spawn primitive.
        pub executable: Vec<u8>,
        /// Exact ordered argument bytes, with no normalization.
        pub arguments: Vec<Vec<u8>>,
        /// Explicit solver-family tag, distinct from executable identity.
        pub solver: Vec<u8>,
    }

    /// Actual successful wait observation; normal need not mean zero exit.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WaitObservation {
        /// Whether the actual wait returned a normal exit code.
        pub normal: bool,
        /// Actual signed exit code; interpreted only when normal is true.
        pub code: i32,
    }

    #[derive(Debug)]
    struct LaunchSlot {
        command: LaunchSpec,
        pid: Option<u32>,
        waited: Option<WaitObservation>,
    }

    /// Owned append-only command inventory and checked per-launch lifecycle.
    #[derive(Debug)]
    pub struct Inventory {
        slots: Vec<LaunchSlot>,
        healthy: bool,
        ended: bool,
    }

    /// An immutable terminal snapshot. A closed process is not a clean publisher.
    #[derive(Debug)]
    pub struct Terminal {
        attempts: usize,
        started: usize,
        closed: usize,
        complete: bool,
    }
}

sail_spec! {
    spec fn slot_started(slot: LaunchSlot) -> bool {
        slot.pid.is_some() && slot.pid.unwrap() != 0
    }
    spec fn slot_closed(slot: LaunchSlot) -> bool {
        slot_started(slot) && slot.waited.is_some()
    }
    spec fn slot_clean(slot: LaunchSlot) -> bool {
        slot_closed(slot) && slot.waited.unwrap().normal && slot.waited.unwrap().code == 0
    }
}

sail_code! {
    impl Inventory {
        sail_impl_spec! {
            pub closed spec fn commands(&self) -> Seq<LaunchSpec> {
                self.slots@.map_values(|slot: LaunchSlot| slot.command)
            }
            pub closed spec fn pid_at(&self, id: int) -> Option<u32> { self.slots@[id].pid }
            pub closed spec fn wait_at(&self, id: int) -> Option<WaitObservation> { self.slots@[id].waited }
            pub closed spec fn healthy(&self) -> bool { self.healthy }
            pub closed spec fn ended(&self) -> bool { self.ended }
            pub closed spec fn started_at(&self, id: int) -> bool { slot_started(self.slots@[id]) }
            pub closed spec fn closed_at(&self, id: int) -> bool { slot_closed(self.slots@[id]) }
            pub closed spec fn clean_at(&self, id: int) -> bool { slot_clean(self.slots@[id]) }
        }

        sail_method! {
            /// Begins an empty inventory; zero launches still require an explicit finish.
            #[allow(clippy::new_without_default, reason = "Only the checked explicit constructor establishes the empty inventory state.") ]
            pub fn new() -> (out: Self)
                requires (true)
                ensures (out.commands().len() == 0, out.healthy(), !out.ended(), out.replay_view() == empty_replay())
            { sail_proof! { reveal(Inventory::replay_view); } Self { slots: Vec::new(), healthy: true, ended: false } }
        }

        sail_method! {
            /// Counts the actual call even after a journal write failed.
            /// A late call after terminal invalidates the mutable inventory.
            pub fn intent(&mut self, command: LaunchSpec) -> (id: usize)
                requires (true)
                ensures (id == old(self).commands().len(),
                    final(self).commands() == old(self).commands().push(command),
                    final(self).replay_view().0 == old(self).replay_view().0.push(launch_bytes(command)),
                    final(self).replay_view().1 == old(self).replay_view().1.push(None),
                    final(self).replay_view().2 == old(self).replay_view().2.push(None),
                    final(self).pid_at(id as int).is_none(), final(self).wait_at(id as int).is_none(),
                    final(self).ended() == old(self).ended(),
                    final(self).healthy() == (old(self).healthy() && !old(self).ended()),
                    forall|i: int| 0 <= i < old(self).commands().len() ==>
                        final(self).pid_at(i) == old(self).pid_at(i) && final(self).wait_at(i) == old(self).wait_at(i))
            {
                let id = self.slots.len();
                if self.ended { self.healthy = false; }
                self.slots.push(LaunchSlot { command, pid: None, waited: None });
                sail_proof! {
                    reveal(Inventory::replay_view);
                    assert(self.replay_view().0 =~= old(self).replay_view().0.push(launch_bytes(command)));
                    assert(self.replay_view().1 =~= old(self).replay_view().1.push(None));
                    assert(self.replay_view().2 =~= old(self).replay_view().2.push(None));
                }
                id
            }
        }

        sail_method! {
            /// Borrows the exact command already recorded for this launch call.
            fn command(&self, id: usize) -> (command: &LaunchSpec)
                requires (id < self.commands().len())
                ensures (*command == self.commands()[id as int])
            { &self.slots[id].command }
        }

        sail_method! {
            /// A failed low-level observation is permanent; counts remain intact.
            pub fn poison(&mut self) -> (out: ())
                requires (true)
                ensures (!final(self).healthy(), final(self).commands() == old(self).commands(),
                    final(self).ended() == old(self).ended(),
                    forall|i: int| 0 <= i < old(self).commands().len() ==>
                        final(self).pid_at(i) == old(self).pid_at(i) && final(self).wait_at(i) == old(self).wait_at(i))
            { self.healthy = false; }
        }

        sail_method! {
            /// Records only the actual returned child PID for a fresh intent.
            pub fn started(&mut self, id: usize, pid: u32) -> (recorded: bool)
                requires (true)
                ensures (final(self).commands() == old(self).commands(), final(self).ended() == old(self).ended(),
                    recorded == (!old(self).ended() && id < old(self).commands().len()
                        && old(self).pid_at(id as int).is_none() && pid != 0),
                    recorded ==> final(self).replay_view() == ((old(self).replay_view().0, old(self).replay_view().1.update(id as int, Some(pid)), old(self).replay_view().2)),
                    recorded ==> final(self).pid_at(id as int) == Some(pid)
                        && final(self).healthy() == old(self).healthy(),
                    !recorded ==> !final(self).healthy(),
                    forall|i: int| 0 <= i < old(self).commands().len() ==>
                        final(self).wait_at(i) == old(self).wait_at(i)
                            && (recorded && i == id || final(self).pid_at(i) == old(self).pid_at(i)))
            {
                if self.ended || id >= self.slots.len() || pid == 0 {
                    self.healthy = false;
                    return false;
                }
                let slot = &mut self.slots[id];
                if slot.pid.is_some() {
                    self.healthy = false;
                    return false;
                }
                slot.pid = Some(pid);
                sail_proof! {
                    reveal(Inventory::replay_view);
                    assert(self.replay_view().0 =~= old(self).replay_view().0);
                    assert(self.replay_view().1 =~= old(self).replay_view().1.update(id as int, Some(pid)));
                    assert(self.replay_view().2 =~= old(self).replay_view().2);
                }
                true
            }
        }

        sail_method! {
            /// Ties an actual wait result to the same owned child and stream.
            pub fn closed(&mut self, id: usize, pid: u32, status: WaitObservation) -> (recorded: bool)
                requires (true)
                ensures (final(self).commands() == old(self).commands(), final(self).ended() == old(self).ended(),
                    recorded == (!old(self).ended() && id < old(self).commands().len() && pid != 0
                        && old(self).pid_at(id as int) == Some(pid) && old(self).wait_at(id as int).is_none()),
                    recorded ==> final(self).replay_view() == ((old(self).replay_view().0, old(self).replay_view().1, old(self).replay_view().2.update(id as int, Some(status)))),
                    recorded ==> final(self).wait_at(id as int) == Some(status)
                        && final(self).healthy() == old(self).healthy(),
                    !recorded ==> !final(self).healthy(),
                    forall|i: int| 0 <= i < old(self).commands().len() ==>
                        final(self).pid_at(i) == old(self).pid_at(i)
                            && (recorded && i == id || final(self).wait_at(i) == old(self).wait_at(i)))
            {
                if self.ended || id >= self.slots.len() || pid == 0 {
                    self.healthy = false;
                    return false;
                }
                let slot = &mut self.slots[id];
                if slot.pid != Some(pid) || slot.waited.is_some() {
                    self.healthy = false;
                    return false;
                }
                slot.waited = Some(status);
                sail_proof! {
                    reveal(Inventory::replay_view);
                    assert(self.replay_view().0 =~= old(self).replay_view().0);
                    assert(self.replay_view().1 =~= old(self).replay_view().1);
                    assert(self.replay_view().2 =~= old(self).replay_view().2.update(id as int, Some(status)));
                }
                true
            }
        }
    }
}

sail_spec! {
    spec fn started_count(slots: Seq<LaunchSlot>) -> nat
        decreases slots.len(),
    {
        if slots.len() == 0 { 0 } else { started_count(slots.drop_last()) + if slot_started(slots.last()) { 1nat } else { 0nat } }
    }
    spec fn closed_count(slots: Seq<LaunchSlot>) -> nat
        decreases slots.len(),
    {
        if slots.len() == 0 { 0 } else { closed_count(slots.drop_last()) + if slot_closed(slots.last()) { 1nat } else { 0nat } }
    }
}

sail_code! {
    impl Inventory {
        sail_impl_spec! {
            pub closed spec fn started_count(&self) -> nat { started_count(self.slots@) }
            pub closed spec fn closed_count(&self) -> nat { closed_count(self.slots@) }
        }
        sail_method! {
            /// An explicit terminal, including a genuine zero-launch run.
            /// Counts derive from the owned slots, not reports or supplied counts.
            pub fn finish(&mut self) -> (terminal: Terminal)
                requires (true)
                ensures (terminal.attempts_view() == old(self).commands().len(),
                    final(self).replay_view() == old(self).replay_view(),
                    terminal.started_view() == old(self).started_count(),
                    terminal.closed_view() == old(self).closed_count(),
                    terminal.complete_view() == (old(self).healthy() && !old(self).ended()
                        && forall|i: int| 0 <= i < old(self).commands().len() ==> old(self).closed_at(i)),
                    terminal.complete_view() ==> (forall|i: int| 0 <= i < final(self).commands().len() ==> final(self).closed_at(i)),
                    terminal.complete_view() ==> terminal.started_view() == terminal.attempts_view()
                        && terminal.closed_view() == terminal.attempts_view(),
                    final(self).ended(), final(self).healthy() == (old(self).healthy() && !old(self).ended()),
                    final(self).commands() == old(self).commands(),
                    forall|i: int| 0 <= i < old(self).commands().len() ==>
                        final(self).pid_at(i) == old(self).pid_at(i) && final(self).wait_at(i) == old(self).wait_at(i))
            {
                let mut started = 0usize;
                let mut closed = 0usize;
                let mut complete = self.healthy && !self.ended;
                let mut i = 0usize;
                sail_while! { (i < self.slots.len())
                    invariant (i <= self.slots.len(), closed <= started <= i,
                        started == started_count(self.slots@.take(i as int)),
                        closed == closed_count(self.slots@.take(i as int)),
                        self.slots@ == old(self).slots@, self.healthy == old(self).healthy(), self.ended == old(self).ended(),
                        complete == (old(self).healthy() && !old(self).ended()
                            && forall|j: int| 0 <= j < i ==> old(self).closed_at(j)),
                        complete ==> started == i && closed == i)
                    decreases (self.slots.len() - i)
                    {
                        let slot = &self.slots[i];
                        let was_started = slot.pid.is_some() && slot.pid != Some(0);
                        let was_closed = was_started && slot.waited.is_some();
                        let previously_complete = complete;
                        if was_started { started += 1; }
                        if was_closed { closed += 1; }
                        complete = previously_complete && was_closed;
                        sail_proof! {
                            assert(self.slots@.take(i as int + 1).drop_last() =~= self.slots@.take(i as int));
                            assert(self.slots@.take(i as int + 1).last() == self.slots@[i as int]);
                            reveal_with_fuel(started_count, 1);
                            reveal_with_fuel(closed_count, 1);
                            assert(was_closed == old(self).closed_at(i as int));
                            if complete {
                                assert forall|j: int| 0 <= j < i + 1 implies old(self).closed_at(j) by {
                                    if j == i { assert(was_closed); }
                                }
                            } else if old(self).healthy() && !old(self).ended()
                                && forall|j: int| 0 <= j < i + 1 ==> old(self).closed_at(j) {
                                assert forall|j: int| 0 <= j < i implies old(self).closed_at(j) by {}
                                assert(previously_complete);
                                assert(was_closed);
                            }
                        }
                        i += 1;
                    }
                }
                sail_proof! { assert(self.slots@.take(self.slots.len() as int) =~= self.slots@); }
                if self.ended { self.healthy = false; }
                self.ended = true;
                sail_proof! {
                    reveal(Inventory::replay_view); reveal(Inventory::closed_at); reveal(Inventory::commands);
                    if complete {
                        assert forall|j: int| 0 <= j < self.commands().len() implies self.closed_at(j) by {
                            assert(old(self).closed_at(j));
                        }
                    }
                }
                Terminal { attempts: self.slots.len(), started, closed, complete }
            }
        }
    }
}

sail_code! {
    impl Terminal {
        sail_impl_spec! {
            pub closed spec fn attempts_view(&self) -> nat { self.attempts as nat }
            pub closed spec fn started_view(&self) -> nat { self.started as nat }
            pub closed spec fn closed_view(&self) -> nat { self.closed as nat }
            pub closed spec fn complete_view(&self) -> bool { self.complete }
        }
        sail_method! {
            /// Returns the number of actual attempted launches in this snapshot.
            pub fn attempts(&self) -> (out: usize)
                requires (true) ensures (out == self.attempts_view())
            { self.attempts }
        }
        sail_method! {
            /// Returns the number of launches with an observed nonzero PID.
            pub fn started(&self) -> (out: usize)
                requires (true) ensures (out == self.started_view())
            { self.started }
        }
        sail_method! {
            /// Returns the number of launches with a matching observed wait.
            pub fn closed(&self) -> (out: usize)
                requires (true) ensures (out == self.closed_view())
            { self.closed }
        }
        sail_method! {
            /// Reports whether all recorded launches closed without a prior observation failure.
            pub fn complete(&self) -> (out: bool)
                requires (true) ensures (out == self.complete_view())
            { self.complete }
        }
    }
}

sail_verified! {
    /// Appends one checked little-endian protocol word to the actual record bytes.
    #[allow(clippy::cast_possible_truncation, reason = "Each byte intentionally selects one checked little-endian octet.") ]
    pub fn append_word(bytes: &mut Vec<u8>, word: u64) -> (out: ())
        requires (true)
        ensures (final(bytes)@ == old(bytes)@ + vstd::bytes::spec_u64_to_le_bytes(word))
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
        bytes.extend_from_slice(&encoded);
    }
}

sail_verified! {
    /// Reads exactly one bounded word and advances only after all eight bytes exist.
    #[allow(clippy::cast_lossless, reason = "Explicit widening casts match the checked bit-vector assembly.") ]
    pub fn read_word(bytes: &[u8], offset: &mut usize) -> (word: Option<u64>)
        requires (*old(offset) <= bytes.len())
        ensures (word.is_some() == (*old(offset) as int + 8 <= bytes@.len()),
            word.is_some() ==> *final(offset) == *old(offset) + 8
                && word.unwrap() == vstd::bytes::spec_u64_from_le_bytes(bytes@.subrange(*old(offset) as int, *old(offset) as int + 8)),
            word.is_none() ==> *final(offset) == *old(offset))
    {
        let start = *offset;
        if bytes.len() - start < 8 { return None; }
        let b0 = bytes[start]; let b1 = bytes[start + 1]; let b2 = bytes[start + 2]; let b3 = bytes[start + 3];
        let b4 = bytes[start + 4]; let b5 = bytes[start + 5]; let b6 = bytes[start + 6]; let b7 = bytes[start + 7];
        let word = (b0 as u64) | ((b1 as u64) << 8) | ((b2 as u64) << 16) | ((b3 as u64) << 24)
            | ((b4 as u64) << 32) | ((b5 as u64) << 40) | ((b6 as u64) << 48) | ((b7 as u64) << 56);
        sail_proof! {
            assert(word == ((b0 as u64) | ((b1 as u64) << 8) | ((b2 as u64) << 16) | ((b3 as u64) << 24) | ((b4 as u64) << 32) | ((b5 as u64) << 40) | ((b6 as u64) << 48) | ((b7 as u64) << 56)) ==> (word & 255u64) as u8 == b0) by (bit_vector);
            assert(word == ((b0 as u64) | ((b1 as u64) << 8) | ((b2 as u64) << 16) | ((b3 as u64) << 24) | ((b4 as u64) << 32) | ((b5 as u64) << 40) | ((b6 as u64) << 48) | ((b7 as u64) << 56)) ==> ((word >> 8) & 255u64) as u8 == b1) by (bit_vector);
            assert(word == ((b0 as u64) | ((b1 as u64) << 8) | ((b2 as u64) << 16) | ((b3 as u64) << 24) | ((b4 as u64) << 32) | ((b5 as u64) << 40) | ((b6 as u64) << 48) | ((b7 as u64) << 56)) ==> ((word >> 16) & 255u64) as u8 == b2) by (bit_vector);
            assert(word == ((b0 as u64) | ((b1 as u64) << 8) | ((b2 as u64) << 16) | ((b3 as u64) << 24) | ((b4 as u64) << 32) | ((b5 as u64) << 40) | ((b6 as u64) << 48) | ((b7 as u64) << 56)) ==> ((word >> 24) & 255u64) as u8 == b3) by (bit_vector);
            assert(word == ((b0 as u64) | ((b1 as u64) << 8) | ((b2 as u64) << 16) | ((b3 as u64) << 24) | ((b4 as u64) << 32) | ((b5 as u64) << 40) | ((b6 as u64) << 48) | ((b7 as u64) << 56)) ==> ((word >> 32) & 255u64) as u8 == b4) by (bit_vector);
            assert(word == ((b0 as u64) | ((b1 as u64) << 8) | ((b2 as u64) << 16) | ((b3 as u64) << 24) | ((b4 as u64) << 32) | ((b5 as u64) << 40) | ((b6 as u64) << 48) | ((b7 as u64) << 56)) ==> ((word >> 40) & 255u64) as u8 == b5) by (bit_vector);
            assert(word == ((b0 as u64) | ((b1 as u64) << 8) | ((b2 as u64) << 16) | ((b3 as u64) << 24) | ((b4 as u64) << 32) | ((b5 as u64) << 40) | ((b6 as u64) << 48) | ((b7 as u64) << 56)) ==> ((word >> 48) & 255u64) as u8 == b6) by (bit_vector);
            assert(word == ((b0 as u64) | ((b1 as u64) << 8) | ((b2 as u64) << 16) | ((b3 as u64) << 24) | ((b4 as u64) << 32) | ((b5 as u64) << 40) | ((b6 as u64) << 48) | ((b7 as u64) << 56)) ==> ((word >> 56) & 255u64) as u8 == b7) by (bit_vector);
            vstd::bytes::spec_u64_to_le_bytes_to_open(word);
            assert(vstd::bytes::spec_u64_to_le_bytes(word) =~= bytes@.subrange(start as int, start as int + 8));
            vstd::bytes::lemma_auto_spec_u64_to_from_le_bytes();
        }
        *offset = start + 8;
        Some(word)
    }
}

sail_verified! {
    /// Appends a length-delimited field without changing any payload byte.
    pub fn append_field(bytes: &mut Vec<u8>, field: &[u8]) -> (out: ())
        requires (true)
        ensures (final(bytes)@ == old(bytes)@ + vstd::bytes::spec_u64_to_le_bytes(field.len() as u64) + field@)
    {
        append_word(bytes, field.len() as u64);
        bytes.extend_from_slice(field);
    }
}

sail_verified! {
    /// Reads one complete length-delimited field; incomplete data cannot borrow a suffix.
    #[allow(clippy::cast_possible_truncation, reason = "The length is checked against the platform slice before conversion.")]
    pub fn read_field<'a>(bytes: &'a [u8], offset: &mut usize) -> (field: Option<&'a [u8]>)
        requires (*old(offset) <= bytes.len())
        ensures (field.is_some() ==> *old(offset) as int + 8 <= bytes@.len()
                && field.unwrap()@.len() == vstd::bytes::spec_u64_from_le_bytes(bytes@.subrange(*old(offset) as int, *old(offset) as int + 8))
                && *final(offset) as int == *old(offset) as int + 8 + field.unwrap()@.len()
                && field.unwrap()@ == bytes@.subrange(*old(offset) as int + 8, *final(offset) as int),
            field.is_none() ==> *final(offset) == *old(offset), *final(offset) <= bytes.len())
    {
        let start = *offset;
        let mut cursor = start;
        let length = read_word(bytes, &mut cursor)?;
        if length > (bytes.len() - cursor) as u64 { return None; }
        let end = cursor + length as usize;
        *offset = end;
        Some(&bytes[cursor..end])
    }
}

sail_spec! {
    pub open spec fn word_bytes(word: u64) -> Seq<u8> { vstd::bytes::spec_u64_to_le_bytes(word) }
    pub open spec fn field_bytes(field: Seq<u8>) -> Seq<u8> { word_bytes(field.len() as u64) + field }
    pub open spec fn argument_fields(arguments: Seq<Vec<u8>>) -> Seq<u8>
        decreases arguments.len(),
    {
        if arguments.len() == 0 { Seq::empty() }
        else { argument_fields(arguments.drop_last()) + field_bytes(arguments.last()@) }
    }
    pub open spec fn argument_bytes(arguments: Seq<Vec<u8>>) -> Seq<u8> {
        word_bytes(arguments.len() as u64) + argument_fields(arguments)
    }
}

sail_verified! {
    /// Encodes every actual argument in its original order, including empty values.
    pub fn encode_arguments(arguments: &[Vec<u8>]) -> (encoded: Vec<u8>)
        requires (true)
        ensures (encoded@ == argument_bytes(arguments@))
    {
        let mut encoded = Vec::new();
        append_word(&mut encoded, arguments.len() as u64);
        let mut i = 0usize;
        sail_while! { (i < arguments.len())
            invariant (i <= arguments.len(), encoded@ == word_bytes(arguments.len() as u64)
                + argument_fields(arguments@.take(i as int)))
            decreases (arguments.len() - i)
            {
                append_field(&mut encoded, &arguments[i]);
                sail_proof! {
                    assert(arguments@.take(i as int + 1).drop_last() =~= arguments@.take(i as int));
                    assert(arguments@.take(i as int + 1).last() == arguments@[i as int]);
                    reveal_with_fuel(argument_fields, 1);
                }
                i += 1;
            }
        }
        sail_proof! { assert(arguments@.take(arguments.len() as int) =~= arguments@); }
        encoded
    }
}

sail_verified! {
    /// Frames one immutable event body. The consumer requires complete frame EOF.
    pub fn frame_event(body: &[u8]) -> (framed: Vec<u8>)
        requires (true)
        ensures (framed@ == field_bytes(body@))
    {
        let mut framed = Vec::new();
        append_field(&mut framed, body);
        framed
    }
}

sail_verified! {
    /// Explicit versioned begin, tied to the preallocated request and actual PID.
    #[allow(clippy::cast_lossless, reason = "The fixed codec widens the actual 32-bit PID to its 64-bit field.") ]
    pub fn encode_begin(request: &[u8], verifier_pid: u32) -> (event: Vec<u8>)
        requires (true)
        ensures (event@ == seq![0u8, 1u8] + field_bytes(request@) + word_bytes(verifier_pid as u64))
    {
        let mut event = vec![0u8, 1u8];
        append_field(&mut event, request);
        append_word(&mut event, verifier_pid as u64);
        event
    }
}

sail_verified! {
    /// Encodes the exact command already owned by the intent ledger.
    pub fn encode_intent(inventory: &Inventory, stream: usize) -> (event: Vec<u8>)
        requires (stream < inventory.commands().len())
        ensures (event@ == seq![1u8] + word_bytes(stream as u64)
            + field_bytes(inventory.commands()[stream as int].solver@)
            + field_bytes(inventory.commands()[stream as int].executable@)
            + argument_bytes(inventory.commands()[stream as int].arguments@))
    {
        let command = inventory.command(stream);
        let mut event = vec![1u8];
        append_word(&mut event, stream as u64);
        append_field(&mut event, &command.solver);
        append_field(&mut event, &command.executable);
        let arguments = encode_arguments(&command.arguments);
        event.extend_from_slice(&arguments);
        event
    }
}

sail_verified! {
    /// Encodes the actual returned process identity without reinterpreting it.
    #[allow(clippy::cast_lossless, reason = "The fixed codec widens the actual 32-bit PID to its 64-bit field.") ]
    pub fn encode_started(stream: usize, pid: u32) -> (event: Vec<u8>)
        requires (true)
        ensures (event@ == seq![2u8] + word_bytes(stream as u64) + word_bytes(pid as u64))
    {
        let mut event = vec![2u8];
        append_word(&mut event, stream as u64);
        append_word(&mut event, pid as u64);
        event
    }
}

sail_verified! {
    /// Normal exit and its signed code remain distinct from signal termination.
    #[allow(clippy::cast_lossless, clippy::cast_sign_loss, clippy::bool_to_int_with_if, reason = "The checked codec preserves the signed exit code bit pattern and explicit flag byte.") ]
    pub fn encode_waited(stream: usize, pid: u32, status: WaitObservation) -> (event: Vec<u8>)
        requires (true)
        ensures (event@ == seq![3u8] + word_bytes(stream as u64) + word_bytes(pid as u64)
            + seq![if status.normal { 1u8 } else { 0u8 }] + word_bytes(status.code as u32 as u64))
    {
        let mut event = vec![3u8];
        append_word(&mut event, stream as u64);
        append_word(&mut event, pid as u64);
        event.push(if status.normal { 1u8 } else { 0u8 });
        append_word(&mut event, status.code as u32 as u64);
        event
    }
}

sail_verified! {
    /// All terminal counts/health come from the checked fold, including zero-SMT.
    #[allow(clippy::cast_lossless, clippy::cast_sign_loss, clippy::bool_to_int_with_if, reason = "The checked codec preserves the signed exit code bit pattern and explicit flag byte.") ]
    pub fn encode_terminal(terminal: &Terminal, verifier_exit: i32) -> (event: Vec<u8>)
        requires (true)
        ensures (event@ == seq![4u8] + word_bytes(terminal.attempts_view() as u64)
            + word_bytes(terminal.started_view() as u64) + word_bytes(terminal.closed_view() as u64)
            + seq![if terminal.complete_view() { 1u8 } else { 0u8 }] + word_bytes(verifier_exit as u32 as u64))
    {
        let mut event = vec![4u8];
        append_word(&mut event, terminal.attempts() as u64);
        append_word(&mut event, terminal.started() as u64);
        append_word(&mut event, terminal.closed() as u64);
        event.push(if terminal.complete() { 1u8 } else { 0u8 });
        append_word(&mut event, verifier_exit as u32 as u64);
        event
    }
}

sail_verified! {
    /// Copies a slice without changing any byte.
    pub fn copy_bytes(bytes: &[u8]) -> (owned: Vec<u8>)
        requires (true) ensures (owned@ == bytes@)
    {
        let mut owned = Vec::new(); let mut i = 0;
        sail_while! { (i < bytes.len()) invariant (
            i <= bytes.len(), owned@ == bytes@.take(i as int)
        ) decreases (bytes.len() - i) { owned.push(bytes[i]); i += 1; }}
        owned
    }
}
sail_spec! {
    pub open spec fn launch_bytes(command: LaunchSpec) -> (Seq<u8>, Seq<Seq<u8>>, Seq<u8>) {
        (command.executable@, command.arguments@.map_values(|v: Vec<u8>| v@), command.solver@)
    }
}
sail_verified! {
    /// Copies exact executable, solver and argument bytes.
    pub fn copy_launch(command: &LaunchSpec) -> (owned: LaunchSpec)
        requires (true) ensures (launch_bytes(owned) == launch_bytes(*command))
    {
        let mut arguments: Vec<Vec<u8>> = Vec::new(); let mut i = 0;
        sail_while! { (i < command.arguments.len()) invariant (
            i <= command.arguments.len(),
            arguments.len() == i,
            forall|j: int| 0 <= j < i ==> #[trigger] arguments@[j]@ == command.arguments@[j]@
        ) decreases (command.arguments.len() - i) {
            arguments.push(copy_bytes(&command.arguments[i])); i += 1;
        }}
        sail_proof! { assert(arguments@.map_values(|v: Vec<u8>| v@) =~= command.arguments@.map_values(|v: Vec<u8>| v@)); }
        LaunchSpec { executable: copy_bytes(&command.executable), solver: copy_bytes(&command.solver), arguments }
    }
}

sail_trait! {
    /// Narrow process and append-write effects. This promises no stream/verdict correctness.
    pub trait ProcessHost<Child, Error, Input, Output> {
        spec {
            spec fn spawn_calls(&self) -> nat;
            spec fn spawn_commands(&self) -> Seq<(Seq<u8>, Seq<Seq<u8>>, Seq<u8>)>;
            spec fn spawn_routes(&self) -> Seq<nat>;
            spec fn wait_calls(&self) -> nat;
            spec fn write_calls(&self) -> Seq<Seq<u8>>;
            spec fn failed_writes(&self) -> nat;
        }
        /// One actual append-write attempt with the exact supplied bytes.
        ///
        /// # Errors
        /// Returns the actual low-level write or flush error.
        fn write_all(&mut self, bytes: &[u8]) -> (result: Result<(), Error>)
            requires (true)
            ensures (final(self).spawn_calls() == old(self).spawn_calls(),
                final(self).spawn_commands() == old(self).spawn_commands(), final(self).spawn_routes() == old(self).spawn_routes(),
                final(self).wait_calls() == old(self).wait_calls(),
                final(self).write_calls() == old(self).write_calls().push(bytes@),
                final(self).failed_writes() == old(self).failed_writes() + if result.is_err() { 1nat } else { 0nat });
        /// One actual spawn attempt with these executable/argument bytes and routing index.
        ///
        /// # Errors
        /// Returns the actual low-level process spawn error.
        fn spawn(&mut self, command: &LaunchSpec, stream: usize) -> (result: Result<Child, Error>)
            requires (true)
            ensures (final(self).spawn_calls() == old(self).spawn_calls() + 1,
                final(self).spawn_commands() == old(self).spawn_commands().push(launch_bytes(*command)),
                final(self).spawn_routes() == old(self).spawn_routes().push(stream as nat),
                final(self).wait_calls() == old(self).wait_calls(), final(self).write_calls() == old(self).write_calls(), final(self).failed_writes() == old(self).failed_writes());
        /// Actual child identity; a zero/mismatched observation cannot close an inventory.
        fn pid(&self, child: &Child) -> (pid: u32)
            requires (true) ensures (true);
        /// Actual wait result for the same owned child handle.
        ///
        /// # Errors
        /// Returns the actual low-level child wait error.
        fn wait(&mut self, child: &mut Child) -> (result: Result<WaitObservation, Error>)
            requires (true)
            ensures (final(self).wait_calls() == old(self).wait_calls() + 1,
                final(self).spawn_calls() == old(self).spawn_calls(),
                final(self).spawn_commands() == old(self).spawn_commands(), final(self).spawn_routes() == old(self).spawn_routes(), final(self).write_calls() == old(self).write_calls(), final(self).failed_writes() == old(self).failed_writes());
        /// Taking a pipe cannot launch another process or change observation history.
        fn take_input(&mut self, child: &mut Child) -> (pipe: Option<Input>)
            requires (true)
            ensures (final(self).spawn_calls() == old(self).spawn_calls(),
                final(self).spawn_commands() == old(self).spawn_commands(), final(self).spawn_routes() == old(self).spawn_routes(),
                final(self).wait_calls() == old(self).wait_calls(), final(self).write_calls() == old(self).write_calls(), final(self).failed_writes() == old(self).failed_writes());
        /// Takes only the existing child output pipe, preserving process histories.
        fn take_output(&mut self, child: &mut Child) -> (pipe: Option<Output>)
            requires (true)
            ensures (final(self).spawn_calls() == old(self).spawn_calls(),
                final(self).spawn_commands() == old(self).spawn_commands(), final(self).spawn_routes() == old(self).spawn_routes(),
                final(self).wait_calls() == old(self).wait_calls(), final(self).write_calls() == old(self).write_calls(), final(self).failed_writes() == old(self).failed_writes());
    }
}

sail_code! {
    /// The actual returned child, its routing index and observed PID stay owned together.
    pub struct ObservedChild<Child> {
        child: Child,
        stream: usize,
        pid: u32,
    }
    /// The actual controller outcome; refusal never attempts physical spawn.
    pub enum SpawnOutcome<Child, Error> {
        /// Sealed inventory: no low-level spawn call is permitted.
        Refused,
        /// The actual low-level spawn error.
        Failed(Error),
        /// The actual returned process, retained with its immutable routing identity.
        Started(ObservedChild<Child>),
    }
}

sail_code! {
    impl<Child> ObservedChild<Child> {
        sail_impl_spec! {
            pub closed spec fn stream_view(&self) -> nat { self.stream as nat }
            pub closed spec fn pid_view(&self) -> u32 { self.pid }
            pub closed spec fn child_view(&self) -> Child { self.child }
        }
        sail_method! {
            /// Returns the immutable stream routing index.
            pub fn stream(&self) -> (stream: usize)
                requires (true) ensures (stream == self.stream_view())
            { self.stream }
        }
        sail_method! {
            /// Returns the immutable PID recorded at spawn.
            pub fn pid(&self) -> (pid: u32)
                requires (true) ensures (pid == self.pid_view())
            { self.pid }
        }
        sail_method! {
            /// Only host pipe-taking primitives can access the mutable raw process.
            pub fn pipes<Host, Error, Input, Output>(&mut self, host: &mut Host)
                -> (pipes: (Option<Input>, Option<Output>))
                where (Host: ProcessHost<Child, Error, Input, Output>)
                requires (true)
                ensures (final(self).stream_view() == old(self).stream_view(),
                    final(self).pid_view() == old(self).pid_view(),
                    final(host).spawn_calls() == old(host).spawn_calls(),
                    final(host).wait_calls() == old(host).wait_calls(), final(host).write_calls() == old(host).write_calls())
            {
                (host.take_input(&mut self.child), host.take_output(&mut self.child))
            }
        }
    }
}

sail_verified! {
    /// The actual intent/write/spawn/start/write controller, shared with the fork.
    /// Sealing forbids physical spawn, including when a later journal write would fail.
    pub fn observed_spawn<Host, Child, Error, Input, Output>(host: &mut Host,
        inventory: &mut Inventory, command: LaunchSpec) -> (result: SpawnOutcome<Child, Error>)
        where (Host: ProcessHost<Child, Error, Input, Output>)
        requires (true)
        ensures (final(inventory).commands() == old(inventory).commands().push(command),
            final(inventory).ended() == old(inventory).ended(),
            final(inventory).healthy() ==> old(inventory).healthy(),
            (result is Refused) == old(inventory).ended(),
            old(inventory).ended() ==> final(host).spawn_calls() == old(host).spawn_calls()
                && final(host).wait_calls() == old(host).wait_calls()
                && final(host).write_calls() == old(host).write_calls()
                && final(host).spawn_commands() == old(host).spawn_commands()
                && final(host).spawn_routes() == old(host).spawn_routes() && !final(inventory).healthy(),
            !old(inventory).ended() ==> final(host).spawn_calls() == old(host).spawn_calls() + 1
                && final(host).wait_calls() == old(host).wait_calls()
                && final(host).write_calls().len() >= old(host).write_calls().len() + 1
                && final(host).spawn_commands() == old(host).spawn_commands().push(launch_bytes(command))
                && final(host).spawn_routes() == old(host).spawn_routes().push(old(inventory).commands().len()),
            result is Started ==> result->Started_0.stream_view() == old(inventory).commands().len()
                && (result->Started_0.pid_view() != 0 ==>
                    final(inventory).pid_at(result->Started_0.stream_view() as int) == Some(result->Started_0.pid_view())),
            result is Failed ==> !final(inventory).healthy(),
            !old(inventory).ended() && result is Failed ==> final(host).write_calls() == old(host).write_calls().push(
                field_bytes(seq![1u8] + word_bytes(old(inventory).commands().len() as u64)
                    + field_bytes(command.solver@) + field_bytes(command.executable@) + argument_bytes(command.arguments@))),
            result is Started ==> final(host).write_calls() == old(host).write_calls().push(
                field_bytes(seq![1u8] + word_bytes(old(inventory).commands().len() as u64)
                    + field_bytes(command.solver@) + field_bytes(command.executable@) + argument_bytes(command.arguments@))).push(
                field_bytes(seq![2u8] + word_bytes(old(inventory).commands().len() as u64) + word_bytes(result->Started_0.pid_view() as u64))),
            final(inventory).healthy() ==> final(host).failed_writes() == old(host).failed_writes())
    {
        let ended = inventory.ended;
        let stream = inventory.intent(command);
        if ended { return SpawnOutcome::Refused; }
        let event = encode_intent(inventory, stream);
        let packet = frame_event(&event);
        if host.write_all(&packet).is_err() { inventory.poison(); }
        match host.spawn(inventory.command(stream), stream) {
            Ok(child) => {
                let pid = host.pid(&child);
                inventory.started(stream, pid);
                let event = encode_started(stream, pid);
                let packet = frame_event(&event);
                if host.write_all(&packet).is_err() { inventory.poison(); }
                SpawnOutcome::Started(ObservedChild { child, stream, pid })
            },
            Err(error) => { inventory.poison(); SpawnOutcome::Failed(error) },
        }
    }
}

sail_verified! {
    /// Waits on the same owned child and records its actual returned status.
    ///
    /// # Errors
    /// Returns the unchanged actual host wait error and poisons the inventory.
    pub fn observed_wait<Host, Child, Error, Input, Output>(host: &mut Host,
        inventory: &mut Inventory, child: &mut ObservedChild<Child>) -> (result: Result<WaitObservation, Error>)
        where (Host: ProcessHost<Child, Error, Input, Output>)
        requires (true)
        ensures (final(host).spawn_calls() == old(host).spawn_calls(),
            final(inventory).ended() == old(inventory).ended(),
            final(inventory).healthy() ==> old(inventory).healthy(),
            final(host).wait_calls() == old(host).wait_calls() + 1,
            final(host).write_calls().len() >= old(host).write_calls().len(),
            final(child).stream_view() == old(child).stream_view(), final(child).pid_view() == old(child).pid_view(),
            final(inventory).commands() == old(inventory).commands(),
            result.is_ok() && old(child).stream_view() < old(inventory).commands().len()
                && !old(inventory).ended() && old(child).pid_view() != 0
                && old(inventory).pid_at(old(child).stream_view() as int) == Some(old(child).pid_view())
                && old(inventory).wait_at(old(child).stream_view() as int).is_none()
                ==> final(inventory).wait_at(old(child).stream_view() as int) == Some(result->Ok_0),
            result.is_err() ==> !final(inventory).healthy(),
            result.is_err() ==> final(host).write_calls() == old(host).write_calls(),
            result.is_ok() ==> final(host).write_calls() == old(host).write_calls().push(field_bytes(
                seq![3u8] + word_bytes(old(child).stream_view() as u64) + word_bytes(old(child).pid_view() as u64)
                    + seq![if result->Ok_0.normal { 1u8 } else { 0u8 }] + word_bytes(result->Ok_0.code as u32 as u64))),
            final(inventory).healthy() ==> final(host).failed_writes() == old(host).failed_writes())
    {
        if host.pid(&child.child) != child.pid { inventory.poison(); }
        let result = host.wait(&mut child.child);
        match result {
            Ok(status) => {
                inventory.closed(child.stream, child.pid, status);
                let event = encode_waited(child.stream, child.pid, status);
                let packet = frame_event(&event);
                if host.write_all(&packet).is_err() { inventory.poison(); }
                Ok(status)
            },
            Err(error) => { inventory.poison(); Err(error) },
        }
    }
}

sail_verified! {
    /// Explicit begin precedes any launch, and records its exact request/PID bytes.
    pub fn begin_observer<Host, Child, Error, Input, Output>(host: &mut Host, request: &[u8], verifier_pid: u32)
        -> (inventory: Inventory)
        where (Host: ProcessHost<Child, Error, Input, Output>)
        requires (true)
        ensures (inventory.commands().len() == 0, !inventory.ended(),
            inventory.healthy() == (final(host).failed_writes() == old(host).failed_writes()),
            final(host).spawn_calls() == old(host).spawn_calls(), final(host).wait_calls() == old(host).wait_calls(),
            final(host).write_calls() == old(host).write_calls().push(field_bytes(
                seq![0u8, 1u8] + field_bytes(request@) + word_bytes(verifier_pid as u64))))
    {
        let mut inventory = Inventory::new();
        let event = encode_begin(request, verifier_pid);
        let packet = frame_event(&event);
        if host.write_all(&packet).is_err() { inventory.poison(); }
        inventory
    }
}

sail_verified! {
    /// Seal before emitting terminal bytes, so no later actual spawn is possible.
    /// A duplicate seal is refused without appending a misleading second terminal.
    pub fn seal_observer<Host, Child, Error, Input, Output>(host: &mut Host, inventory: &mut Inventory,
        verifier_exit: i32) -> (terminal: Option<Terminal>)
        where (Host: ProcessHost<Child, Error, Input, Output>)
        requires (true)
        ensures (terminal.is_some() == !old(inventory).ended(), final(inventory).ended(),
            final(inventory).healthy() ==> old(inventory).healthy(),
            final(host).spawn_calls() == old(host).spawn_calls(), final(host).wait_calls() == old(host).wait_calls(),
            final(inventory).commands() == old(inventory).commands(),
            terminal.is_some() ==> terminal.unwrap().attempts_view() == old(inventory).commands().len()
                && terminal.unwrap().started_view() == old(inventory).started_count()
                && terminal.unwrap().closed_view() == old(inventory).closed_count()
                && terminal.unwrap().complete_view() == (old(inventory).healthy()
                    && forall|i: int| 0 <= i < old(inventory).commands().len() ==> old(inventory).closed_at(i)),
            terminal.is_some() ==> final(host).write_calls() == old(host).write_calls().push(field_bytes(
                seq![4u8] + word_bytes(terminal.unwrap().attempts_view() as u64)
                    + word_bytes(terminal.unwrap().started_view() as u64) + word_bytes(terminal.unwrap().closed_view() as u64)
                    + seq![if terminal.unwrap().complete_view() { 1u8 } else { 0u8 }] + word_bytes(verifier_exit as u32 as u64))),
            terminal.is_none() ==> final(host).write_calls() == old(host).write_calls() && !final(inventory).healthy())
    {
        if inventory.ended { inventory.poison(); return None; }
        let terminal = inventory.finish();
        let event = encode_terminal(&terminal, verifier_exit);
        let packet = frame_event(&event);
        if host.write_all(&packet).is_err() { inventory.poison(); }
        Some(terminal)
    }
}

sail_code! {
    /// Deterministic primitive implementation used by executable lifecycle controls.
    pub struct ControlHost {
        /// Actual stream indexes observed by this executable control host.
        pub spawns: Vec<usize>,
        /// Exact command copies observed by this executable control host.
        pub commands: Vec<LaunchSpec>,
        /// Owned child identities passed to the wait primitive.
        pub waits: Vec<u32>,
        /// Exact byte slices supplied to append-write attempts.
        pub writes: Vec<Vec<u8>>,
        /// One marker for each actual write error returned by this control host.
        pub failures: Vec<()>,
        /// Configures the deterministic control host write result.
        pub fail_write: bool,
    }
    impl ProcessHost<u32, (), (), ()> for ControlHost {
        sail_trait_impl_spec! {
            open spec fn spawn_calls(&self) -> nat { self.spawns@.len() }
            open spec fn spawn_commands(&self) -> Seq<(Seq<u8>, Seq<Seq<u8>>, Seq<u8>)> { self.commands@.map_values(|command: LaunchSpec| launch_bytes(command)) }
            open spec fn spawn_routes(&self) -> Seq<nat> { self.spawns@.map_values(|i: usize| i as nat) }
            open spec fn wait_calls(&self) -> nat { self.waits@.len() }
            open spec fn write_calls(&self) -> Seq<Seq<u8>> { self.writes@.map_values(|v: Vec<u8>| v@) }
            open spec fn failed_writes(&self) -> nat { self.failures@.len() }
        }
        fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
            let owned = copy_bytes(bytes);
            self.writes.push(owned);
            sail_proof! {
                assert(owned@ == bytes@);
                assert(self.writes@.map_values(|v: Vec<u8>| v@) =~=
                    old(self).writes@.map_values(|v: Vec<u8>| v@).push(bytes@));
            }
            if self.fail_write { self.failures.push(()); Err(()) } else { Ok(()) }
        }
        fn spawn(&mut self, command: &LaunchSpec, stream: usize) -> Result<u32, ()> {
            self.spawns.push(stream);
            self.commands.push(copy_launch(command));
            sail_proof! { assert(self.commands@.map_values(|command: LaunchSpec| launch_bytes(command)) =~=
                old(self).commands@.map_values(|command: LaunchSpec| launch_bytes(command)).push(launch_bytes(*command))); }
            sail_proof! { assert(self.spawns@.map_values(|i: usize| i as nat) =~=
                old(self).spawns@.map_values(|i: usize| i as nat).push(stream as nat)); }
            Ok(41)
        }
        fn pid(&self, child: &u32) -> u32 { *child }
        fn wait(&mut self, child: &mut u32) -> Result<WaitObservation, ()> {
            self.waits.push(*child);
            Ok(WaitObservation { normal: true, code: 0 })
        }
        fn take_input(&mut self, _child: &mut u32) -> Option<()> { Some(()) }
        /// Takes only the existing child output pipe, preserving process histories.
        fn take_output(&mut self, _child: &mut u32) -> Option<()> { Some(()) }
    }
}

sail_verified! {
    /// A real nonempty lifecycle executes exactly one spawn and wait before seal.
    pub fn nonempty_lifecycle_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        sail_proof! { reveal(Inventory::ended); reveal(Inventory::healthy); reveal(Inventory::commands);
            reveal(Terminal::complete_view);
            reveal(<ControlHost as ProcessHost<u32, (), (), ()>>::write_calls);
            reveal(<ControlHost as ProcessHost<u32, (), (), ()>>::failed_writes);
            reveal(Seq::map_values);
        }
        let mut host = ControlHost { spawns: Vec::new(), commands: Vec::new(), waits: Vec::new(), writes: Vec::new(),
            failures: Vec::new(), fail_write: false };
        sail_proof! { assert(host.write_calls().len() == 0); assert(host.failed_writes() == 0); }
        let mut inventory = begin_observer(&mut host, &[5u8, 0u8, 9u8], 17);
        sail_proof! { assert(host.write_calls().len() == 1); assert(host.write_calls().len() == host.writes.len()); assert(host.writes.len() == 1); }
        let command = LaunchSpec { executable: vec![b'z', b'3'], arguments: vec![vec![b'-', b'i', b'n']],
            solver: vec![b'z', b'3'] };
        let outcome = observed_spawn(&mut host, &mut inventory, command);
        if let SpawnOutcome::Started(mut child) = outcome {
            let _ = child.pipes(&mut host);
            let _ = observed_wait(&mut host, &mut inventory, &mut child);
        }
        sail_proof! { assert(host.write_calls().len() == host.writes.len()); }
        let terminal = seal_observer(&mut host, &mut inventory, 0);
        sail_proof! { assert(host.write_calls().len() == host.writes.len()); }
        // The generic process contract intentionally promises neither successful
        // spawn nor correct solver output; the exact call inventory still holds.
        { sail_proof! { assert(inventory.ended); assert(inventory.slots.len() == 1);
            assert(host.spawns.len() == 1); assert(terminal.is_some()); assert(host.write_calls().len() >= 3); assert(host.writes.len() >= 3); }
            inventory.ended && inventory.slots.len() == 1 && host.spawns.len() == 1 && terminal.is_some() && host.writes.len() >= 3 }
    }
}

sail_verified! {
    /// Critical finality control: a broken journal cannot hide a spawn after End.
    pub fn sealed_late_spawn_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        sail_proof! { reveal(Inventory::ended); reveal(Inventory::healthy); reveal(Inventory::commands);
            reveal(Terminal::complete_view);
            reveal(<ControlHost as ProcessHost<u32, (), (), ()>>::write_calls);
            reveal(<ControlHost as ProcessHost<u32, (), (), ()>>::failed_writes);
            reveal(Seq::map_values);
        }
        let mut host = ControlHost { spawns: Vec::new(), commands: Vec::new(), waits: Vec::new(), writes: Vec::new(),
            failures: Vec::new(), fail_write: false };
        sail_proof! { assert(host.write_calls().len() == 0); assert(host.failed_writes() == 0); }
        let mut inventory = begin_observer(&mut host, &[1u8], 17);
        sail_proof! { assert(host.write_calls().len() == host.writes.len()); }
        let terminal = seal_observer(&mut host, &mut inventory, 0);
        sail_proof! { assert(host.write_calls().len() == host.writes.len()); }
        host.fail_write = true;
        let outcome = observed_spawn(&mut host, &mut inventory,
            LaunchSpec { executable: vec![b'z'], arguments: Vec::new(), solver: vec![b'z'] });
        let duplicate = seal_observer(&mut host, &mut inventory, 0);
        sail_proof! { assert(host.write_calls().len() == host.writes.len()); }
        { sail_proof! { assert(matches!(outcome, SpawnOutcome::Refused)); assert(terminal.is_some()); assert(duplicate.is_none());
            assert(host.spawns.len() == 0); assert(host.writes.len() == 2); assert(!inventory.healthy); }
        matches!(outcome, SpawnOutcome::Refused) && terminal.is_some() && duplicate.is_none()
            && host.spawns.is_empty() && host.writes.len() == 2 && !inventory.healthy }
    }
}

sail_verified! {
    /// Failed observation writes still count every actual subsequent launch call.
    pub fn failed_journal_keeps_launch_inventory_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        sail_proof! { reveal(Inventory::ended); reveal(Inventory::healthy); reveal(Inventory::commands);
            reveal(Terminal::complete_view);
            reveal(<ControlHost as ProcessHost<u32, (), (), ()>>::write_calls);
            reveal(<ControlHost as ProcessHost<u32, (), (), ()>>::failed_writes);
            reveal(Seq::map_values);
        }
        let mut host = ControlHost { spawns: Vec::new(), commands: Vec::new(), waits: Vec::new(), writes: Vec::new(),
            failures: Vec::new(), fail_write: true };
        sail_proof! { assert(host.write_calls().len() == 0); assert(host.failed_writes() == 0); }
        let mut inventory = begin_observer(&mut host, &[1u8], 17);
        let begin_healthy = inventory.healthy;
        let _ = observed_spawn(&mut host, &mut inventory,
            LaunchSpec { executable: vec![b'z'], arguments: Vec::new(), solver: vec![b'z'] });
        let _ = observed_spawn(&mut host, &mut inventory,
            LaunchSpec { executable: vec![b'c'], arguments: Vec::new(), solver: vec![b'c'] });
        let terminal = seal_observer(&mut host, &mut inventory, 1);
        { sail_proof! { assert(host.spawns.len() == 2); assert(inventory.slots.len() == 2); assert(terminal.is_some());
            assert(!begin_healthy ==> !terminal.unwrap().complete && !inventory.healthy); }
        let incomplete = match terminal.as_ref() { Some(value) => !value.complete, None => false };
        host.spawns.len() == 2 && inventory.slots.len() == 2 && terminal.is_some()
            && (begin_healthy || (incomplete && !inventory.healthy)) }
    }
}

sail_code! {
    /// Untrusted decoded data. Only checked whole-journal admission grants evidence.
    #[derive(Debug)]
    pub enum JournalEvent {
        /// Explicit versioned verifier begin, including zero-SMT invocations.
        Begin {
            /// Exact independently retained request bytes.
            request: Vec<u8>,
            /// Actual verifier process identifier.
            verifier_pid: u32,
        },
        /// One actual call to the solver launch controller.
        Intent {
            /// Derived monotonic stream index.
            stream: usize,
            /// Exact command passed to the native spawn primitive.
            command: LaunchSpec,
        },
        /// A successful spawn observation.
        Started {
            /// Previously recorded intent index.
            stream: usize,
            /// Actual PID returned by the owned child.
            pid: u32,
        },
        /// One matching wait on the same owned child.
        Waited {
            /// Previously started stream index.
            stream: usize,
            /// Same actual child PID.
            pid: u32,
            /// Actual wait status, including nonzero or signal termination.
            status: WaitObservation,
        },
        /// Final complete-ledger snapshot; decoder-supplied values remain untrusted.
        End {
            /// Derived number of intent records.
            attempts: usize,
            /// Derived number of successful spawn observations.
            started: usize,
            /// Derived number of matching wait observations.
            closed: usize,
            /// All attempts closed with no prior journal failure.
            complete: bool,
            /// Actual verifier main exit code.
            verifier_exit: i32,
        },
    }
}

sail_spec! {
    pub open spec fn event_bytes(event: JournalEvent) -> Seq<u8> {
        match event {
            JournalEvent::Begin { request, verifier_pid } => seq![0u8, 1u8] + field_bytes(request@) + word_bytes(verifier_pid as u64),
            JournalEvent::Intent { stream, command } => seq![1u8] + word_bytes(stream as u64)
                + field_bytes(command.solver@) + field_bytes(command.executable@) + argument_bytes(command.arguments@),
            JournalEvent::Started { stream, pid } => seq![2u8] + word_bytes(stream as u64) + word_bytes(pid as u64),
            JournalEvent::Waited { stream, pid, status } => seq![3u8] + word_bytes(stream as u64) + word_bytes(pid as u64)
                + seq![if status.normal { 1u8 } else { 0u8 }] + word_bytes(status.code as u32 as u64),
            JournalEvent::End { attempts, started, closed, complete, verifier_exit } => seq![4u8]
                + word_bytes(attempts as u64) + word_bytes(started as u64) + word_bytes(closed as u64)
                + seq![if complete { 1u8 } else { 0u8 }] + word_bytes(verifier_exit as u32 as u64),
        }
    }
    pub open spec fn journal_bytes(events: Seq<JournalEvent>) -> Seq<u8>
        decreases events.len(),
    {
        if events.len() == 0 { Seq::empty() }
        else { journal_bytes(events.drop_last()) + field_bytes(event_bytes(events.last())) }
    }
}

sail_verified! {
    /// Re-encodes untrusted decoded values using the same checked producer codec.
    pub fn encode_event(event: &JournalEvent) -> (bytes: Vec<u8>)
        requires (true) ensures (bytes@ == event_bytes(*event))
    {
        match event {
            JournalEvent::Begin { request, verifier_pid } => encode_begin(request, *verifier_pid),
            JournalEvent::Intent { stream, command } => {
                let mut bytes = vec![1u8];
                append_word(&mut bytes, *stream as u64);
                append_field(&mut bytes, &command.solver);
                append_field(&mut bytes, &command.executable);
                let arguments = encode_arguments(&command.arguments);
                let mut i = 0;
                sail_while! { (i < arguments.len()) invariant (
                    i <= arguments.len(), bytes@ == seq![1u8] + word_bytes(*stream as u64)
                        + field_bytes(command.solver@) + field_bytes(command.executable@) + arguments@.take(i as int)
                ) decreases (arguments.len() - i) { bytes.push(arguments[i]); i += 1; }}
                bytes
            },
            JournalEvent::Started { stream, pid } => encode_started(*stream, *pid),
            JournalEvent::Waited { stream, pid, status } => encode_waited(*stream, *pid, *status),
            JournalEvent::End { attempts, started, closed, complete, verifier_exit } => {
                let terminal = Terminal { attempts: *attempts, started: *started, closed: *closed, complete: *complete };
                encode_terminal(&terminal, *verifier_exit)
            },
        }
    }
}

sail_verified! {
    /// Serializes every event in order with complete outer framing.
    pub fn encode_journal(events: &[JournalEvent]) -> (bytes: Vec<u8>)
        requires (true) ensures (bytes@ == journal_bytes(events@))
    {
        let mut bytes = Vec::new(); let mut i = 0;
        sail_while! { (i < events.len()) invariant (
            i <= events.len(), bytes@ == journal_bytes(events@.take(i as int))
        ) decreases (events.len() - i) {
            let event = encode_event(&events[i]);
            append_field(&mut bytes, &event);
            sail_proof! {
                assert(events@.take(i as int + 1).drop_last() =~= events@.take(i as int));
                assert(events@.take(i as int + 1).last() == events@[i as int]);
                reveal_with_fuel(journal_bytes, 1);
            }
            i += 1;
        }}
        sail_proof! { assert(events@.take(events.len() as int) =~= events@); }
        bytes
    }
}

sail_verified! {
    /// Compares full byte slices, including length.
    pub fn bytes_equal(left: &[u8], right: &[u8]) -> (equal: bool)
        requires (true) ensures (equal == (left@ == right@))
    {
        if left.len() != right.len() { return false; }
        let mut i = 0;
        sail_while! { (i < left.len()) invariant (
            i <= left.len(), left.len() == right.len(),
            forall|j: int| 0 <= j < i ==> left@[j] == right@[j]
        ) decreases (left.len() - i) {
            if left[i] != right[i] { return false; }
            i += 1;
        }}
        sail_proof! { assert(left@ =~= right@); }
        true
    }
}

sail_spec! {
    pub open spec fn empty_replay() -> (Seq<(Seq<u8>, Seq<Seq<u8>>, Seq<u8>)>, Seq<Option<u32>>, Seq<Option<WaitObservation>>) {
        (Seq::empty(), Seq::empty(), Seq::empty())
    }
    pub open spec fn replay_step(before: (Seq<(Seq<u8>, Seq<Seq<u8>>, Seq<u8>)>, Seq<Option<u32>>, Seq<Option<WaitObservation>>), event: JournalEvent) -> Option<(Seq<(Seq<u8>, Seq<Seq<u8>>, Seq<u8>)>, Seq<Option<u32>>, Seq<Option<WaitObservation>>)> {
        match event {
            JournalEvent::Intent { stream, command } => {
                if stream == before.0.len() {
                    Some((before.0.push(launch_bytes(command)), before.1.push(None), before.2.push(None)))
                } else { None }
            },
            JournalEvent::Started { stream, pid } => {
                if stream < before.0.len() && pid != 0 && before.1[stream as int].is_none() {
                    Some((before.0, before.1.update(stream as int, Some(pid)), before.2))
                } else { None }
            },
            JournalEvent::Waited { stream, pid, status } => {
                if stream < before.0.len() && pid != 0 && before.1[stream as int] == Some(pid)
                    && before.2[stream as int].is_none() {
                    Some((before.0, before.1, before.2.update(stream as int, Some(status))))
                } else { None }
            },
            _ => None,
        }
    }
    pub open spec fn replay(events: Seq<JournalEvent>) -> Option<(Seq<(Seq<u8>, Seq<Seq<u8>>, Seq<u8>)>, Seq<Option<u32>>, Seq<Option<WaitObservation>>)>
        decreases events.len(),
    {
        if events.len() == 0 { Some(empty_replay()) }
        else {
            let previous = replay(events.drop_last());
            if previous.is_none() { None } else { replay_step(previous.unwrap(), events.last()) }
        }
    }
}

sail_code! {
    impl Inventory {
        sail_impl_spec! {
            pub closed spec fn replay_view(&self) -> (Seq<(Seq<u8>, Seq<Seq<u8>>, Seq<u8>)>, Seq<Option<u32>>, Seq<Option<WaitObservation>>) {
                (self.slots@.map_values(|slot: LaunchSlot| launch_bytes(slot.command)),
                    self.slots@.map_values(|slot: LaunchSlot| slot.pid),
                    self.slots@.map_values(|slot: LaunchSlot| slot.waited))
            }
        }
    }
}

sail_verified! {
    /// Reconstructs the same ownership ledger from untrusted individual records.
    /// Begin/End are forbidden here and handled once by whole-journal admission.
    pub fn replay_event(inventory: &mut Inventory, event: &JournalEvent) -> (accepted: bool)
        requires (old(inventory).healthy(), !old(inventory).ended())
        ensures (accepted == replay_step(old(inventory).replay_view(), *event).is_some(),
            accepted ==> final(inventory).healthy() && !final(inventory).ended()
                && final(inventory).replay_view() == replay_step(old(inventory).replay_view(), *event).unwrap(),
            !accepted ==> !final(inventory).healthy())
    {
        sail_proof! { reveal(Inventory::replay_view); reveal(Inventory::commands);
            reveal(Inventory::healthy); reveal(Inventory::ended); reveal(Inventory::pid_at); reveal(Inventory::wait_at); }
        let accepted = match event {
            JournalEvent::Intent { stream, command } => {
                if *stream != inventory.slots.len() { inventory.poison(); return false; }
                let _ = inventory.intent(copy_launch(command));
                true
            },
            JournalEvent::Started { stream, pid } => inventory.started(*stream, *pid),
            JournalEvent::Waited { stream, pid, status } => inventory.closed(*stream, *pid, *status),
            _ => { inventory.poison(); false },
        };
        sail_proof! {
            if accepted {
                let expected = replay_step(old(inventory).replay_view(), *event).unwrap();
                assert(inventory.replay_view().0 =~= expected.0);
                assert(inventory.replay_view().1 =~= expected.1);
                assert(inventory.replay_view().2 =~= expected.2);
            }
        }
        accepted
    }
}

sail_spec! {
    pub open spec fn journal_records_valid(events: Seq<JournalEvent>, request: Seq<u8>, pid: u32, exit: i32) -> bool {
        events.len() >= 2 && pid != 0
        && (match events.first() {
            JournalEvent::Begin { request: observed, verifier_pid } => observed@ == request && verifier_pid == pid,
            _ => false,
        })
        && replay(events.subrange(1, events.len() as int - 1)).is_some()
        && {
            let state = replay(events.subrange(1, events.len() as int - 1)).unwrap();
            state.1.len() == state.0.len() && state.2.len() == state.0.len()
            && (forall|i: int| 0 <= i < state.0.len() ==> #[trigger] state.1[i].is_some()
                && state.1[i].unwrap() != 0 && state.2[i].is_some())
            && (match events.last() {
                JournalEvent::End { attempts, started, closed, complete, verifier_exit } =>
                    complete && attempts == state.0.len() && started == attempts && closed == attempts && verifier_exit == exit,
                _ => false,
            })
        }
    }
}

sail_code! {
    /// Complete typed lifecycle bound to exact raw journal bytes. This is not a
    /// solver-result/origin capability without the protected producer binding.
    pub struct AdmittedJournal<'a> {
        raw: &'a [u8],
        events: &'a [JournalEvent],
        inventory: Inventory,
    }
    impl AdmittedJournal<'_> {
        sail_impl_spec! {
            pub closed spec fn raw_view(&self) -> Seq<u8> { self.raw@ }
            pub closed spec fn events_view(&self) -> Seq<JournalEvent> { self.events@ }
            pub closed spec fn replay_view(&self) -> (Seq<(Seq<u8>, Seq<Seq<u8>>, Seq<u8>)>, Seq<Option<u32>>, Seq<Option<WaitObservation>>) { self.inventory.replay_view() }
        }
        sail_method! {
            /// Returns the complete reconstructed stream count, including zero.
            pub fn stream_count(&self) -> (count: usize)
                requires (true) ensures (count == self.replay_view().0.len())
            {
                sail_proof! { reveal(AdmittedJournal::replay_view); reveal(Inventory::replay_view); }
                self.inventory.slots.len()
            }
        }
        sail_method! {
            /// Borrows an exact command and its matching actual PID/wait observation.
            pub fn stream(&self, index: usize) -> (entry: Option<(&LaunchSpec, u32, WaitObservation)>)
                requires (true)
                ensures (entry.is_some() ==> index < self.replay_view().0.len()
                    && launch_bytes(*entry.unwrap().0) == self.replay_view().0[index as int]
                    && Some(entry.unwrap().1) == self.replay_view().1[index as int]
                    && Some(entry.unwrap().2) == self.replay_view().2[index as int])
            {
                sail_proof! { reveal(AdmittedJournal::replay_view); reveal(Inventory::replay_view); }
                if index >= self.inventory.slots.len() { return None; }
                let slot = &self.inventory.slots[index];
                match (slot.pid, slot.waited) {
                    (Some(pid), Some(status)) => Some((&slot.command, pid, status)),
                    _ => None,
                }
            }
        }
        sail_method! {
            /// Returns the original complete byte slice checked at admission.
            pub fn raw(&self) -> (bytes: &[u8])
                requires (true) ensures (bytes@ == self.raw_view())
            { self.raw }
        }
        sail_method! {
            /// Returns the exact complete decoded record inventory checked at admission.
            pub fn events(&self) -> (events: &[JournalEvent])
                requires (true) ensures (events@ == self.events_view())
            { self.events }
        }
    }
}

sail_verified! {
    /// The decoder is untrusted: every decoded value must re-encode to the whole
    /// actual file, then independently replay into a fully closed ownership ledger.
    pub fn admit_journal<'a>(raw: &'a [u8], events: &'a [JournalEvent], request: &[u8],
        actual_pid: u32, actual_status: WaitObservation) -> (admitted: Option<AdmittedJournal<'a>>)
        requires (true)
        ensures (admitted.is_some() ==> admitted.unwrap().raw_view() == raw@
            && admitted.unwrap().events_view() == events@
            && raw@ == journal_bytes(events@)
            && journal_records_valid(events@, request@, actual_pid, actual_status.code)
            && actual_status.normal
            && admitted.unwrap().replay_view() == replay(events@.subrange(1, events.len() as int - 1)).unwrap())
    {
        sail_proof! { reveal(Inventory::healthy); reveal(Inventory::ended); reveal(Inventory::replay_view);
            reveal(Inventory::commands); reveal(Inventory::closed_at); reveal(Inventory::pid_at); reveal(Inventory::wait_at);
            reveal(Terminal::attempts_view); reveal(Terminal::started_view); reveal(Terminal::closed_view); reveal(Terminal::complete_view);
        }
        if !actual_status.normal || events.len() < 2 || actual_pid == 0 { return None; }
        let encoded = encode_journal(events);
        if !bytes_equal(raw, &encoded) { return None; }
        match &events[0] {
            JournalEvent::Begin { request: observed, verifier_pid } => {
                if *verifier_pid != actual_pid || !bytes_equal(observed, request) { return None; }
            },
            _ => return None,
        }
        let mut inventory = Inventory::new();
        let mut i = 1;
        sail_proof! { assert(inventory.replay_view() == empty_replay()); reveal_with_fuel(replay, 1); }
        sail_while! { (i < events.len() - 1) invariant (
            1 <= i <= events.len() - 1, events.len() >= 2,
            inventory.healthy(), !inventory.ended(),
            replay(events@.subrange(1, i as int)) == Some(inventory.replay_view())
        ) decreases (events.len() - 1 - i) {
            if !replay_event(&mut inventory, &events[i]) { return None; }
            sail_proof! {
                assert(events@.subrange(1, i as int + 1).drop_last() =~= events@.subrange(1, i as int));
                assert(events@.subrange(1, i as int + 1).last() == events@[i as int]);
                reveal_with_fuel(replay, 1);
            }
            i += 1;
        }}
        let terminal = inventory.finish();
        match &events[events.len() - 1] {
            JournalEvent::End { attempts, started, closed, complete, verifier_exit } => {
                if !*complete || !terminal.complete() || *attempts != terminal.attempts()
                    || *started != terminal.started() || *closed != terminal.closed() || *verifier_exit != actual_status.code {
                    return None;
                }
            },
            _ => return None,
        }
        sail_proof! {
            assert forall|j: int| 0 <= j < inventory.replay_view().0.len() implies
                #[trigger] inventory.replay_view().1[j].is_some() && inventory.replay_view().1[j].unwrap() != 0
                    && inventory.replay_view().2[j].is_some() by {
                assert(inventory.closed_at(j));
            }
            assert(journal_records_valid(events@, request@, actual_pid, actual_status.code));
        }
        Some(AdmittedJournal { raw, events, inventory })
    }
}
