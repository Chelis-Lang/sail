//! Insertion generations and transitions over the actual owned memory slots.
//!
//! These preserve candidate data; they do not admit solver-origin authority.
//! A native handle must retain its original memory endpoint. Generations are
//! local to that endpoint, not globally unique allocation identities.
#[cfg(sail_verus)]
use vstd::prelude::*;

use crate::inputs::byte_eq;
use std::collections::BTreeMap;

sail_code! {
    /// A checked lifetime clock, shared by insertion generations and LRU touches.
    /// No operation resets it, including cache clearing.
    #[derive(Debug)]
    pub struct Clock {
        last: u64,
    }

    /// The actual memory slot's persistence state; neither variant grants origin.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SlotState<Stamp> {
        /// Its original candidate write remains pending.
        Pending,
        /// The candidate write or read supplied this file stamp.
        OnDisk(Stamp),
    }

    /// The actual payload, state and LRU position stored in the native map.
    /// Its insertion generation never changes when touched or written.
    #[derive(Debug)]
    pub struct Slot<Value, Stamp> {
        value: Value,
        state: SlotState<Stamp>,
        tick: u64,
        generation: u64,
    }

    /// The actual owned key/slot pair returned by native map removal.
    pub type Entry<Value, Stamp> = ([u8; 32], Slot<Value, Stamp>);

    /// An actual entry paired with its insertion-issued ticket.
    pub type IssuedEntry<Value, Stamp> = Option<(Entry<Value, Stamp>, Ticket)>;

    /// An actual entry paired with its original retained endpoint.
    pub type BoundEntry<Endpoint, Value, Stamp> = Option<(Entry<Value, Stamp>, Held<Endpoint>)>;

    /// A privately issued insertion or read observation. Native handles also
    /// retain their original memory endpoint; this ordinal alone is not a domain.
    #[derive(Debug, Clone, Copy)]
    pub struct Ticket {
        key: [u8; 32],
        generation: u64,
    }

    /// Carries its actual memory endpoint and ticket together. It is never
    /// reconstructed from a caller-supplied generation or persisted record.
    #[derive(Debug)]
    pub struct Held<Endpoint> {
        endpoint: Endpoint,
        ticket: Ticket,
    }

    /// The actual disk-cache return type, parameterized only by its native stamp.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Publication<Stamp> {
        /// The disk operation returned its new file stamp.
        Written(Stamp),
        /// It retained an already existing entry.
        Exists,
    }

    /// The actual disk operation's candidate outcome, without an origin claim.
    #[derive(Debug)]
    pub enum Completion<Stamp> {
        /// The real operation returned a newly written stamp.
        Written(Stamp),
        /// An existing file withheld this candidate's publication.
        Exists,
        /// The real write failed.
        Failed,
    }

    /// Owns the actual map entry consumed by a transition. Removed payloads
    /// return to native accounting; retained payloads return to the same map.
    #[derive(Debug)]
    pub enum EntryUpdate<Value, Stamp> {
        /// The observation or pending requirement did not match.
        Unchanged(([u8; 32], Slot<Value, Stamp>)),
        /// The same pending insertion acquired its actual write stamp.
        Written(([u8; 32], Slot<Value, Stamp>)),
        /// Only the observed original insertion is removed.
        Removed(([u8; 32], Slot<Value, Stamp>)),
    }

    /// The actual native tier's declared byte and pending-slot counters.
    /// Costs are supplied by the native charge function for the same owned slot;
    /// this record does not assert a global map, allocator or origin invariant.
    #[derive(Debug)]
    pub struct Accounting {
        bytes: u64,
        pending: u64,
    }
}

