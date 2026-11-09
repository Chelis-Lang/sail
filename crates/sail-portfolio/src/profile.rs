//! Spike prototype: a streaming quantifier-instantiation counter over a Z3
//! trace (`trace=true trace_file_name=…` on Z3's command line).
//!
//! Only three kinds of trace line matter:
//!
//! ```text
//! [mk-quant] #27 q_fg 1 #26 #25          term id, qid, bound variables, …
//! [new-match] 2175005514 #27 #26 #41 ; … fingerprint, quantifier term id, …
//! [instance] 2175005514 ; 1              fingerprint, … (0 or 0x0: theory)
//! ```
//!
//! A quantifier's term id is redefined whenever Z3 rewrites it, and ids are
//! reused, so the id-to-qid table follows the stream. An `[instance]` names
//! only a fingerprint; the `[new-match]` that introduced the fingerprint
//! says which quantifier it belongs to. Z3 repeats the `[new-match]` before
//! it instantiates the same match again after a backtrack, so a fingerprint
//! is forgotten once instantiated, which bounds the table by the matches
//! still pending.
//!
//! Nothing here is a verdict: a traced run is a measurement (the trace can
//! change the search), and the counts describe that run only.

use std::collections::HashMap;

/// Counts for one `:qid`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuantCount {
    /// The quantifier's `:qid`.
    pub qid: String,
    /// `[mk-quant]` lines naming it (definitions and rewrites).
    pub definitions: u64,
    /// `[new-match]` lines for it.
    pub matches: u64,
    /// `[instance]` lines whose fingerprint belongs to it.
    pub instances: u64,
}

/// What a whole trace contained.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    /// Per-qid counts, most instances first (ties: more matches, then qid).
    pub quantifiers: Vec<QuantCount>,
    /// Lines read.
    pub lines: u64,
    /// `[begin-check]` lines.
    pub checks: u64,
    /// `[instance]` lines with fingerprint zero (theory instances).
    pub theory_instances: u64,
    /// `[instance]` lines whose fingerprint no pending `[new-match]` named.
    pub unattributed_instances: u64,
    /// `[new-match]` lines whose quantifier id no `[mk-quant]` defined.
    pub unattributed_matches: u64,
    /// The largest number of pending fingerprints held at once.
    pub peak_pending: usize,
    /// Whether the trace ended with `[eof]`.
    pub complete: bool,
}

impl Profile {
    /// Quantifier instances attributed to a qid.
    #[must_use]
    pub fn instances(&self) -> u64 {
        self.quantifiers.iter().map(|q| q.instances).sum()
    }
}

/// The streaming counter. Feed it trace bytes in any chunking.
#[derive(Debug, Default)]
pub struct Counter {
    partial: Vec<u8>,
    /// Term id (`#27`, `datatype#6`) to index in `counts`.
    ids: HashMap<Vec<u8>, u32>,
    by_qid: HashMap<Vec<u8>, u32>,
    counts: Vec<QuantCount>,
    /// Fingerprint to index in `counts`, until instantiated.
    pending: HashMap<u64, u32>,
    /// Count only between the n-th `[begin-check]` (from 1) and the next.
    only_check: Option<u64>,
    out: Profile,
}

fn fingerprint(token: &[u8]) -> Option<u64> {
    let text = std::str::from_utf8(token).ok()?;
    match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    }
}

impl Counter {
    /// A counter with nothing read.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A counter for the `check`-th check of the trace only (from 1): a
    /// session replayed whole, so that the solver reaches the check with
    /// its real history, but profiled for that one check. Definitions and
    /// pending matches are still followed throughout.
    #[must_use]
    pub fn for_check(check: u64) -> Self {
        Self {
            only_check: Some(check),
            ..Self::default()
        }
    }

    fn counting(&self) -> bool {
        self.only_check.is_none_or(|n| n == self.out.checks)
    }

