//! Supported successful-command output policy over exact validated head bytes.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// The response count for successful commands when print-success is disabled.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ResponsePolicy {
        /// A successful command has no response; errors remain possible.
        None,
        /// Exactly one response, including check results and queries.
        One,
        /// The command is outside this policy and cannot drive cache pairing.
        Unknown,
    }
}

sail_spec! {
    #[verifier::opaque]
    pub open spec fn named(head: Seq<u8>, word: Seq<char>) -> bool {
        head == vstd::utf8::encode_utf8(word)
    }
    #[verifier::opaque]
    pub open spec fn prefixed(head: Seq<u8>, word: Seq<char>) -> bool {
        let bytes = vstd::utf8::encode_utf8(word);
        bytes.len() <= head.len() && head.take(bytes.len() as int) == bytes
    }
    pub open spec fn one_head(head: Seq<u8>) -> bool {
        named(head,"check-sat"@) || named(head,"check-sat-assuming"@) || named(head,"check-sat-using"@) || named(head,"echo"@) || prefixed(head,"get-"@)
    }
    pub open spec fn silent_head(head: Seq<u8>) -> bool {
        named(head,"push"@)
            || named(head,"pop"@)
            || named(head,"assert"@)
            || named(head,"declare-fun"@)
            || named(head,"declare-const"@)
            || named(head,"declare-sort"@)
            || named(head,"declare-datatype"@)
            || named(head,"declare-datatypes"@)
            || named(head,"define-fun"@)
            || named(head,"define-fun-rec"@)
            || named(head,"define-funs-rec"@)
            || named(head,"define-sort"@)
            || named(head,"define-const"@)
            || named(head,"set-option"@)
            || named(head,"set-info"@)
            || named(head,"set-logic"@)
            || named(head,"reset"@)
            || named(head,"reset-assertions"@)
            || named(head,"exit"@)
    }
}

sail_verified! {
    /// Exact original-byte comparison to a fixed UTF-8 grammar word.
    pub fn word_eq(head: &[u8], word: &str) -> (same: bool)
        requires (true) ensures (same == named(head@,word@))
    {
        sail_proof! { reveal(named); }
        let bytes = word.as_bytes();
        crate::inputs::byte_eq(head,bytes)
    }
}
sail_verified! {
    /// Tests a grammar prefix without changing or allocating the caller's bytes.
    pub fn word_prefix(head: &[u8], word: &str) -> (same: bool)
        requires (true) ensures (same == prefixed(head@,word@))
    {
        sail_proof! { reveal(prefixed); }
        let bytes = word.as_bytes();
        if head.len() < bytes.len() { return false; }
        crate::inputs::byte_eq(&head[..bytes.len()],bytes)
    }
}
sail_verified! {
    /// Computes the supported default output policy from the actual accepted bare head.
    pub fn response_policy(head: &[u8]) -> (out: ResponsePolicy)
        requires (true)
        ensures ((out is One) == one_head(head@),
            (out is None) == (!one_head(head@) && silent_head(head@)),
            (out is Unknown) == (!one_head(head@) && !silent_head(head@)))
    {
        if word_eq(head,"check-sat")
            || word_eq(head,"check-sat-assuming")
            || word_eq(head,"check-sat-using")
            || word_eq(head,"echo") || word_prefix(head,"get-") {
            ResponsePolicy::One
        } else if word_eq(head,"push")
            || word_eq(head,"pop")
            || word_eq(head,"assert")
            || word_eq(head,"declare-fun")
            || word_eq(head,"declare-const")
            || word_eq(head,"declare-sort")
            || word_eq(head,"declare-datatype")
            || word_eq(head,"declare-datatypes")
            || word_eq(head,"define-fun")
            || word_eq(head,"define-fun-rec")
            || word_eq(head,"define-funs-rec")
            || word_eq(head,"define-sort")
            || word_eq(head,"define-const")
            || word_eq(head,"set-option")
            || word_eq(head,"set-info")
            || word_eq(head,"set-logic")
            || word_eq(head,"reset")
            || word_eq(head,"reset-assertions")
            || word_eq(head,"exit") {
            ResponsePolicy::None
        } else { ResponsePolicy::Unknown }
    }
}

sail_verified! {
    /// The tactic-check bytes are concretely in the one-response class.
    #[allow(clippy::byte_char_slices, reason = "Explicit bytes form a constructive UTF-8 grammar witness.")]
    pub fn tactic_policy_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let head = [b'c',b'h',b'e',b'c',b'k',b'-',b's',b'a',b't',b'-',b'u',b's',b'i',b'n',b'g'];
        sail_proof! {
            reveal(named);
            reveal_strlit("check-sat-using");
            assert(vstd::string::is_ascii("check-sat-using"));
            vstd::string::is_ascii_spec_bytes("check-sat-using");
            assert(head@ =~= vstd::utf8::encode_utf8("check-sat-using"@));
        }
        matches!(response_policy(&head),ResponsePolicy::One)
    }
}
