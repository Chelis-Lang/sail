//! Actual Cargo-prefix selector scanning over immutable original arguments.
//!
//! Ranges name original argument bytes; they do not certify Cargo parsing,
//! metadata enumeration, physical compiler input or proof authority.
#![allow(
    clippy::redundant_as_str,
    reason = "The pinned stock Verus specifies str::as_bytes, not String::as_bytes."
)]

use sail_acceptance::authority::{engine_pattern_at, exact_bytes};
use sail_acceptance::{sail_code, sail_proof, sail_spec, sail_verified, sail_while};

#[cfg(sail_verus)]
use vstd::prelude::*;
#[cfg(sail_verus)]
use vstd::string::StringSliceAdditionalSpecFns;

sail_code! {
    /// One borrowed value, represented without copying or decoding its bytes.
    pub struct ArgumentSpan {
        /// Index in the original immutable argument vector.
        pub argument: usize,
        /// First byte of the selected suffix in that original argument.
        pub offset: usize,
    }

    /// Selector data constructed before inspecting any verifier result.
    pub struct OriginalSelectors {
        /// First Cargo/Verus separator, or the original argument count.
        pub prefix_end: usize,
        /// Original package selector values, retaining repetitions and order.
        pub packages: Vec<ArgumentSpan>,
        /// Original excluded package values, retaining repetitions and order.
        pub excluded: Vec<ArgumentSpan>,
        /// Original binary selector values, retaining repetitions and order.
        pub bins: Vec<ArgumentSpan>,
        /// Original unconsumed library selector.
        pub library: bool,
        /// Original unconsumed all-binaries selector.
        pub all_bins: bool,
    }
}

sail_spec! {
    pub open spec fn first_selector_separator(arguments: Seq<String>, end: int) -> bool {
        0 <= end <= arguments.len()
            && (forall |i: int| 0 <= i < end ==>
                crate::acceptance::string_bytes(&arguments[i]) != b"--"@)
            && (end < arguments.len() ==>
                crate::acceptance::string_bytes(&arguments[end]) == b"--"@)
    }

    pub open spec fn spans_valid(arguments: Seq<String>, spans: Seq<ArgumentSpan>) -> bool {
        forall |i: int| 0 <= i < spans.len() ==> spans[i].argument < arguments.len()
            && spans[i].offset <= crate::acceptance::string_bytes(&arguments[spans[i].argument as int]).len()
    }

    /// Original scan state after exactly `count` prefix tokens. The final
    /// component records which flag consumes the next value (0, package,
    /// exclusion or binary); a consumed flag spelling is always ordinary data.
    pub open spec fn selector_prefix(arguments: Seq<String>, count: nat)
        -> (Seq<ArgumentSpan>, Seq<ArgumentSpan>, Seq<ArgumentSpan>, bool, bool, u8)
        recommends count <= arguments.len()
        decreases count
    {
        if count == 0 { (Seq::empty(), Seq::empty(), Seq::empty(), false, false, 0u8) }
        else {
            let previous = selector_prefix(arguments, (count - 1) as nat);
            let index = (count - 1) as usize;
            let bytes = crate::acceptance::string_bytes(&arguments[count - 1]);
            let span = ArgumentSpan { argument: index, offset: 0 };
            if previous.5 == 1 { (previous.0.push(span), previous.1, previous.2, previous.3, previous.4, 0) }
            else if previous.5 == 2 { (previous.0, previous.1.push(span), previous.2, previous.3, previous.4, 0) }
            else if previous.5 == 3 { (previous.0, previous.1, previous.2.push(span), previous.3, previous.4, 0) }
            else if bytes == b"-p"@ || bytes == b"--package"@ {
                (previous.0, previous.1, previous.2, previous.3, previous.4, 1)
            } else if bytes == b"--exclude"@ {
                (previous.0, previous.1, previous.2, previous.3, previous.4, 2)
            } else if bytes == b"--bin"@ {
                (previous.0, previous.1, previous.2, previous.3, previous.4, 3)
            } else if bytes == b"--lib"@ {
                (previous.0, previous.1, previous.2, true, previous.4, 0)
            } else if bytes == b"--bins"@ {
                (previous.0, previous.1, previous.2, previous.3, true, 0)
            } else {
                let packages = if sail_acceptance::authority::engine_prefix_at(bytes, b"--package="@, 0) {
                    previous.0.push(ArgumentSpan { argument: index, offset: 10 })
                } else if sail_acceptance::authority::engine_prefix_at(bytes, b"-p"@, 0) {
                    previous.0.push(ArgumentSpan { argument: index,
                        offset: if bytes.len() > 2 && bytes[2] == 61 { 3usize } else { 2usize } })
                } else { previous.0 };
                let excluded = if sail_acceptance::authority::engine_prefix_at(bytes, b"--exclude="@, 0) {
                    previous.1.push(ArgumentSpan { argument: index, offset: 10 })
                } else { previous.1 };
                let bins = if sail_acceptance::authority::engine_prefix_at(bytes, b"--bin="@, 0) {
                    previous.2.push(ArgumentSpan { argument: index, offset: 6 })
                } else { previous.2 };
                (packages, excluded, bins, previous.3, previous.4, 0)
            }
        }
    }

    pub open spec fn selection_matches(arguments: Seq<String>, selection: &OriginalSelectors) -> bool {
        first_selector_separator(arguments, selection.prefix_end as int)
            && selection.packages@ == selector_prefix(arguments, selection.prefix_end as nat).0
            && selection.excluded@ == selector_prefix(arguments, selection.prefix_end as nat).1
            && selection.bins@ == selector_prefix(arguments, selection.prefix_end as nat).2
            && selection.library == selector_prefix(arguments, selection.prefix_end as nat).3
            && selection.all_bins == selector_prefix(arguments, selection.prefix_end as nat).4
            && spans_valid(arguments, selection.packages@)
            && spans_valid(arguments, selection.excluded@)
            && spans_valid(arguments, selection.bins@)
    }

    pub proof fn selector_marker_lengths()
        ensures b"-p"@.len() == 2, b"-p"@ != b"--"@, b"--lib"@ != b"--"@,
            b"--package="@.len() == 10,
            b"--exclude="@.len() == 10, b"--bin="@.len() == 6,
    {
        vstd::array::array_len_matches_n(b"-p");
        vstd::array::array_len_matches_n(b"--");
        vstd::array::array_len_matches_n(b"--lib");
        reveal_byteslit(b"-p"); reveal_byteslit(b"--");
        vstd::array::array_len_matches_n(b"--package=");
        vstd::array::array_len_matches_n(b"--exclude=");
        vstd::array::array_len_matches_n(b"--bin=");
    }
}

