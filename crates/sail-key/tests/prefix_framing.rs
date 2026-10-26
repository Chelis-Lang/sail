//! Exact preexisting v2 byte-format controls independent of the checked encoder.
use sail_key::{Mode, PrefixHasher, SolverIdentity};

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
}