sail_code! {
    impl Clock {
        sail_impl_spec! {
            /// Last issued lifetime ordinal; exhausted clocks remain exhausted.
            pub closed spec fn last_view(&self) -> u64 { self.last }
        }
        sail_method! {
            /// Start the clock of a fresh empty tier.
            pub fn start() -> (out: Self)
                requires (true) ensures (out.last_view() == 0)
            { Self { last: 0 } }
        }
        sail_method! {
            /// Permanently refuse further ordinals; this never restarts a domain.
            pub fn exhaust(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).last_view() == u64::MAX,
                    final(self).last_view() >= old(self).last_view())
            { self.last = u64::MAX; }
        }
        sail_method! {
            /// Issue the next ordinal without overflow or reuse.
            pub fn advance(&mut self) -> (out: Option<u64>)
                requires (true)
                ensures (out.is_some() == (old(self).last_view() < u64::MAX),
                    final(self).last_view() >= old(self).last_view(),
                    match out {
                        Some(ordinal) => ordinal == final(self).last_view()
                            && ordinal as int == old(self).last_view() as int + 1,
                        None => final(self).last_view() == old(self).last_view()
                            && final(self).last_view() == u64::MAX,
                    })
            {
                if self.last == u64::MAX { return None; }
                self.last += 1;
                Some(self.last)
            }
        }
        sail_method! {
            /// Create the actual owned entry and its matching ticket in one
            /// checked operation. The supplied payload is moved without copying.
            #[allow(clippy::question_mark, reason = "Explicit refusal preserves the source-shared checked transition and its full-body controls.")]
            pub fn entry<Value, Stamp>(&mut self, key: [u8; 32], value: Value,
                state: SlotState<Stamp>) -> (out: IssuedEntry<Value, Stamp>)
                requires (true)
                ensures (out.is_some() == (old(self).last_view() < u64::MAX),
                    final(self).last_view() >= old(self).last_view(),
                    match out {
                        Some((entry, ticket)) => entry.0@ == key@
                            && entry.1.value_view() == value && entry.1.state_view() == state
                            && entry.1.tick_view() == final(self).last_view()
                            && entry.1.generation_view() == final(self).last_view()
                            && ticket.key_view() == key@
                            && ticket.generation_view() == final(self).last_view()
                            && final(self).last_view() as int == old(self).last_view() as int + 1,
                        None => final(self).last_view() == u64::MAX
                            && old(self).last_view() == u64::MAX,
                    })
            {
                let Some(generation) = self.advance() else { return None; };
                let slot = Slot { value, state, tick: generation, generation };
                let ticket = Ticket { key, generation };
                Some(((key, slot), ticket))
            }
        }
    }

    impl Clock {
        sail_method! {
            /// Retain the actual insertion endpoint in the same operation that
            /// creates the entry and ticket. Endpoints are forwarded, not compared.
            #[allow(clippy::question_mark, reason = "Explicit refusal preserves the source-shared checked transition and its full-body controls.")]
            pub fn entry_bound<Endpoint, Value, Stamp>(&mut self, endpoint: Endpoint,
                key: [u8; 32], value: Value, state: SlotState<Stamp>)
                -> (out: BoundEntry<Endpoint, Value, Stamp>)
                requires (true)
                ensures (out.is_some() == (old(self).last_view() < u64::MAX),
                    final(self).last_view() >= old(self).last_view(),
                    match out {
                        Some((entry, held)) => entry.0@ == key@
                            && entry.1.value_view() == value && entry.1.state_view() == state
                            && entry.1.tick_view() == final(self).last_view()
                            && entry.1.generation_view() == final(self).last_view()
                            && held.endpoint_view() == endpoint
                            && held.ticket_view().key_view() == key@
                            && held.ticket_view().generation_view() == final(self).last_view()
                            && final(self).last_view() as int == old(self).last_view() as int + 1,
                        None => old(self).last_view() == u64::MAX
                            && final(self).last_view() == u64::MAX,
                    })
            {
                let Some((entry, ticket)) = self.entry(key, value, state) else { return None; };
                Some((entry, Held { endpoint, ticket }))
            }
        }
    }

    impl<Endpoint> Held<Endpoint> {
        sail_impl_spec! {
            /// The original native endpoint, retained without identity inference.
            pub closed spec fn endpoint_view(&self) -> Endpoint { self.endpoint }
            /// The ticket paired with that endpoint at insertion or observation.
            pub closed spec fn ticket_view(&self) -> Ticket { self.ticket }
        }
        sail_method! {
            /// Borrow exactly the original endpoint for the actual native lock.
            pub fn endpoint(&self) -> (out: &Endpoint)
                requires (true) ensures (*out == self.endpoint_view())
            { &self.endpoint }
        }
        sail_method! {
            /// Borrow the original paired ticket; no caller can replace its fields.
            pub fn ticket(&self) -> (out: &Ticket)
                requires (true) ensures (*out == self.ticket_view())
            { &self.ticket }
        }
    }

    impl Ticket {
        sail_impl_spec! {
            /// Exact raw key originally bound to this observation.
            pub closed spec fn key_view(&self) -> Seq<u8> { self.key@ }
            /// The original insertion ordinal in the native handle's tier.
            pub closed spec fn generation_view(&self) -> u64 { self.generation }
        }
        sail_method! {
            /// The exact native map key for this observation.
            pub fn key(&self) -> (out: [u8; 32])
                requires (true) ensures (out@ == self.key_view())
            { self.key }
        }
    }

    impl<Value, Stamp> Slot<Value, Stamp> {
        sail_impl_spec! {
            /// The actual owned candidate payload.
            pub closed spec fn value_view(&self) -> Value { self.value }
            /// The actual persistence state.
            pub closed spec fn state_view(&self) -> SlotState<Stamp> { self.state }
            /// Most recent LRU touch.
            pub closed spec fn tick_view(&self) -> u64 { self.tick }
            /// Immutable insertion ordinal.
            pub closed spec fn generation_view(&self) -> u64 { self.generation }
        }
        sail_method! {
            /// Borrow the original candidate without replacing it.
            pub fn value(&self) -> (out: &Value)
                requires (true) ensures (*out == self.value_view())
            { &self.value }
        }
        sail_method! {
            /// Borrow the actual persistence state.
            pub fn state(&self) -> (out: &SlotState<Stamp>)
                requires (true) ensures (*out == self.state_view())
            { &self.state }
        }
        sail_method! {
            /// Return the actual LRU ordinal.
            pub fn tick(&self) -> (out: u64)
                requires (true) ensures (out == self.tick_view())
            { self.tick }
        }
        sail_method! {
            /// A read observes this insertion; it does not mint a new generation.
            pub fn observe(&self, key: [u8; 32]) -> (out: Ticket)
                requires (true)
                ensures (out.key_view() == key@,
                    out.generation_view() == self.generation_view())
            { Ticket { key, generation: self.generation } }
        }
        sail_method! {
            /// Retain the actual read endpoint and observed insertion together.
            pub fn observe_bound<Endpoint>(&self, endpoint: Endpoint, key: [u8; 32])
                -> (out: Held<Endpoint>)
                requires (true)
                ensures (out.endpoint_view() == endpoint,
                    out.ticket_view().key_view() == key@,
                    out.ticket_view().generation_view() == self.generation_view())
            { Held { endpoint, ticket: self.observe(key) } }
        }
        sail_method! {
            /// LRU touching preserves the original candidate and generation.
            pub fn touch(&mut self, tick: u64) -> (out: ())
                requires (true)
                ensures (final(self).tick_view() == tick,
                    final(self).generation_view() == old(self).generation_view(),
                    final(self).value_view() == old(self).value_view(),
                    final(self).state_view() == old(self).state_view())
            { self.tick = tick; }
        }
    }

    impl<Value, Stamp> EntryUpdate<Value, Stamp> {
        sail_impl_spec! {
            /// The real map key retained through the operation.
            pub closed spec fn key_view(&self) -> Seq<u8> {
                match self { Self::Unchanged(entry) | Self::Written(entry) | Self::Removed(entry) => entry.0@ }
            }
            /// The actual owned slot retained or removed by the operation.
            pub closed spec fn slot_view(&self) -> Slot<Value, Stamp> {
                match self { Self::Unchanged(entry) | Self::Written(entry) | Self::Removed(entry) => entry.1 }
            }
            /// Only this result permits native retirement of the supplied entry.
            pub closed spec fn removed_view(&self) -> bool { matches!(self, Self::Removed(_)) }
            /// Only this result decrements pending for an installed write stamp.
            pub closed spec fn written_view(&self) -> bool { matches!(self, Self::Written(_)) }
        }
        sail_method! {
            /// Borrow the actual owned slot before computing its native charge.
            pub fn slot(&self) -> (out: &Slot<Value, Stamp>)
                requires (true) ensures (*out == self.slot_view())
            {
                match self {
                    Self::Unchanged((_, slot)) | Self::Written((_, slot)) | Self::Removed((_, slot)) => slot,
                }
            }
        }
    }

    impl Accounting {
        sail_impl_spec! {
            /// Actual charged payload/slot occupancy, excluding fixed tier metadata.
            pub closed spec fn bytes_view(&self) -> u64 { self.bytes }
            /// Actual number of declared pending slots.
            pub closed spec fn pending_view(&self) -> u64 { self.pending }
        }
        sail_method! {
            /// Initialize the counters of a fresh empty tier.
            pub fn empty() -> (out: Self)
                requires (true) ensures (out.bytes_view() == 0, out.pending_view() == 0)
            { Self { bytes: 0, pending: 0 } }
        }
        sail_method! {
            /// Report the actual charged occupancy used by native eviction/stats.
            pub fn bytes(&self) -> (out: u64)
                requires (true) ensures (out == self.bytes_view())
            { self.bytes }
        }
        sail_method! {
            /// Report the actual pending count used by native stats.
            pub fn pending(&self) -> (out: u64)
                requires (true) ensures (out == self.pending_view())
            { self.pending }
        }
        sail_method! {
            /// Clear occupancy when native disablement clears its actual maps.
            /// This does not touch or restart the separate lifetime clock.
            pub fn clear(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).bytes_view() == 0, final(self).pending_view() == 0)
            { self.bytes = 0; self.pending = 0; }
        }
        sail_method! {
            /// Reserve the supplied charge for this actual slot. Both additions
            /// must fit; either refusal leaves both counters unchanged.
            pub fn reserve<Value, Stamp>(&mut self, slot: &Slot<Value, Stamp>, cost: u64)
                -> (out: bool)
                requires (true)
                ensures (out == (old(self).bytes_view() as int + cost as int <= u64::MAX
                        && (!matches!(slot.state_view(), SlotState::Pending)
                            || old(self).pending_view() < u64::MAX)),
                    final(self).bytes_view() as int == if out {
                        old(self).bytes_view() as int + cost as int
                    } else { old(self).bytes_view() as int },
                    final(self).pending_view() as int == if out && matches!(slot.state_view(), SlotState::Pending) {
                        old(self).pending_view() as int + 1
                    } else { old(self).pending_view() as int })
            {
                let Some(bytes) = self.bytes.checked_add(cost) else { return false; };
                let pending = if matches!(slot.state, SlotState::Pending) {
                    let Some(pending) = self.pending.checked_add(1) else { return false; };
                    pending
                } else { self.pending };
                self.bytes = bytes;
                self.pending = pending;
                true
            }
        }
        sail_method! {
            /// Account for the actual owned transition and return that exact
            /// transition to the native map/order operations. No replacement
            /// payload or separately supplied completion flag is accepted.
            #[allow(clippy::implicit_saturating_sub, reason = "Explicit guarded subtraction preserves the checked body and its full-runtime mutation controls.")]
            pub fn apply<Value, Stamp>(&mut self, update: EntryUpdate<Value, Stamp>, cost: u64)
                -> (out: EntryUpdate<Value, Stamp>)
                requires (true)
                ensures (out == update,
                    final(self).bytes_view() as int == if update.removed_view() {
                        if old(self).bytes_view() >= cost { old(self).bytes_view() as int - cost as int } else { 0 }
                    } else { old(self).bytes_view() as int },
                    final(self).pending_view() as int == if update.written_view()
                        || (update.removed_view() && matches!(update.slot_view().state_view(), SlotState::Pending)) {
                        if old(self).pending_view() > 0 { old(self).pending_view() as int - 1 } else { 0 }
                    } else { old(self).pending_view() as int })
            {
                if matches!(update, EntryUpdate::Removed(_)) {
                    self.bytes = if self.bytes >= cost { self.bytes - cost } else { 0 };
                }
                if matches!(update, EntryUpdate::Written(_))
                    || (matches!(update, EntryUpdate::Removed(_))
                        && matches!(update.slot().state(), SlotState::Pending)) {
                    self.pending = if self.pending > 0 { self.pending - 1 } else { 0 };
                }
                update
            }
        }
    }
}