    /// Feeds the next bytes of the trace.
    pub fn push(&mut self, mut bytes: &[u8]) {
        while let Some(at) = bytes.iter().position(|&b| b == b'\n') {
            let (head, rest) = bytes.split_at(at);
            if self.partial.is_empty() {
                self.line(head);
            } else {
                self.partial.extend_from_slice(head);
                let line = std::mem::take(&mut self.partial);
                self.line(&line);
            }
            bytes = rest.get(1..).unwrap_or_default();
        }
        self.partial.extend_from_slice(bytes);
    }

    fn line(&mut self, line: &[u8]) {
        self.out.lines += 1;
        // Every line of interest starts with `[`; most lines are `[mk-app]`
        // and `[attach-enode]`, rejected on the third byte.
        let mut words = line
            .split(u8::is_ascii_whitespace)
            .filter(|w| !w.is_empty());
        match words.next() {
            Some(b"[mk-quant]" | b"[mk-lambda]") => {
                let (Some(id), Some(qid)) = (words.next(), words.next()) else {
                    return;
                };
                let next = u32::try_from(self.counts.len()).unwrap_or(u32::MAX);
                let index = *self.by_qid.entry(qid.to_vec()).or_insert(next);
                if index == next {
                    self.counts.push(QuantCount {
                        qid: String::from_utf8_lossy(qid).into_owned(),
                        ..QuantCount::default()
                    });
                }
                if let Some(c) = self.counts.get_mut(index as usize) {
                    c.definitions += 1;
                }
                self.ids.insert(id.to_vec(), index);
            }
            Some(b"[new-match]") => {
                let (Some(fp), Some(id)) = (words.next().and_then(fingerprint), words.next())
                else {
                    return;
                };
                let counting = self.counting();
                let Some(&index) = self.ids.get(id) else {
                    self.out.unattributed_matches += u64::from(counting);
                    return;
                };
                if counting && let Some(c) = self.counts.get_mut(index as usize) {
                    c.matches += 1;
                }
                self.pending.insert(fp, index);
                self.out.peak_pending = self.out.peak_pending.max(self.pending.len());
            }
            Some(b"[instance]") => {
                let counting = u64::from(self.counting());
                match words.next().and_then(fingerprint) {
                    Some(0) => self.out.theory_instances += counting,
                    Some(fp) => match self.pending.remove(&fp) {
                        Some(index) => {
                            if let Some(c) = self.counts.get_mut(index as usize) {
                                c.instances += counting;
                            }
                        }
                        None => self.out.unattributed_instances += counting,
                    },
                    None => self.out.unattributed_instances += counting,
                }
            }
            Some(b"[begin-check]") => self.out.checks += 1,
            Some(b"[eof]") => self.out.complete = true,
            _ => {}
        }
    }

    /// Ends the trace and returns the counts.
    #[must_use]
    pub fn finish(mut self) -> Profile {
        if !self.partial.is_empty() {
            let line = std::mem::take(&mut self.partial);
            self.line(&line);
        }
        let mut out = self.out;
        out.quantifiers = self.counts;
        out.quantifiers.sort_by(|a, b| {
            (b.instances, b.matches, &a.qid).cmp(&(a.instances, a.matches, &b.qid))
        });
        out
    }
}

/// Counts a whole trace held in memory.
#[must_use]
pub fn count(trace: &[u8]) -> Profile {
    let mut c = Counter::new();
    c.push(trace);
    c.finish()
}

/// Where a Verus-emitted qid comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// `user` (a `forall`/`exists`/`choose` written in source), `definition`
    /// (a function's definition or fuel axiom), `contract` (a function's
    /// `requires`/`ensures` axiom), `datatype`/`trait` (encoding axioms of
    /// a named item), `prelude` (Verus's fixed prelude) or `other`.
    pub kind: &'static str,
    /// The Verus item path (`crate::module::item`), when the qid names one.
    pub item: Option<String>,
    /// For `user`: the quantifier's ordinal within the item.
    pub ordinal: Option<u64>,
}

