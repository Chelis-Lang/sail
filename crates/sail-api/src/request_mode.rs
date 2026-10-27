//! Actual construction of a report command from per-package verifier options.
//!
//! The option values are observed inputs, not proof of compiler/launch origin.
//! This operation changes the reported command exactly as the existing adapter
//! does; it never changes the physical compiler arguments or environment.
#![allow(
    clippy::redundant_as_str,
    reason = "The pinned stock Verus specifies str::as_bytes, not String::as_bytes."
)]

use crate::acceptance::{verification_enabled_command, verification_enabled_options};
use sail_acceptance::authority::exact_bytes;
use sail_acceptance::{sail_proof, sail_spec, sail_verified, sail_while};

#[cfg(sail_verus)]
use vstd::prelude::*;
#[cfg(sail_verus)]
use vstd::string::StringSliceAdditionalSpecFns;

sail_spec! {
    pub open spec fn has_report_separator(arguments: Seq<String>) -> bool {
        exists |i: int| 0 <= i < arguments.len()
            && crate::acceptance::string_bytes(&arguments[i]) == b"--"@
    }

    pub open spec fn kept_report_arguments(original: Seq<String>, enabled: bool, end: nat) -> Seq<String>
        recommends end <= original.len()
        decreases end
    {
        if end == 0 { Seq::empty() }
        else if enabled && has_report_separator(original.take(end as int))
            && crate::acceptance::string_bytes(&original[end - 1]) == b"--no-verify"@
        { kept_report_arguments(original, enabled, (end - 1) as nat) }
        else { kept_report_arguments(original, enabled, (end - 1) as nat).push(original[end - 1]) }
    }

    pub open spec fn report_mode_projection(original: Seq<String>, enabled: bool, projected: Seq<String>) -> bool {
        let kept = kept_report_arguments(original, enabled, original.len());
        if enabled || crate::acceptance::verification_disabled(kept) { projected == kept }
        else if has_report_separator(original) {
            projected.len() == kept.len() + 1 && projected.take(kept.len() as int) == kept
                && crate::acceptance::string_bytes(&projected.last()) == b"--no-verify"@
        } else {
            projected.len() == kept.len() + 2 && projected.take(kept.len() as int) == kept
                && crate::acceptance::string_bytes(&projected[kept.len() as int]) == b"--"@
                && crate::acceptance::string_bytes(&projected.last()) == b"--no-verify"@
        }
    }

    /// Interprets only the actual literal definitions used by this constructor.
    pub proof fn report_marker_literals()
        ensures b"--"@ != b"--no-verify"@,
            vstd::utf8::encode_utf8("--"@) == b"--"@,
            vstd::utf8::encode_utf8("--no-verify"@) == b"--no-verify"@,
    {
        vstd::array::array_len_matches_n(b"--");
        vstd::array::array_len_matches_n(b"--no-verify");
        reveal_strlit("--"); reveal_byteslit(b"--");
        reveal_strlit("--no-verify"); reveal_byteslit(b"--no-verify");
        assert(vstd::string::is_ascii("--"));
        assert(vstd::string::is_ascii("--no-verify"));
        vstd::string::is_ascii_spec_bytes("--");
        vstd::string::is_ascii_spec_bytes("--no-verify");
        assert("--".spec_bytes() =~= b"--"@);
        assert("--no-verify".spec_bytes() =~= b"--no-verify"@);
    }

    /// Frames an actual push without assuming a mode or dropping an argument.
    pub proof fn report_prefix_push(prefix: Seq<String>, argument: String)
        ensures has_report_separator(prefix.push(argument)) ==
                (has_report_separator(prefix) || crate::acceptance::string_bytes(&argument) == b"--"@),
            crate::acceptance::verification_disabled(prefix.push(argument)) ==
                (crate::acceptance::verification_disabled(prefix)
                    || (has_report_separator(prefix) && crate::acceptance::string_bytes(&argument) == b"--no-verify"@)),
    {
        if has_report_separator(prefix.push(argument)) {
            let i = choose |i: int| 0 <= i < prefix.len() + 1
                && crate::acceptance::string_bytes(&prefix.push(argument)[i]) == b"--"@;
            if i < prefix.len() { assert(has_report_separator(prefix)); }
        }
        if has_report_separator(prefix) {
            let i = choose |i: int| 0 <= i < prefix.len()
                && crate::acceptance::string_bytes(&prefix[i]) == b"--"@;
            assert(crate::acceptance::string_bytes(&prefix.push(argument)[i]) == b"--"@);
        }
        if crate::acceptance::verification_disabled(prefix.push(argument)) {
            let (i, j) = choose |i: int, j: int| 0 <= i < j < prefix.len() + 1
                && crate::acceptance::string_bytes(&prefix.push(argument)[i]) == b"--"@
                && crate::acceptance::string_bytes(&prefix.push(argument)[j]) == b"--no-verify"@;
            if j < prefix.len() { assert(crate::acceptance::verification_disabled(prefix)); }
            else { assert(has_report_separator(prefix)); }
        }
        if crate::acceptance::verification_disabled(prefix) {
            let (i, j) = choose |i: int, j: int| 0 <= i < j < prefix.len()
                && crate::acceptance::string_bytes(&prefix[i]) == b"--"@
                && crate::acceptance::string_bytes(&prefix[j]) == b"--no-verify"@;
            assert(crate::acceptance::verification_disabled(prefix.push(argument)));
        }
        if has_report_separator(prefix) && crate::acceptance::string_bytes(&argument) == b"--no-verify"@ {
            let i = choose |i: int| 0 <= i < prefix.len()
                && crate::acceptance::string_bytes(&prefix[i]) == b"--"@;
            assert(crate::acceptance::verification_disabled(prefix.push(argument)));
        }
    }
}