sail_spec! {
    /// Compare the supplied key and generation. The actual native handle retains
    /// its endpoint separately; this predicate asserts no global ownership identity.
    pub open spec fn matches_ticket<Value, Stamp>(entry: ([u8; 32], Slot<Value, Stamp>), ticket: Ticket) -> bool {
        entry.0@ == ticket.key_view() && entry.1.generation_view() == ticket.generation_view()
    }
}

sail_verified! {
    /// Apply the actual owned accounting transition and retire only its original
    /// LRU tick. The supplied map is the native tier's real ordered index.
    /// This does not assert that the slot map, index, lock or history are globally
    /// consistent, and it grants no candidate origin or proof authority.
    pub fn apply_accounted_order<Value, Stamp>(accounting: &mut Accounting,
        order: &mut BTreeMap<u64, [u8; 32]>, update: EntryUpdate<Value, Stamp>, cost: u64)
        -> (out: EntryUpdate<Value, Stamp>)
        requires (true)
        ensures (out == update,
            final(order)@ == if update.removed_view() {
                old(order)@.remove(update.slot_view().tick_view())
            } else { old(order)@ },
            final(accounting).bytes_view() as int == if update.removed_view() {
                if old(accounting).bytes_view() >= cost {
                    old(accounting).bytes_view() as int - cost as int
                } else { 0 }
            } else { old(accounting).bytes_view() as int },
            final(accounting).pending_view() as int == if update.written_view()
                || (update.removed_view() && matches!(update.slot_view().state_view(), SlotState::Pending)) {
                if old(accounting).pending_view() > 0 {
                    old(accounting).pending_view() as int - 1
                } else { 0 }
            } else { old(accounting).pending_view() as int })
    {
        sail_proof! { broadcast use vstd::laws_cmp::group_laws_cmp; }
        let update = accounting.apply(update, cost);
        if matches!(update, EntryUpdate::Removed(_)) {
            order.remove(&update.slot().tick());
        }
        update
    }
}