sail_verified! {
    /// Finds the actual first separator without treating any later word as Cargo.
    pub fn original_selector_end(arguments: &[String]) -> (end: usize)
        requires(true)
        ensures(first_selector_separator(arguments@, end as int))
    {
        let mut end = 0usize;
        sail_while! { (end < arguments.len())
            invariant(end <= arguments.len(),
                forall |i: int| 0 <= i < end ==>
                    crate::acceptance::string_bytes(&arguments@[i]) != b"--"@)
            decreases(arguments.len() - end)
            {
                if exact_bytes(arguments[end].as_str().as_bytes(), b"--") { return end; }
                end += 1;
            }
        }
        end
    }
}

sail_verified! {
    /// Scans the actual immutable Cargo prefix, keeping the old value-consumption
    /// and compact-option rules. This does not assert Cargo accepted these tokens.
    pub fn scan_original_selectors(arguments: &[String]) -> (selection: OriginalSelectors)
        requires(true)
        ensures(selection_matches(arguments@, &selection))
    {
        let end = original_selector_end(arguments);
        let mut packages: Vec<ArgumentSpan> = Vec::new();
        let mut excluded: Vec<ArgumentSpan> = Vec::new();
        let mut bins: Vec<ArgumentSpan> = Vec::new();
        let mut library = false;
        let mut all_bins = false;
        let mut pending = 0u8;
        let mut index = 0usize;
        sail_proof! { selector_marker_lengths(); }
        sail_while! { (index < end)
            invariant(index <= end, first_selector_separator(arguments@, end as int),
                packages@ == selector_prefix(arguments@, index as nat).0,
                excluded@ == selector_prefix(arguments@, index as nat).1,
                bins@ == selector_prefix(arguments@, index as nat).2,
                library == selector_prefix(arguments@, index as nat).3,
                all_bins == selector_prefix(arguments@, index as nat).4,
                pending == selector_prefix(arguments@, index as nat).5,
                spans_valid(arguments@, packages@), spans_valid(arguments@, excluded@),
                spans_valid(arguments@, bins@),
                b"-p"@.len() == 2, b"--package="@.len() == 10,
                b"--exclude="@.len() == 10, b"--bin="@.len() == 6)
            decreases(end - index)
            {
                let bytes = arguments[index].as_str().as_bytes();
                if pending == 1 {
                    packages.push(ArgumentSpan { argument: index, offset: 0 });
                    pending = 0;
                } else if pending == 2 {
                    excluded.push(ArgumentSpan { argument: index, offset: 0 });
                    pending = 0;
                } else if pending == 3 {
                    bins.push(ArgumentSpan { argument: index, offset: 0 });
                    pending = 0;
                } else if exact_bytes(bytes, b"-p") || exact_bytes(bytes, b"--package") {
                    pending = 1;
                } else if exact_bytes(bytes, b"--exclude") {
                    pending = 2;
                } else if exact_bytes(bytes, b"--bin") {
                    pending = 3;
                } else if exact_bytes(bytes, b"--lib") {
                    library = true;
                } else if exact_bytes(bytes, b"--bins") {
                    all_bins = true;
                } else {
                    if engine_pattern_at(bytes, b"--package=", 0) {
                        packages.push(ArgumentSpan { argument: index, offset: 10 });
                    } else if engine_pattern_at(bytes, b"-p", 0) {
                        let offset = if bytes.len() > 2 && bytes[2] == b'=' { 3 } else { 2 };
                        packages.push(ArgumentSpan { argument: index, offset });
                    }
                    if engine_pattern_at(bytes, b"--exclude=", 0) {
                        excluded.push(ArgumentSpan { argument: index, offset: 10 });
                    }
                    if engine_pattern_at(bytes, b"--bin=", 0) {
                        bins.push(ArgumentSpan { argument: index, offset: 6 });
                    }
                }
                index += 1;
            }
        }
        OriginalSelectors { prefix_end: end, packages, excluded, bins, library, all_bins }
    }
}