sail_verified! {
    /// Retains exact report tokens/order while recording the observed package's
    /// observed option mode. Absence remains unknown and cannot become enabled.
    #[allow(clippy::manual_let_else, clippy::question_mark,
        reason = "Keep missing-option refusal explicit in checked source and ordinary mutations.")]
    pub fn construct_report_verification_arguments(report_args: &[String], effective: Option<&[String]>)
        -> (out: Option<Vec<String>>)
        requires(true)
        ensures(out.is_some() == effective.is_some(),
            match effective { None => out.is_none(), Some(options) => out.is_some() && {
                let enabled = !crate::acceptance::options_disable_verification(options@);
                report_mode_projection(report_args@, enabled, out.unwrap()@)
                    && !crate::acceptance::verification_disabled(out.unwrap()@) == enabled
            } })
    {
        let effective = match effective { Some(value) => value, None => return None };
        let enabled = verification_enabled_options(effective);
        sail_proof! { report_marker_literals(); }
        let mut forwarded = false;
        let mut arguments: Vec<String> = Vec::with_capacity(report_args.len());
        let mut i = 0usize;
        sail_while! { (i < report_args.len())
            invariant(i <= report_args.len(),
                arguments@ == kept_report_arguments(report_args@, enabled, i as nat),
                forwarded == has_report_separator(report_args@.take(i as int)),
                forwarded == has_report_separator(arguments@),
                enabled ==> !crate::acceptance::verification_disabled(arguments@))
            decreases(report_args.len() - i)
            {
                let argument = &report_args[i];
                let bytes = argument.as_str().as_bytes();
                sail_proof! {
                    report_prefix_push(report_args@.take(i as int), *argument);
                    assert(report_args@.take(i as int + 1)
                        =~= report_args@.take(i as int).push(*argument));
                }
                if exact_bytes(bytes, b"--") { forwarded = true; }
                if !(enabled && forwarded && exact_bytes(bytes, b"--no-verify")) {
                    sail_proof! { report_prefix_push(arguments@, *argument); }
                    arguments.push(argument.clone());
                }
                i += 1;
            }
        }
        sail_proof! { assert(report_args@.take(i as int) =~= report_args@); }
        if !enabled && verification_enabled_command(&arguments) {
            if !forwarded {
                let separator = "--".to_owned();
                sail_proof! { report_prefix_push(arguments@, separator); }
                arguments.push(separator);
            }
            let no_verify = "--no-verify".to_owned();
            sail_proof! { report_prefix_push(arguments@, no_verify); }
            arguments.push(no_verify);
        }
        Some(arguments)
    }
}