sail_verified! {
    /// Apply an actual disk outcome only to its original still-pending insertion.
    /// This owns the real entry removed from the native map, not a shadow ledger.
    pub fn complete<Value, Stamp>(entry: ([u8; 32], Slot<Value, Stamp>), ticket: &Ticket,
        completion: Completion<Stamp>) -> (out: EntryUpdate<Value, Stamp>)
        requires (true)
        ensures (out.key_view() == entry.0@,
            out.slot_view().value_view() == entry.1.value_view(),
            out.slot_view().generation_view() == entry.1.generation_view(),
            out.slot_view().tick_view() == entry.1.tick_view(),
            out.written_view() == (matches_ticket(entry, *ticket)
                && matches!(entry.1.state_view(), SlotState::Pending)
                && matches!(completion, Completion::Written(_))),
            out.removed_view() == (matches_ticket(entry, *ticket)
                && matches!(entry.1.state_view(), SlotState::Pending)
                && !matches!(completion, Completion::Written(_))),
            match completion {
                Completion::Written(stamp) => out.slot_view().state_view()
                    == if matches_ticket(entry, *ticket) && matches!(entry.1.state_view(), SlotState::Pending)
                        { SlotState::OnDisk(stamp) } else { entry.1.state_view() },
                _ => out.slot_view().state_view() == entry.1.state_view(),
            })
    {
        let (key, mut slot) = entry;
        if !byte_eq(&key, &ticket.key) || slot.generation != ticket.generation
            || !matches!(slot.state, SlotState::Pending) {
            return EntryUpdate::Unchanged((key, slot));
        }
        match completion {
            Completion::Written(stamp) => {
                slot.state = SlotState::OnDisk(stamp);
                EntryUpdate::Written((key, slot))
            }
            Completion::Exists | Completion::Failed => EntryUpdate::Removed((key, slot)),
        }
    }
}

