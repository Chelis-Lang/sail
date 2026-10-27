//! Exact preexisting v2 byte-format controls independent of the checked encoder.
use sail_key::{Mode, PrefixHasher, SessionPrefix, SolverIdentity};

#[test]
fn original_v2_digest_and_silence_vectors_survive_all_fragment_boundaries() {
    let solver = SolverIdentity {
        name: "z3".into(),
        version: "4.16.0".into(),
        binary_sha256: core::array::from_fn(|i| u8::try_from(i).unwrap_or_default()),
        platform: "linux-x86_64".into(),
    };
    let raw = b"; raw key fixture\n(assert (str.contains \"a;()\" \"a\"))\n(check-sat)";
    // Generated independently from the existing length-prefixed wire format
    // with Python hashlib/struct, rather than through PrefixHasher.
    let cases = [
        (
            Mode::Default,
            "b7abc68dabd2c28423d6da8216e67fc8b3a9fc0716f189544a0f64268efaa10b",
            "38d2bca829c4021ca6449acad7714a6db6c86446cab7715a1782585f2855478d",
        ),
        (
            Mode::OptIn(String::new()),
            "20aec4c0e1218c1dfd7974c1a19a4dfea0023d4abfe9eed5e65bd08f4bd0873a",
            "a0f831becd7c10ce1d76ad2e69551e0f6b8bfb9eac38b0210b189a817add30d8",
        ),
        (
            Mode::OptIn("accept-first-proof".into()),
            "55b7c87eaa4ad7fc635a2ebd50c389ba4b6ff7a84ebc71368e730edb02820141",
            "fdf329935614ebef37fc28ea4eeb7013f041ecd1b825415d8b82e802a35363b5",
        ),
    ];
    for (mode, key, silence) in cases {
        for split in 0..=raw.len() {
            let mut hasher = PrefixHasher::new(&solver, &mode);
            hasher.update(&raw[..split]);
            let mut copied = hasher.clone();
            hasher.update(&raw[split..]);
            copied.update(&raw[split..]);
            assert!(hasher.is_exact());
            assert_eq!(hasher.len(), u64::try_from(raw.len()).unwrap_or_default());
            assert_eq!(hasher.key().to_string(), key);
            assert_eq!(hasher.silence_key().to_string(), silence);
            assert_eq!(copied.key(), hasher.key());
        }
    }
}

#[test]
fn native_shared_primitive_controls_execute() {
    assert!(sail_acceptance::key_prefix::nonempty_framing_control());
    assert!(sail_acceptance::key_prefix::overflow_framing_control());
    assert!(sail_acceptance::key_prefix::owned_feed_control());
    assert!(sail_acceptance::key_prefix::joint_owner_control());
    assert!(sail_acceptance::key_prefix::owner_append_items_control());
    assert!(sail_acceptance::query::commented_head_control());
    assert!(sail_acceptance::query_policy::tactic_policy_control());
    assert!(sail_acceptance::query_split::outer_atom_offsets_control());
    assert!(sail_acceptance::query_split::outer_crlf_finish_control());
    assert!(sail_acceptance::query_split::outer_chunks_control());
    assert!(sail_acceptance::query_split::outer_clone_initial_control());
}

#[test]
fn actual_command_items_share_the_original_raw_owner_at_every_boundary() {
    use sail_acceptance::query_split::ItemKind;
    let solver = SolverIdentity {
        name: "z3".into(),
        version: "4.16.0".into(),
        binary_sha256: [7; 32],
        platform: "linux-x86_64".into(),
    };
    let raw = b";c\r\na)()\r\nfoo";
    for boundary in 0..=raw.len() {
        let mut owner = SessionPrefix::new(&solver, &Mode::Default);
        let mut items = owner.append_items(&raw[..boundary]);
        items.extend(owner.append_items(&raw[boundary..]));
        assert_eq!(owner.bytes(), raw);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].kind, ItemKind::Atom);
        assert_eq!(items[0].offset, 4);
        assert_eq!(items[0].bytes, b"a");
        assert_eq!(items[1].kind, ItemKind::Stray);
        assert_eq!(items[1].offset, 5);
        assert_eq!(items[1].bytes, b")");
        assert_eq!(items[2].kind, ItemKind::List);
        assert_eq!(items[2].offset, 6);
        assert_eq!(items[2].bytes, b"()");
        assert!(owner.has_pending_item());
        let terminated = owner.append_items(b"\n");
        assert_eq!(terminated.len(), 1);
        assert_eq!(terminated[0].kind, ItemKind::Atom);
        assert_eq!(terminated[0].offset, 10);
        assert_eq!(terminated[0].bytes, b"foo");
        assert!(!owner.has_pending_item());
        assert_eq!(owner.bytes(), [raw.as_slice(), b"\n"].concat());
        assert!(owner.hash_tail());
        let mut reference = PrefixHasher::new(&solver, &Mode::Default);
        reference.update(raw);
        reference.update(b"\n");
        assert_eq!(owner.silence_key(), reference.silence_key());
    }
}