/// Turns an AIR item name (`crate!module.item.`, with `.?` and `%` marks)
/// into a Rust-like path.
fn air_path(name: &str) -> String {
    let name = name.trim_end_matches(['?', '.']);
    let (krate, rest) = name.split_once('!').unwrap_or(("", name));
    let rest = rest.replace('.', "::").replace("impl&__", "impl&%");
    if krate.is_empty() {
        rest
    } else {
        format!("{krate}::{rest}")
    }
}

/// Maps a qid as the pinned Verus writes it to its origin. Exact for the
/// shapes below; anything else is `other` with no item, never a guess.
///
/// - `user_<crate>__<module>__<item>_<n>`: a source quantifier. Path
///   segments are joined with `__`, which a segment may itself contain, so
///   `item` keeps the mangled text.
/// - `internal_<air name>_definition` and `…_fuel_…`: a function's body.
/// - `internal_req__…`/`internal_ens__…`: a function's contract.
/// - `prelude_…`: the fixed prelude.
#[must_use]
pub fn origin(qid: &str) -> Origin {
    let other = |kind| Origin {
        kind,
        item: None,
        ordinal: None,
    };
    if let Some(rest) = qid.strip_prefix("user_") {
        let (item, ordinal) = match rest.rsplit_once('_') {
            Some((item, n)) if n.bytes().all(|b| b.is_ascii_digit()) && !n.is_empty() => {
                (item, n.parse().ok())
            }
            _ => (rest, None),
        };
        return Origin {
            kind: "user",
            item: Some(item.to_owned()),
            ordinal,
        };
    }
    if qid.starts_with("prelude_") {
        return other("prelude");
    }
    let Some(rest) = qid.strip_prefix("internal_") else {
        return other("other");
    };
    // Verus writes `%` as `__` inside a qid (`req%f` is `req__f`).
    for (prefix, kind) in [
        ("req__", "contract"),
        ("ens__", "contract"),
        ("req%", "contract"),
        ("ens%", "contract"),
    ] {
        if let Some(name) = rest.strip_prefix(prefix) {
            let name = name.strip_suffix("_definition").unwrap_or(name);
            return Origin {
                kind,
                item: Some(air_path(name)),
                ordinal: None,
            };
        }
    }
    for (suffix, kind) in [
        ("_definition", "definition"),
        ("_fuel_to_zero_definition", "definition"),
        ("_fuel_to_body_definition", "definition"),
        ("_pre_post_definition", "contract"),
        ("_has_type_always_definition", "datatype"),
        ("_invariant_definition", "datatype"),
        ("_box_axiom_definition", "datatype"),
        ("_unbox_axiom_definition", "datatype"),
        ("_trait_type_bounds_definition", "trait"),
        ("_trait_impl_definition", "trait"),
        ("_constructor_definition", "datatype"),
        ("_accessor_definition", "datatype"),
        ("_ext_equal_definition", "datatype"),
    ]
    .into_iter()
    .rev()
    {
        // A function's AIR name always carries its crate (`crate!path.`);
        // datatype and trait axioms also come with `__`-joined paths.
        if let Some(name) = rest.strip_suffix(suffix)
            && (name.contains('!') || matches!(kind, "datatype" | "trait"))
        {
            let name = name.rsplit_once("./").map_or(name, |(head, _)| head);
            let item = if name.contains('!') {
                air_path(name)
            } else {
                name.replace("__", "::")
            };
            return Origin {
                kind,
                item: Some(item),
                ordinal: None,
            };
        }
    }
    other("other")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trace Z3 4.16.0 wrote for `tests/fixtures/profile/two-checks.smt2`.
    const TRACE: &[u8] = include_bytes!("../tests/fixtures/profile/two-checks.trace");

    /// Counted by hand from the fixture (see its README): each quantifier
    /// is matched once in the first check and three times in the second
    /// (one match is found again after a backtrack), and every match is
    /// instantiated. Z3's own `:quant-instantiations` for the session is 8.
    #[test]
    fn counts_match_the_hand_count() {
        let p = count(TRACE);
        let row = |qid: &str| p.quantifiers.iter().find(|q| q.qid == qid).cloned();
        assert_eq!(
            row("q_fg"),
            Some(QuantCount {
                qid: "q_fg".to_owned(),
                definitions: 1,
                matches: 4,
                instances: 4
            })
        );
        assert_eq!(
            row("q_g"),
            Some(QuantCount {
                qid: "q_g".to_owned(),
                definitions: 2,
                matches: 4,
                instances: 4
            })
        );
        assert_eq!(
            row("constructor_accessor_axiom").map(|q| (q.definitions, q.matches, q.instances)),
            Some((2, 0, 0))
        );
        assert_eq!(p.instances(), 8);
        assert_eq!(p.theory_instances, 16);
        assert_eq!(p.unattributed_instances, 0);
        assert_eq!(p.unattributed_matches, 0);
        assert_eq!(p.checks, 2);
        assert_eq!(p.lines, 335);
        assert!(p.complete);
        assert_eq!(p.quantifiers.first().map(|q| q.qid.as_str()), Some("q_fg"));
    }

    /// The hand count per check: one match and instance of each quantifier
    /// in the first check, three in the second.
    #[test]
    fn one_check_of_a_whole_session() {
        for (check, each) in [(1, 1), (2, 3)] {
            let mut c = Counter::for_check(check);
            c.push(TRACE);
            let p = c.finish();
            let row = |qid: &str| {
                p.quantifiers
                    .iter()
                    .find(|q| q.qid == qid)
                    .map(|q| (q.matches, q.instances))
            };
            assert_eq!(row("q_fg"), Some((each, each)), "check {check}");
            assert_eq!(row("q_g"), Some((each, each)), "check {check}");
            assert_eq!(p.instances(), 2 * each);
        }
    }

    #[test]
    fn chunking_does_not_change_the_counts() {
        let whole = count(TRACE);
        for size in [1, 7, 4096] {
            let mut c = Counter::new();
            for chunk in TRACE.chunks(size) {
                c.push(chunk);
            }
            assert_eq!(c.finish(), whole, "chunk size {size}");
        }
    }

    #[test]
    fn a_truncated_trace_is_marked_incomplete() {
        let cut = TRACE.len() / 2;
        let p = count(TRACE.get(..cut).unwrap_or_default());
        assert!(!p.complete);
        assert!(p.instances() < 8);
    }

    #[test]
    fn origins_of_verus_qids() {
        let o = origin("user_vstd__seq__axiom_seq_index_decreases_12");
        assert_eq!(
            (o.kind, o.item.as_deref(), o.ordinal),
            (
                "user",
                Some("vstd__seq__axiom_seq_index_decreases"),
                Some(12)
            )
        );
        assert_eq!(origin("prelude_unbox_int").kind, "prelude");
        let o = origin("internal_ironsht!host_impl_v.impl&__0.valid.?_definition");
        assert_eq!(
            (o.kind, o.item.as_deref()),
            ("definition", Some("ironsht::host_impl_v::impl&%0::valid"))
        );
        let o = origin("internal_req__lib!m.f._definition");
        assert_eq!((o.kind, o.item.as_deref()), ("contract", Some("lib::m::f")));
        let o = origin("internal_core!option.Option./Some/0_invariant_definition");
        assert_eq!(
            (o.kind, o.item.as_deref()),
            ("datatype", Some("core::option::Option"))
        );
        let o = origin("internal_core__option__Option_box_axiom_definition");
        assert_eq!(
            (o.kind, o.item.as_deref()),
            ("datatype", Some("core::option::Option"))
        );
        assert_eq!(origin("k!12").kind, "other");
    }
}