sail_verified! {
    /// Remove only the original read observation, regardless of its persistence state.
    pub fn remove_observed<Value, Stamp>(entry: ([u8; 32], Slot<Value, Stamp>), ticket: &Ticket)
        -> (out: EntryUpdate<Value, Stamp>)
        requires (true)
        ensures (out.key_view() == entry.0@,
            out.slot_view().value_view() == entry.1.value_view(),
            out.slot_view().state_view() == entry.1.state_view(),
            out.slot_view().generation_view() == entry.1.generation_view(),
            out.slot_view().tick_view() == entry.1.tick_view(), !out.written_view(),
            out.removed_view() == matches_ticket(entry, *ticket))
    {
        let (key, slot) = entry;
        if !byte_eq(&key, &ticket.key) || slot.generation != ticket.generation {
            return EntryUpdate::Unchanged((key, slot));
        }
        EntryUpdate::Removed((key, slot))
    }
}

sail_verified! {
    /// Nonempty actual owned entries exercise reservation, write settlement,
    /// stale replacement refusal and retirement through the native-used counters.
    pub fn accounting_transition_control() -> (out: bool)
        requires (true) ensures (out)
    {
        let key = [19u8; 32];
        let mut clock = Clock::start();
        let mut accounting = Accounting::empty();
        let Some((entry, original)) = clock.entry(key, vec![117u8,110u8,115u8,97u8,116u8,10u8], SlotState::<u64>::Pending)
            else { return false; };
        if !accounting.reserve(&entry.1, 142) { return false; }
        let written = accounting.apply(complete(entry, &original, Completion::Written(11)), 142);
        let EntryUpdate::Written(entry) = written else { return false; };
        if entry.1.value().len() != 6 || accounting.bytes() != 142 || accounting.pending() != 0 { return false; }
        let read = entry.1.observe(entry.0);
        let retired = accounting.apply(remove_observed(entry, &read), 142);
        if !matches!(retired, EntryUpdate::Removed(_)) || accounting.bytes() != 0 { return false; }
        let Some((replacement, newer)) = clock.entry(key, vec![117u8,110u8,115u8,97u8,116u8,10u8], SlotState::<u64>::Pending)
            else { return false; };
        if !accounting.reserve(&replacement.1, 142) { return false; }
        let stale = accounting.apply(complete(replacement, &original, Completion::Written(99)), 142);
        let EntryUpdate::Unchanged(replacement) = stale else { return false; };
        if accounting.bytes() != 142 || accounting.pending() != 1 { return false; }
        let refused = accounting.apply(complete(replacement, &newer, Completion::Exists), 142);
        matches!(refused, EntryUpdate::Removed(_)) && accounting.bytes() == 0 && accounting.pending() == 0
    }
}

sail_verified! {
    /// Reservation overflow preserves both counters; clear and underflow-safe
    /// retirement use the same executable operations as actual disablement.
    pub fn accounting_overflow_control() -> (out: bool)
        requires (true) ensures (out)
    {
        let mut clock = Clock::start();
        let Some((entry, ticket)) = clock.entry([23u8; 32], vec![1u8], SlotState::<u64>::Pending)
            else { return false; };
        let mut accounting = Accounting::empty();
        if !accounting.reserve(&entry.1, u64::MAX) { return false; }
        if accounting.reserve(&entry.1, 1) { return false; }
        if accounting.bytes() != u64::MAX || accounting.pending() != 1 { return false; }
        accounting.clear();
        let retired = accounting.apply(complete(entry, &ticket, Completion::Failed), 1);
        matches!(retired, EntryUpdate::Removed(_)) && accounting.bytes() == 0 && accounting.pending() == 0
    }
}

sail_verified! {
    /// Nonempty original and replacement data exercise issuance, real slot
    /// transition, read removal and permanent exhaustion without owner assumptions.
    pub fn generation_transition_control() -> (out: bool)
        requires (true) ensures (out)
    {
        let key = [7u8; 32];
        let mut clock = Clock::start();
        let Some((entry, original)) = clock.entry(key, vec![117u8,110u8,115u8,97u8,116u8,10u8], SlotState::<u64>::Pending)
            else { return false; };
        let written = complete(entry, &original, Completion::Written(11));
        let EntryUpdate::Written((same_key, same_slot)) = written else { return false; };
        if same_slot.value().len() != 6 || same_slot.tick() != 1 { return false; }
        let observed = same_slot.observe(same_key);
        let removed = remove_observed((same_key, same_slot), &observed);
        if !matches!(removed, EntryUpdate::Removed(_)) { return false; }
        let Some((replacement, newer)) = clock.entry(key, vec![117u8,110u8,115u8,97u8,116u8,10u8], SlotState::<u64>::Pending)
            else { return false; };
        let stale = complete(replacement, &original, Completion::Written(99));
        let EntryUpdate::Unchanged(replacement) = stale else { return false; };
        let stale_remove = remove_observed(replacement, &observed);
        let EntryUpdate::Unchanged(replacement) = stale_remove else { return false; };
        let refused = complete(replacement, &newer, Completion::Exists);
        if !matches!(refused, EntryUpdate::Removed(_)) { return false; }
        let mut exhausted = Clock::start();
        exhausted.exhaust();
        if exhausted.advance().is_some() || exhausted.advance().is_some() { return false; }
        true
    }
}

