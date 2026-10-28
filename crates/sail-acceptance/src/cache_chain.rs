//! Actual owned collision links with checked iterative destruction.
//!
//! This source is staged for the slot-map implementation. It preserves original
//! entries; it does not select native storage or certify solver origin.
#[cfg(sail_verus)]
use vstd::prelude::*;

use crate::cache_candidate::Entry;
use crate::cache_candidate::Slot;
use crate::inputs::byte_eq;

sail_code! {
    /// Owns every actual collision node, including during clear and destruction.
    pub struct Chain<Value, Stamp> {
        head: Option<Box<Node<Value, Stamp>>>,
    }

    struct Node<Value, Stamp> {
        entry: Entry<Value, Stamp>,
        next: Chain<Value, Stamp>,
    }
}

sail_code! {
    impl<Value, Stamp> Chain<Value, Stamp> {
        sail_impl_spec! {
            /// Actual original entries in link order; no copied runtime ledger.
            pub closed spec fn entries_view(&self) -> Seq<Entry<Value, Stamp>>
                decreases self,
            {
                match self.head {
                    None => Seq::empty(),
                    Some(node) => seq![node.entry] + node.next.entries_view(),
                }
            }
            /// Structural termination measure, independent of a supplied counter.
            pub closed spec fn length_view(&self) -> nat
                decreases self,
            {
                match self.head {
                    None => 0,
                    Some(node) => 1 + node.next.length_view(),
                }
            }
            /// Final empty-tail condition required before native field drop.
            pub closed spec fn empty_view(&self) -> bool { self.head.is_none() }
            /// Relate the actual head discriminator to the original entry domain.
            pub proof fn empty_entries(&self)
                ensures self.empty_view() == (self.entries_view().len() == 0),
            { }
        }

        sail_method! {
            /// Observe the actual padded private node carrier. This is a native
            /// type-layout observation, not allocator bookkeeping or RSS.
            pub fn node_bytes() -> (out: usize)
                requires (true)
                ensures (out as nat == vstd::layout::size_of::<Node<Value, Stamp>>())
            { std::mem::size_of::<Node<Value, Stamp>>() }
        }

        sail_method! {
            /// Construct only an actually empty chain.
            pub fn empty() -> (out: Self)
                requires (true)
                ensures (out.empty_view(), out.entries_view() == Seq::<Entry<Value, Stamp>>::empty(),
                    out.length_view() == 0)
            { Self { head: None } }
        }

        sail_method! {
            /// Move the exact original entry and owned tail into one real node.
            pub fn prepend(entry: Entry<Value, Stamp>, next: Self) -> (out: Self)
                requires (true)
                ensures (!out.empty_view(), out.entries_view() == seq![entry] + next.entries_view(),
                    out.length_view() == next.length_view() + 1)
            { Self { head: Some(Box::new(Node { entry, next })) } }
        }

        sail_no_unwind_method! {
            /// Observe actual emptiness without traversing or cloning an endpoint.
            #[allow(clippy::redundant_pattern_matching, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            pub fn is_empty(&self) -> (out: bool)
                requires (true) ensures (out == self.empty_view())
            { matches!(&self.head, None) }
        }

        sail_no_unwind_method! {
            /// Detach the original Box for allocation-free transfer between chains.
            #[allow(clippy::question_mark, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            fn pop_node(&mut self) -> (out: Option<Box<Node<Value, Stamp>>>)
                requires (true)
                ensures (match out {
                    Some(node) => old(self).entries_view().len() > 0
                        && node.entry == old(self).entries_view()[0]
                        && node.next.empty_view()
                        && final(self).entries_view() == old(self).entries_view().skip(1)
                        && final(self).length_view() + 1 == old(self).length_view(),
                    None => old(self).empty_view() && final(self).empty_view()
                        && final(self).entries_view() == old(self).entries_view(),
                })
            {
                let mut current = None;
                std::mem::swap(&mut self.head, &mut current);
                let Some(mut node) = current else { return None; };
                std::mem::swap(&mut self.head, &mut node.next.head);
                sail_proof! { assert(node.next.empty_view()); }
                Some(node)
            }
        }

        sail_no_unwind_method! {
            /// Reuse an already detached original Box, without allocating or cloning.
            fn prepend_node(&mut self, mut node: Box<Node<Value, Stamp>>) -> (out: ())
                requires (node.next.empty_view())
                ensures (final(self).entries_view() == seq![node.entry] + old(self).entries_view(),
                    final(self).length_view() == old(self).length_view() + 1)
            {
                std::mem::swap(&mut self.head, &mut node.next.head);
                sail_proof! { assert(self.empty_view()); }
                self.head = Some(node);
            }
        }

        sail_no_unwind_method! {
            /// Move out only the original first entry and detach its actual tail.
            #[allow(clippy::question_mark, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            pub fn pop(&mut self) -> (out: Option<Entry<Value, Stamp>>)
                requires (true)
                ensures (match out {
                    Some(entry) => old(self).entries_view().len() > 0
                        && entry == old(self).entries_view()[0]
                        && final(self).entries_view() == old(self).entries_view().skip(1)
                        && final(self).length_view() + 1 == old(self).length_view(),
                    None => old(self).empty_view() && final(self).empty_view()
                        && final(self).entries_view() == old(self).entries_view(),
                })
            {
                let Some(node) = self.pop_node() else { return None; };
                let Node { entry, next: _next } = *node;
                sail_proof! { assert(_next.empty_view()); }
                Some(entry)
            }
        }

        sail_method! {
            /// Return only a slot whose original complete32-byte key matches.
            /// Equal route prefixes alone never select a neighbor.
            #[allow(clippy::question_mark, clippy::redundant_pattern_matching, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            pub fn get<'a>(&'a self, key: &[u8; 32]) -> (out: Option<&'a Slot<Value, Stamp>>)
                requires (true)
                ensures (match out {
                    Some(slot) => exists|i: int| 0 <= i < self.entries_view().len()
                        && self.entries_view()[i].0@ == key@
                        && *slot == self.entries_view()[i].1,
                    None => forall|i: int| 0 <= i < self.entries_view().len() ==>
                        self.entries_view()[i].0@ != key@,
                })
            {
                let mut cursor = self;
                sail_snapshot!(mut skipped = Seq::<Entry<Value, Stamp>>::empty());
                sail_while! { (matches!(&cursor.head, Some(_))) invariant (
                    self.entries_view() == skipped + cursor.entries_view(),
                    forall|i: int| 0 <= i < skipped.len() ==> skipped[i].0@ != key@)
                    decreases (cursor.length_view()) {
                    let Some(node) = cursor.head.as_ref() else { return None; };
                    if byte_eq(&node.entry.0, key) {
                        sail_proof! {
                            let index = skipped.len();
                            assert(cursor.entries_view() == seq![node.entry] + node.next.entries_view());
                            assert(0 <= index < self.entries_view().len());
                            assert(self.entries_view()[index as int] == node.entry);
                            assert(exists|i: int| 0 <= i < self.entries_view().len()
                                && self.entries_view()[i].0@ == key@
                                && node.entry.1 == self.entries_view()[i].1) by {
                                assert(self.entries_view()[index as int].0@ == key@);
                                assert(node.entry.1 == self.entries_view()[index as int].1);
                            }
                        }
                        return Some(&node.entry.1);
                    }
                    sail_proof! { skipped = skipped.push(node.entry); }
                    cursor = &node.next;
                } }
                None
            }
        }

        sail_no_unwind_method! {
            /// Restore the actual reversed visited nodes without new allocations.
            #[allow(clippy::redundant_pattern_matching, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            fn restore_prefix(&mut self, prefix: &mut Self) -> (out: ())
                requires (true)
                ensures (final(prefix).empty_view(),
                    final(self).entries_view() == old(prefix).entries_view().reverse() + old(self).entries_view())
            {
                sail_snapshot!(original_prefix = prefix.entries_view());
                sail_snapshot!(original_self = self.entries_view());
                sail_while! { (matches!(&prefix.head, Some(_))) invariant (
                    prefix.entries_view().reverse() + self.entries_view()
                        == original_prefix.reverse() + original_self)
                    decreases (prefix.length_view()) {
                    let Some(node) = prefix.pop_node() else { return; };
                    self.prepend_node(node);
                } }
            }
        }

        sail_method! {
            /// Remove only the original complete-key entry and retain every
            /// nonselected Box/Slot in its original relative order.
            #[allow(clippy::redundant_pattern_matching, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            pub fn take(&mut self, key: &[u8; 32]) -> (out: Option<Entry<Value, Stamp>>)
                requires (true)
                ensures (match out {
                    Some(entry) => exists|i: int| 0 <= i < old(self).entries_view().len()
                        && entry == old(self).entries_view()[i] && entry.0@ == key@
                        && final(self).entries_view() == old(self).entries_view().remove(i)
                        && forall|j: int| 0 <= j < i ==> old(self).entries_view()[j].0@ != key@,
                    None => final(self).entries_view() == old(self).entries_view()
                        && forall|i: int| 0 <= i < old(self).entries_view().len() ==>
                            old(self).entries_view()[i].0@ != key@,
                })
            {
                sail_snapshot!(original = self.entries_view());
                let mut skipped = Self::empty();
                sail_while! { (matches!(&self.head, Some(_))) invariant (
                    original == old(self).entries_view(),
                    original == skipped.entries_view().reverse() + self.entries_view(),
                    forall|i: int| 0 <= i < skipped.entries_view().len() ==>
                        skipped.entries_view()[i].0@ != key@,
                    forall|i: int| 0 <= i < skipped.entries_view().reverse().len() ==>
                        skipped.entries_view().reverse()[i].0@ != key@)
                    decreases (self.length_view()) {
                    sail_proof! { assert(!self.empty_view()); }
                    // None has an empty head; rechecking the loop condition then
                    // takes the same normal restoration path as exhaustion.
                    let Some(node) = self.pop_node() else { continue; };
                    if byte_eq(&node.entry.0, key) {
                        sail_snapshot!(visited = skipped.entries_view().reverse());
                        sail_snapshot!(remaining = self.entries_view());
                        sail_proof! {
                            let index = visited.len();
                            assert(original == visited + seq![node.entry] + remaining);
                            assert(0 <= index < original.len());
                            assert(original[index as int] == node.entry);
                            assert(original.remove(index as int) == visited + remaining) by { }
                            assert forall|j: int| 0 <= j < index implies original[j].0@ != key@ by {
                                assert(original[j] == visited[j]);
                            }
                        }
                        self.restore_prefix(&mut skipped);
                        let Node { entry, next: _next } = *node;
                        sail_proof! {
                            assert(_next.empty_view());
                            assert(original == old(self).entries_view());
                            let index = visited.len();
                            assert(self.entries_view().len() == visited.len() + remaining.len()) by { }
                            assert(self.entries_view() == visited + remaining) by { }
                            assert(exists|i: int| 0 <= i < original.len()
                                && entry == original[i] && entry.0@ == key@
                                && self.entries_view() == original.remove(i)
                                && forall|j: int| 0 <= j < i ==> original[j].0@ != key@) by {
                                assert(entry == original[index as int]);
                            }
                        }
                        return Some(entry);
                    }
                    skipped.prepend_node(node);
                } }
                sail_proof! {
                    self.empty_entries();
                    assert(self.empty_view());
                    assert(original == skipped.entries_view().reverse());
                    assert forall|i: int| 0 <= i < original.len() implies original[i].0@ != key@ by { }
                }
                self.restore_prefix(&mut skipped);
                None
            }
        }

        sail_method! {
            /// Retick only the original first complete-key slot, retaining its
            /// Box, payload, insertion generation, state and all neighbor order.
            #[allow(clippy::redundant_pattern_matching, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            pub fn touch(&mut self, key: &[u8; 32], tick: u64) -> (out: Option<u64>)
                requires (true)
                ensures (match out {
                    Some(previous) => exists|i: int| 0 <= i < old(self).entries_view().len()
                        && old(self).entries_view()[i].0@ == key@
                        && old(self).entries_view()[i].1.tick_view() == previous
                        && final(self).entries_view() == old(self).entries_view().update(i,
                            (old(self).entries_view()[i].0,
                                old(self).entries_view()[i].1.reticked_view(tick)))
                        && forall|j: int| 0 <= j < i ==> old(self).entries_view()[j].0@ != key@,
                    None => final(self).entries_view() == old(self).entries_view()
                        && forall|i: int| 0 <= i < old(self).entries_view().len() ==>
                            old(self).entries_view()[i].0@ != key@,
                })
            {
                sail_snapshot!(original = self.entries_view());
                let mut skipped = Self::empty();
                sail_while! { (matches!(&self.head, Some(_))) invariant (
                    original == old(self).entries_view(),
                    original == skipped.entries_view().reverse() + self.entries_view(),
                    forall|i: int| 0 <= i < skipped.entries_view().len() ==>
                        skipped.entries_view()[i].0@ != key@,
                    forall|i: int| 0 <= i < skipped.entries_view().reverse().len() ==>
                        skipped.entries_view().reverse()[i].0@ != key@)
                    decreases (self.length_view()) {
                    let Some(mut node) = self.pop_node() else { continue; };
                    if byte_eq(&node.entry.0, key) {
                        sail_snapshot!(visited = skipped.entries_view().reverse());
                        sail_snapshot!(remaining = self.entries_view());
                        sail_snapshot!(before = node.entry);
                        let previous = node.entry.1.tick();
                        node.entry.1.touch(tick);
                        sail_proof! {
                            Slot::reticked_exact(before.1, node.entry.1, tick);
                            let index = visited.len();
                            assert(original == visited + seq![before] + remaining);
                            assert(0 <= index < original.len());
                            assert(original[index as int] == before);
                            assert(original.update(index as int, node.entry)
                                == visited + seq![node.entry] + remaining);
                            assert forall|j: int| 0 <= j < index implies original[j].0@ != key@ by {
                                assert(original[j] == visited[j]);
                            }
                        }
                        sail_snapshot!(updated = node.entry);
                        self.prepend_node(node);
                        self.restore_prefix(&mut skipped);
                        sail_proof! {
                            assert(original == old(self).entries_view());
                            let index = visited.len();
                            assert(self.entries_view().len() == visited.len() + 1 + remaining.len()) by { }
                            assert(self.entries_view() == visited + seq![updated] + remaining) by { }
                            assert(exists|i: int| 0 <= i < original.len()
                                && original[i].0@ == key@ && original[i].1.tick_view() == previous
                                && self.entries_view() == original.update(i,
                                    (original[i].0, original[i].1.reticked_view(tick)))
                                && forall|j: int| 0 <= j < i ==> original[j].0@ != key@) by {
                                assert(original[index as int] == before);
                            }
                        }
                        return Some(previous);
                    }
                    skipped.prepend_node(node);
                } }
                sail_proof! {
                    self.empty_entries();
                    assert(original == skipped.entries_view().reverse());
                    assert forall|i: int| 0 <= i < original.len() implies original[i].0@ != key@ by { }
                }
                self.restore_prefix(&mut skipped);
                None
            }
        }

        sail_method! {
            /// Install this exact entry, returning only the original first full-key
            /// match. Every other original node retains its relative order.
            pub fn insert(&mut self, entry: Entry<Value, Stamp>) -> (out: Option<Entry<Value, Stamp>>)
                requires (true)
                ensures (final(self).entries_view().len() > 0,
                    final(self).entries_view()[0] == entry,
                    match out {
                        Some(original) => original.0@ == entry.0@
                            && exists|i: int| 0 <= i < old(self).entries_view().len()
                                && original == old(self).entries_view()[i]
                                && final(self).entries_view() == seq![entry] + old(self).entries_view().remove(i),
                        None => final(self).entries_view() == seq![entry] + old(self).entries_view()
                            && forall|i: int| 0 <= i < old(self).entries_view().len() ==>
                                old(self).entries_view()[i].0@ != entry.0@,
                    })
            {
                let key = entry.0;
                let original = self.take(&key);
                let mut tail = Self::empty();
                std::mem::swap(&mut self.head, &mut tail.head);
                sail_proof! { assert(self.empty_view()); }
                *self = Self::prepend(entry, tail);
                original
            }
        }

        sail_no_unwind_method! {
            /// Drop every actual node iteratively, with checked structural progress.
            #[allow(clippy::redundant_pattern_matching, reason = "Preserve the source-shared checked body, explicit variant/refusal paths and complete fault controls.")]
            pub fn drain(&mut self) -> (out: ())
                requires (true) ensures (final(self).empty_view())
            {
                sail_while! { (matches!(&self.head, Some(_))) invariant (true)
                    decreases (self.length_view()) {
                    sail_snapshot!(previous_length = self.length_view());
                    let mut current = None;
                    std::mem::swap(&mut self.head, &mut current);
                    let Some(mut node) = current else { return; };
                    std::mem::swap(&mut self.head, &mut node.next.head);
                    sail_proof! {
                        assert(node.next.empty_view());
                        assert(self.length_view() < previous_length);
                    }
                } }
            }
        }
    }

    impl<Value, Stamp> Drop for Chain<Value, Stamp> {
        sail_drop! {
            fn drop(&mut self) {
                self.drain();
                sail_proof! { assert(self.empty_view()); }
            }
        }
    }
}