sail_spec! {
    pub open spec fn original_span_matches(arguments: Seq<String>, span: &ArgumentSpan, candidate: Seq<u8>) -> bool {
        span.argument < arguments.len()
            && span.offset <= crate::acceptance::string_bytes(&arguments[span.argument as int]).len()
            && crate::acceptance::string_bytes(&arguments[span.argument as int]).skip(span.offset as int) == candidate
    }

    pub open spec fn selected_package_bytes(bytes: Seq<u8>, begin: int, name: Seq<u8>, id: Seq<u8>, version: Option<Seq<u8>>) -> bool {
        0 <= begin <= bytes.len() && (bytes.skip(begin) == name || bytes.skip(begin) == id
            || exists |at: int| begin <= at < bytes.len() && bytes[at] == 64
                && (forall |i: int| begin <= i < at ==> bytes[i] != 64)
                && bytes.subrange(begin, at) == name && version.is_some()
                && bytes.skip(at + 1) == version.unwrap())
    }
}

sail_verified! {
    /// Compares a complete raw byte window without normalization or allocation.
    pub fn original_window_matches(bytes: &[u8], begin: usize, end: usize, candidate: &[u8]) -> (matched: bool)
        requires(true)
        ensures(matched == (begin <= end <= bytes.len()
            && bytes@.subrange(begin as int, end as int) == candidate@))
    {
        if begin > end || end > bytes.len() || end - begin != candidate.len() { return false; }
        engine_pattern_at(bytes, candidate, begin)
    }
}

sail_verified! {
    /// Compares the actual selected suffix with one metadata string's bytes.
    pub fn original_span_matches_str(arguments: &[String], span: &ArgumentSpan, candidate: &str) -> (matched: bool)
        requires(true)
        ensures(matched == original_span_matches(arguments@, span, vstd::utf8::encode_utf8(candidate@)))
    {
        if span.argument >= arguments.len() { return false; }
        let bytes = arguments[span.argument].as_str().as_bytes();
        original_window_matches(bytes, span.offset, bytes.len(), candidate.as_bytes())
    }
}

sail_verified! {
    /// Matches the old name/id/first-at-version package rule on original bytes.
    #[allow(clippy::manual_let_else, clippy::question_mark,
        reason = "Retain explicit missing metadata/version outcomes in the shared source and refusal controls.")]
    pub fn original_package_matches(arguments: &[String], span: &ArgumentSpan, name: &str, id: &str, version: Option<&str>)
        -> (matched: bool)
        requires(true)
        ensures(matched == (span.argument < arguments.len() && selected_package_bytes(
            crate::acceptance::string_bytes(&arguments@[span.argument as int]), span.offset as int,
            vstd::utf8::encode_utf8(name@), vstd::utf8::encode_utf8(id@),
            match version { Some(value) => Some(vstd::utf8::encode_utf8(value@)), None => None })))
    {
        if original_span_matches_str(arguments, span, name) || original_span_matches_str(arguments, span, id) { return true; }
        let version = match version { Some(value) => value, None => return false };
        if span.argument >= arguments.len() { return false; }
        let bytes = arguments[span.argument].as_str().as_bytes();
        if span.offset > bytes.len() { return false; }
        let mut at = span.offset;
        sail_while! { (at < bytes.len())
            invariant(span.offset <= at <= bytes.len(), span.argument < arguments.len(),
                forall |i: int| span.offset <= i < at ==> bytes@[i] != 64)
            decreases(bytes.len() - at)
            {
                if bytes[at] == b'@' {
                    return original_window_matches(bytes, span.offset, at, name.as_bytes())
                        && original_window_matches(bytes, at + 1, bytes.len(), version.as_bytes());
                }
                at += 1;
            }
        }
        false
    }
}