sail_verified! {
    /// Nonempty native-shaped ordered entries exercise retirement, written and
    /// unchanged outcomes without asserting a global slot-map/lock invariant.
    pub fn accounted_order_control() -> (out: bool)
        requires (true) ensures (out)
    {
        sail_proof! { broadcast use vstd::laws_cmp::group_laws_cmp; }
        let key = [29u8; 32];
        let other_key = [31u8; 32];
        let mut order = BTreeMap::<u64, [u8; 32]>::new();
        let mut accounting = Accounting::empty();
        let mut clock = Clock::start();
        let Some((entry, ticket)) = clock.entry(key,
            vec![117u8,110u8,115u8,97u8,116u8,10u8], SlotState::<u64>::Pending)
            else { return false; };
        let Some((other_entry, _)) = clock.entry(other_key,
            vec![117u8,110u8,107u8,110u8,111u8,119u8,110u8], SlotState::OnDisk(21u64))
            else { return false; };
        order.insert(1, key);
        order.insert(other_entry.1.tick(), other_key);
        if !accounting.reserve(&entry.1, 142) || !accounting.reserve(&other_entry.1, 143) { return false; }
        let removed = apply_accounted_order(&mut accounting, &mut order,
            complete(entry, &ticket, Completion::Exists), 142);
        if !matches!(removed, EntryUpdate::Removed(_)) || order.len() != 1
            || order.contains_key(&1) || accounting.bytes() != 143 || accounting.pending() != 0 {
            return false;
        }
        let Some(other) = order.get(&2) else { return false; };
        if !byte_eq(other, &other_key) { return false; }
        let Some((entry, ticket)) = clock.entry(key,
            vec![117u8,110u8,115u8,97u8,116u8,10u8], SlotState::<u64>::Pending)
            else { return false; };
        order.insert(3, key);
        if !accounting.reserve(&entry.1, 142) { return false; }
        let written = apply_accounted_order(&mut accounting, &mut order,
            complete(entry, &ticket, Completion::Written(17)), 142);
        let EntryUpdate::Written(entry) = written else { return false; };
        if order.len() != 2 || !order.contains_key(&3)
            || accounting.bytes() != 285 || accounting.pending() != 0 {
            return false;
        }
        let unchanged = apply_accounted_order(&mut accounting, &mut order,
            complete(entry, &ticket, Completion::Written(99)), 142);
        matches!(unchanged, EntryUpdate::Unchanged(_)) && order.len() == 2
            && order.contains_key(&3) && order.contains_key(&2)
            && accounting.bytes() == 285 && accounting.pending() == 0
    }
}

sail_verified! {
    /// Exercise the complete paired factory and both endpoint/ticket projections
    /// for an actual supplied endpoint, without inventing an ownership label.
    pub fn bound_factory_control<Endpoint>(endpoint: Endpoint) -> (out: bool)
        requires (true) ensures (out)
    {
        let key = [9u8; 32];
        let mut clock = Clock::start();
        let Some((entry, held)) = clock.entry_bound(endpoint, key,
            vec![117u8,110u8,115u8,97u8,116u8,10u8], SlotState::<u64>::Pending)
            else { return false; };
        sail_proof! { assert(held.endpoint_view() == endpoint); }
        let written = complete(entry, held.ticket(), Completion::Written(13));
        let EntryUpdate::Written((key, slot)) = written else { return false; };
        if slot.value().len() != 6 { return false; }
        let endpoint_borrow = held.endpoint();
        let read = slot.observe_bound(endpoint_borrow, key);
        sail_proof! { assert(read.endpoint_view() == endpoint_borrow); }
        let removed = remove_observed((key, slot), read.ticket());
        matches!(removed, EntryUpdate::Removed(_))
    }
}

sail_code! {
    /// The actual immutable cache value; native code supplies its existing effort type.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CandidateValue<Effort> {
        /// The exact verification namespace.
        pub mode: String,
        /// Exact candidate reply bytes, without an origin assertion.
        pub reply: Vec<u8>,
        /// The original candidate effort metadata.
        pub effort: Option<Effort>,
    }

    /// The actual queued write's immutable key, value and metadata.
    #[derive(Debug)]
    pub struct CandidateWrite<Effort> {
        key: [u8; 32],
        value: std::sync::Arc<CandidateValue<Effort>>,
        command: String,
        provenance: String,
        solver: String,
    }

    /// Direct borrows from the original candidate, consumed by the native disk call.
    pub struct WriteArguments<'a, Effort> {
        key: &'a [u8; 32],
        mode: &'a str,
        reply: &'a [u8],
        effort: Option<&'a Effort>,
        command: &'a str,
        provenance: &'a str,
        solver: &'a str,
    }

    /// The real paired preparation result; failure returns the original candidate.
    pub type Preparation<Endpoint, Effort, Stamp> = Result<(
        Entry<std::sync::Arc<CandidateValue<Effort>>, Stamp>, Prepared<Endpoint, Effort>
    ), CandidateWrite<Effort>>;

    /// Owns the actual original candidate and optional insertion-issued handle.
    /// No constructor accepts an independently supplied candidate and ticket.
    #[derive(Debug)]
    pub struct Prepared<Endpoint, Effort> {
        candidate: CandidateWrite<Effort>,
        pending: Option<Held<Endpoint>>,
    }
}