#[test]
fn input_carriage_return_terminates_atoms_at_every_raw_boundary() {
    use sail_acceptance::query_split::ItemKind;
    let solver = SolverIdentity {
        name: "z3".into(),
        version: "4.16.0".into(),
        binary_sha256: [7; 32],
        platform: "linux-x86_64".into(),
    };
    let raw = b";c\r\na)()\r\nfoo\r";
    for boundary in 0..=raw.len() {
        let mut owner = SessionPrefix::new(&solver, &Mode::Default);
        let mut items = owner.append_items(&raw[..boundary]);
        items.extend(owner.append_items(&raw[boundary..]));
        assert_eq!(owner.bytes(), raw);
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].kind, ItemKind::Atom);
        assert_eq!(items[0].offset, 4);
        assert_eq!(items[0].bytes, b"a");
        assert_eq!(items[1].kind, ItemKind::Stray);
        assert_eq!(items[1].offset, 5);
        assert_eq!(items[1].bytes, b")");
        assert_eq!(items[2].kind, ItemKind::List);
        assert_eq!(items[2].offset, 6);
        assert_eq!(items[2].bytes, b"()");
        assert_eq!(items[3].kind, ItemKind::Atom);
        assert_eq!(items[3].offset, 10);
        assert_eq!(items[3].bytes, b"foo");
        assert!(!owner.has_pending_item());
        assert_eq!(
            owner.append_items(b"\n"),
            [] as [sail_acceptance::query_split::Item; 0]
        );
        assert!(!owner.has_pending_item());
        assert_eq!(owner.bytes(), [raw.as_slice(), b"\n"].concat());
        assert!(owner.hash_tail());
        let mut reference = PrefixHasher::new(&solver, &Mode::Default);
        reference.update(raw);
        reference.update(b"\n");
        assert_eq!(owner.silence_key(), reference.silence_key());
    }
}

#[test]
fn joint_owner_retains_raw_bytes_and_command_keys_across_fragmentation() {
    let solver = SolverIdentity {
        name: "z3".into(),
        version: "4.16.0".into(),
        binary_sha256: [7; 32],
        platform: "linux-x86_64".into(),
    };
    let leading = b"; original\r\n";
    let assertion = b"(assert (str.contains \"a;()\" \"a\"))";
    let check = b"(check-sat)";
    let tail = b" ; unchanged tail\n";
    let mut raw = Vec::new();
    raw.extend_from_slice(leading);
    raw.extend_from_slice(assertion);
    raw.push(b'\n');
    raw.extend_from_slice(check);
    raw.extend_from_slice(tail);
    let check_offset = leading.len() + assertion.len() + 1;
    for mode in [
        Mode::Default,
        Mode::OptIn(String::new()),
        Mode::OptIn("accept-first-proof".into()),
    ] {
        for split in 0..=raw.len() {
            let mut owner = SessionPrefix::new(&solver, &mode);
            owner.append(&raw[..split]);
            owner.append(&raw[split..]);
            assert_eq!(owner.bytes(), raw);
            let first = owner.command(leading.len(), assertion);
            assert!(first.is_some());
            assert_eq!(
                first.as_ref().map(sail_key::CommandFrame::policy),
                Some(sail_acceptance::query_policy::ResponsePolicy::None)
            );
            let mut reference = PrefixHasher::new(&solver, &mode);
            reference.update(&raw[..leading.len() + assertion.len()]);
            assert_eq!(
                first.as_ref().map(sail_key::CommandFrame::key),
                Some(reference.key())
            );
            let second = owner.command(check_offset, check);
            assert_eq!(
                second.as_ref().map(sail_key::CommandFrame::policy),
                Some(sail_acceptance::query_policy::ResponsePolicy::One)
            );
            assert_eq!(
                second.as_ref().map(sail_key::CommandFrame::head),
                Some(check.as_slice().get(1..10).unwrap_or_default())
            );
            reference.update(&raw[leading.len() + assertion.len()..check_offset + check.len()]);
            assert_eq!(
                second.as_ref().map(sail_key::CommandFrame::key),
                Some(reference.key())
            );
            assert!(owner.trailing_trivia());
            assert!(owner.hash_tail());
            reference.update(tail);
            assert_eq!(owner.key(), reference.key());
            assert_eq!(owner.silence_key(), reference.silence_key());
            assert_eq!(owner.bytes(), raw);
            owner.refuse();
            owner.append(b"\x00still replayed");
            assert!(!owner.valid());
            assert!(!owner.hash_tail());
            assert_eq!(
                owner.bytes().get(raw.len()..),
                Some(b"\x00still replayed".as_slice())
            );
        }
    }
}