sail_code! {
    impl<Effort> CandidateWrite<Effort> {
        sail_impl_spec! {
            /// Exact raw key.
            pub closed spec fn key_view(&self) -> Seq<u8> { self.key@ }
            /// The original held native value, forwarded rather than compared.
            pub closed spec fn value_view(&self) -> std::sync::Arc<CandidateValue<Effort>> { self.value }
            /// Exact original command.
            pub closed spec fn command_view(&self) -> Seq<char> { self.command@ }
            /// Exact original provenance.
            pub closed spec fn provenance_view(&self) -> Seq<char> { self.provenance@ }
            /// Exact original solver summary.
            pub closed spec fn solver_view(&self) -> Seq<char> { self.solver@ }
        }
        sail_method! {
            /// Retain a raw candidate without granting solver-origin authority.
            pub fn new(key: [u8; 32], value: std::sync::Arc<CandidateValue<Effort>>,
                command: String, provenance: String, solver: String) -> (out: Self)
                requires (true)
                ensures (out.key_view() == key@, out.value_view() == value,
                    out.command_view() == command@, out.provenance_view() == provenance@,
                    out.solver_view() == solver@)
            { Self { key, value, command, provenance, solver } }
        }
        sail_method! {
            /// Return the original raw key.
            pub fn key(&self) -> (out: [u8; 32])
                requires (true) ensures (out@ == self.key_view())
            { self.key }
        }
        sail_method! {
            /// Borrow the original value; no mutable replacement is exposed.
            pub fn value(&self) -> (out: &std::sync::Arc<CandidateValue<Effort>>)
                requires (true) ensures (*out == self.value_view())
            { &self.value }
        }
        sail_method! {
            /// Project every field of the actual disk request from this candidate.
            pub fn arguments(&self) -> (out: WriteArguments<'_, Effort>)
                requires (true)
                ensures (out.key_view() == self.key_view(),
                    out.mode_view() == self.value_view().mode@,
                    out.reply_view() == self.value_view().reply@,
                    out.effort_view() == self.value_view().effort,
                    out.command_view() == self.command_view(),
                    out.provenance_view() == self.provenance_view(), out.solver_view() == self.solver_view())
            {
                WriteArguments { key: &self.key, mode: self.value.mode.as_str(),
                    reply: self.value.reply.as_slice(), effort: self.value.effort.as_ref(),
                    command: self.command.as_str(), provenance: self.provenance.as_str(),
                    solver: self.solver.as_str() }
            }
        }
    }

    impl<'a, Effort> WriteArguments<'a, Effort> {
        sail_impl_spec! {
            /// Original raw key.
            pub closed spec fn key_view(&self) -> Seq<u8> { self.key@ }
            /// Original namespace.
            pub closed spec fn mode_view(&self) -> Seq<char> { self.mode@ }
            /// Original payload.
            pub closed spec fn reply_view(&self) -> Seq<u8> { self.reply@ }
            /// Original effort metadata.
            pub closed spec fn effort_view(&self) -> Option<Effort> {
                match self.effort { Some(value) => Some(*value), None => None }
            }
            /// Original command.
            pub closed spec fn command_view(&self) -> Seq<char> { self.command@ }
            /// Original provenance.
            pub closed spec fn provenance_view(&self) -> Seq<char> { self.provenance@ }
            /// Original solver summary.
            pub closed spec fn solver_view(&self) -> Seq<char> { self.solver@ }
        }
        sail_method! {
            /// Return the exact original key.
            pub fn key(&self) -> (out: [u8; 32]) requires (true) ensures (out@ == self.key_view())
            { *self.key }
        }
        sail_method! {
            /// Borrow the original mode.
            pub fn mode(&self) -> (out: &'a str) requires (true) ensures (out@ == self.mode_view())
            { self.mode }
        }
        sail_method! {
            /// Borrow the original reply.
            pub fn reply(&self) -> (out: &'a [u8]) requires (true) ensures (out@ == self.reply_view())
            { self.reply }
        }
        sail_method! {
            /// Borrow the original effort.
            pub fn effort(&self) -> (out: Option<&'a Effort>) requires (true)
                ensures (match out { Some(value) => Some(*value), None => None } == self.effort_view())
            { self.effort }
        }
        sail_method! {
            /// Borrow the original command.
            pub fn command(&self) -> (out: &'a str) requires (true) ensures (out@ == self.command_view())
            { self.command }
        }
        sail_method! {
            /// Borrow the original provenance.
            pub fn provenance(&self) -> (out: &'a str) requires (true) ensures (out@ == self.provenance_view())
            { self.provenance }
        }
        sail_method! {
            /// Borrow the original solver summary.
            pub fn solver(&self) -> (out: &'a str) requires (true) ensures (out@ == self.solver_view())
            { self.solver }
        }
    }

    impl<Endpoint, Effort> Prepared<Endpoint, Effort> {
        sail_impl_spec! {
            /// The complete original candidate.
            pub closed spec fn candidate_view(&self) -> CandidateWrite<Effort> { self.candidate }
            /// The original insertion-issued endpoint and ticket, if memory admitted it.
            pub closed spec fn pending_view(&self) -> Option<Held<Endpoint>> { self.pending }
        }
        sail_method! {
            /// Keep disk publication available when memory cannot retain a candidate.
            pub fn without_memory(candidate: CandidateWrite<Effort>) -> (out: Self)
                requires (true) ensures (out.candidate_view() == candidate, out.pending_view().is_none())
            { Self { candidate, pending: None } }
        }
        sail_method! {
            /// Borrow the complete original candidate for the actual disk call.
            pub fn candidate(&self) -> (out: &CandidateWrite<Effort>)
                requires (true) ensures (*out == self.candidate_view())
            { &self.candidate }
        }
        sail_method! {
            /// Consume the actual record after the actual disk operation returns.
            pub fn into_parts(self) -> (out: (CandidateWrite<Effort>, Option<Held<Endpoint>>))
                requires (true) ensures (out.0 == self.candidate_view(), out.1 == self.pending_view())
            { (self.candidate, self.pending) }
        }
    }

    impl Clock {
        sail_method! {
            /// Move the real candidate and bind its original value clone, slot and
            /// private pending endpoint in one operation. No ticket is an input.
            /// # Errors
            /// Returns the original candidate when the lifetime clock is exhausted.
            pub fn prepare<Endpoint, Effort, Stamp>(&mut self, endpoint: Endpoint,
                candidate: CandidateWrite<Effort>)
                -> (out: Preparation<Endpoint, Effort, Stamp>)
                requires (true)
                ensures (out.is_ok() == (old(self).last_view() < u64::MAX),
                    final(self).last_view() >= old(self).last_view(),
                    match out {
                        Ok((entry, prepared)) => entry.0@ == candidate.key_view()
                            && entry.1.value_view() == candidate.value_view()
                            && matches!(entry.1.state_view(), SlotState::Pending)
                            && entry.1.generation_view() == final(self).last_view()
                            && entry.1.tick_view() == final(self).last_view()
                            && prepared.candidate_view() == candidate
                            && prepared.pending_view().is_some()
                            && prepared.pending_view().unwrap().endpoint_view() == endpoint
                            && prepared.pending_view().unwrap().ticket_view().key_view() == candidate.key_view()
                            && prepared.pending_view().unwrap().ticket_view().generation_view() == final(self).last_view()
                            && final(self).last_view() as int == old(self).last_view() as int + 1,
                        Err(original) => original == candidate && final(self).last_view() == u64::MAX,
                    })
            {
                let value = std::sync::Arc::clone(&candidate.value);
                let Some((entry, held)) = self.entry_bound(endpoint, candidate.key, value,
                    SlotState::<Stamp>::Pending) else { return Err(candidate); };
                Ok((entry, Prepared { candidate, pending: Some(held) }))
            }
        }
    }
}

sail_verified! {
    /// Project only the actual returned disk Result; no success flag is accepted.
    pub fn observed_completion<Stamp, Failure>(result: &Result<Publication<Stamp>, Failure>)
        -> (out: Completion<Stamp>)
        where (Stamp: Copy)
        requires (true)
        ensures (out == match result {
            Ok(Publication::Written(stamp)) => Completion::Written(*stamp),
            Ok(Publication::Exists) => Completion::Exists,
            Err(_) => Completion::Failed,
        })
    {
        match result {
            Ok(Publication::Written(stamp)) => Completion::Written(*stamp),
            Ok(Publication::Exists) => Completion::Exists,
            Err(_) => Completion::Failed,
        }
    }
}

sail_verified! {
    /// A nonempty supplied candidate traverses the actual paired factory,
    /// argument projection, returned-outcome projection and original slot update.
    pub fn prepared_transfer_control<Endpoint>(endpoint: Endpoint, candidate: CandidateWrite<u64>)
        -> (out: bool)
        requires (candidate.value_view().reply@.len() > 0)
        ensures (out)
    {
        let mut clock = Clock::start();
        let Ok((entry, prepared)) = clock.prepare::<Endpoint, u64, u64>(endpoint, candidate)
            else { return false; };
        let arguments = prepared.candidate().arguments();
        if arguments.reply().is_empty() { return false; }
        let returned: Result<Publication<u64>, u8> = Ok(Publication::Written(17));
        let completion = observed_completion(&returned);
        let (_, pending) = prepared.into_parts();
        let Some(held) = pending else { return false; };
        let updated = complete(entry, held.ticket(), completion);
        matches!(updated, EntryUpdate::Written(_))
    }
}
