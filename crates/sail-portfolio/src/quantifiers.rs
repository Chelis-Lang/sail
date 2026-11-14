//! Bounded counters for the pinned Z3 4.16.0 diagnostic trace.
//!
//! Counts describe the traced execution only. This decoder authenticates no
//! solver, input, function, verdict or currentness. Its caller must bind those
//! separately and compare the actual traced and plain executions.

use std::collections::HashMap;
use std::fmt;

use sail_smtlib::lex::{Lexer, TokenKind};
use serde::Serialize;

/// Maximum complete trace input, including framing and ignored AST records.
pub const MAX_TRACE_BYTES: usize = 64 * 1024 * 1024;
const MAX_LINE: usize = 64 * 1024;
const MAX_TERMS: usize = 100_000;
const MAX_QIDS: usize = 10_000;
const MAX_LABEL_BYTES: usize = 4 * 1024 * 1024;
const MAX_FINGERPRINTS: usize = 100_000;
const MAX_DEPTH: usize = 1024;

/// An unavailable trace measurement, never a proof outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    line: u64,
    reason: &'static str,
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "error[SAIL5103]: diagnostic trace line {}: {}",
            self.line, self.reason
        )
    }
}
impl std::error::Error for Error {}

/// Counts grouped by the exact printed qid, including its quotation if present.
/// Rewrites and separate quantifiers sharing that qid remain one aggregate.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct Quantifier {
    /// Exact printed qid; not an authenticated source/function identity.
    pub qid: String,
    /// Quantifier definitions, including rewrites.
    pub definitions: u64,
    /// E-matching discoveries naming this qid.
    pub matches: u64,
    /// Model-based discoveries naming this qid.
    pub mbqi_discoveries: u64,
    /// Actual instance events with unambiguous fingerprint attribution.
    pub instances: u64,
}

/// One complete trace, with explicit unavailable instance attribution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Counts {
    /// Physical lines including the tool header and EOF record.
    pub lines: u64,
    /// Actual begin-check events; counts are cumulative across these checks.
    pub checks: u64,
    /// qid aggregates sorted by instances, matches, then exact qid.
    pub quantifiers: Vec<Quantifier>,
    /// Attributed quantifier instances.
    pub quantifier_instances: u64,
    /// Instance events explicitly associated with theory discovery.
    pub theory_instances: u64,
    /// Instances with missing or conflicting fingerprint ownership.
    pub unattributed_instances: u64,
    /// Discoveries whose quantifier term had no definition.
    pub unattributed_discoveries: u64,
    /// Distinct fingerprints observed with contradictory ownership.
    pub ambiguous_fingerprints: u64,
    /// Whether every instance/discovery had unambiguous attribution.
    pub attribution_complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Fingerprint {
    Hash(u32),
    Null,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Owner {
    Quantifier(usize),
    Theory,
    Unknown,
}

/// Streaming decoder with sticky refusals and fixed byte/metadata bounds.
#[derive(Default)]
pub struct Counter {
    state: State,
    partial: Vec<u8>,
    error: Option<Error>,
}

#[derive(Default)]
struct State {
    bytes: usize,
    lines: u64,
    checks: u64,
    header: bool,
    eof: bool,
    depth: usize,
    label_bytes: usize,
    terms: HashMap<String, usize>,
    labels: HashMap<String, usize>,
    fingerprints: HashMap<Fingerprint, Owner>,
    quantifiers: Vec<Quantifier>,
    instances: u64,
    theory: u64,
    unattributed: u64,
    discoveries: u64,
    ambiguous: u64,
}

impl Counter {
    /// Starts an empty decoder; the first complete line must name pinned Z3.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds original bytes, in any chunking. Refusals remain sticky.
    ///
    /// # Errors
    /// `SAIL5103` for unsupported, malformed, trailing or over-budget input.
    pub fn push(&mut self, mut bytes: &[u8]) -> Result<(), Error> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let result = (|| {
            self.state.bytes = self
                .state
                .bytes
                .checked_add(bytes.len())
                .filter(|size| *size <= MAX_TRACE_BYTES)
                .ok_or_else(|| self.state.fail("trace byte budget exceeded"))?;
            while !bytes.is_empty() {
                let end = bytes.iter().position(|byte| *byte == b'\n');
                let size = end.unwrap_or(bytes.len());
                if self.partial.len().saturating_add(size) > MAX_LINE {
                    return Err(self.state.fail("trace line budget exceeded"));
                }
                if let Some(end) = end {
                    let head = &bytes[..end];
                    if self.partial.is_empty() {
                        self.state.line(head)?;
                    } else {
                        self.partial.extend_from_slice(head);
                        let mut line = std::mem::take(&mut self.partial);
                        let result = self.state.line(&line);
                        line.clear();
                        self.partial = line;
                        result?;
                    }
                    bytes = &bytes[end + 1..];
                } else {
                    self.partial.extend_from_slice(bytes);
                    break;
                }
            }
            Ok(())
        })();
        if let Err(error) = &result {
            self.error = Some(error.clone());
        }
        result
    }

    /// Requires a closed EOF record; truncated input never yields counts.
    ///
    /// # Errors
    /// `SAIL5103` for any earlier refusal or incomplete trace.
    pub fn finish(mut self) -> Result<Counts, Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if !self.partial.is_empty() || !self.state.header || !self.state.eof {
            return Err(self.state.fail("trace is incomplete"));
        }
        self.state.quantifiers.sort_by(|a, b| {
            b.instances
                .cmp(&a.instances)
                .then_with(|| b.matches.cmp(&a.matches))
                .then_with(|| a.qid.cmp(&b.qid))
        });
        Ok(Counts {
            lines: self.state.lines,
            checks: self.state.checks,
            quantifiers: self.state.quantifiers,
            quantifier_instances: self.state.instances,
            theory_instances: self.state.theory,
            unattributed_instances: self.state.unattributed,
            unattributed_discoveries: self.state.discoveries,
            ambiguous_fingerprints: self.state.ambiguous,
            attribution_complete: self.state.unattributed == 0
                && self.state.discoveries == 0
                && self.state.ambiguous == 0,
        })
    }
}

impl State {
    fn fail(&self, reason: &'static str) -> Error {
        Error {
            line: self.lines.saturating_add(1),
            reason,
        }
    }
    fn line(&mut self, raw: &[u8]) -> Result<(), Error> {
        if self.eof {
            return Err(self.fail("records after EOF"));
        }
        let text = std::str::from_utf8(raw).map_err(|_| self.fail("trace is not UTF-8"))?;
        if text
            .chars()
            .any(|character| character.is_control() && character != '\t')
        {
            return Err(self.fail("unexpected control byte"));
        }
        if !self.header {
            if text != "[tool-version] Z3 4.16.0" {
                return Err(self.fail("unsupported tool header"));
            }
            self.header = true;
            self.lines += 1;
            return Ok(());
        }
        let (tag, body) = text
            .split_once(']')
            .ok_or_else(|| self.fail("missing record tag"))?;
        if !body.is_empty() && !body.starts_with(' ') {
            return Err(self.fail("missing record separator"));
        }
        let body = body.strip_prefix(' ').unwrap_or(body);
        match tag {
            "[mk-quant" | "[mk-lambda" => self.quantifier(body)?,
            "[mk-app" | "[mk-var" | "[mk-proof" => {
                let mut words = body.split_ascii_whitespace();
                let id = words
                    .next()
                    .filter(|id| term(id))
                    .ok_or_else(|| self.fail("invalid term definition"))?;
                if words.next().is_none() {
                    return Err(self.fail("incomplete term definition"));
                }
                self.terms.remove(id);
            }
            "[new-match" => self.discovery(body, false)?,
            "[inst-discovered" => self.discovered(body)?,
            "[instance" => self.instance(body)?,
            "[end-of-instance" => {
                if !body.is_empty() || self.depth == 0 {
                    return Err(self.fail("unbalanced instance end"));
                }
                self.depth -= 1;
            }
            "[begin-check" => {
                if number(body).is_none() {
                    return Err(self.fail("invalid begin-check"));
                }
                self.checks += 1;
            }
            "[eof" => {
                if !body.is_empty() || self.depth != 0 {
                    return Err(self.fail("incomplete instance at EOF"));
                }
                self.eof = true;
            }
            "[assign" | "[attach-enode" | "[attach-meaning" | "[attach-var-names" | "[conflict"
            | "[eq-expl" | "[push" | "[pop" | "[resolve-lit" | "[resolve-process"
            | "[decide-and-or" => {
                if body.is_empty() {
                    return Err(self.fail("incomplete opaque record"));
                }
            }
            _ => return Err(self.fail("unsupported trace record")),
        }
        self.lines += 1;
        Ok(())
    }

    fn quantifier(&mut self, body: &str) -> Result<(), Error> {
        let fields = atoms(body).ok_or_else(|| self.fail("invalid quantifier fields"))?;
        if fields.len() < 4
            || !term(fields[0])
            || !qid(fields[1])
            || number(fields[2]).is_none_or(|n| n == 0)
            || !fields[3..].iter().all(|id| term(id))
        {
            return Err(self.fail("invalid quantifier definition"));
        }
        let qid = fields[1];
        let index = if let Some(index) = self.labels.get(qid) {
            *index
        } else {
            if self.labels.len() >= MAX_QIDS
                || self.label_bytes.saturating_add(qid.len()) > MAX_LABEL_BYTES
            {
                return Err(self.fail("quantifier label budget exceeded"));
            }
            let index = self.quantifiers.len();
            self.label_bytes += qid.len();
            self.labels.insert(qid.to_owned(), index);
            self.quantifiers.push(Quantifier {
                qid: qid.to_owned(),
                ..Quantifier::default()
            });
            index
        };
        if !self.terms.contains_key(fields[0]) && self.terms.len() >= MAX_TERMS {
            return Err(self.fail("quantifier term budget exceeded"));
        }
        self.quantifiers[index].definitions += 1;
        self.terms.insert(fields[0].to_owned(), index);
        Ok(())
    }

    fn register(&mut self, fp: Fingerprint, owner: Owner) -> Result<(), Error> {
        if let Some(previous) = self.fingerprints.get_mut(&fp) {
            if *previous != owner && *previous != Owner::Unknown {
                *previous = Owner::Unknown;
                self.ambiguous += 1;
            }
        } else {
            if self.fingerprints.len() >= MAX_FINGERPRINTS {
                return Err(self.fail("fingerprint budget exceeded"));
            }
            self.fingerprints.insert(fp, owner);
        }
        Ok(())
    }
    fn discovery(&mut self, body: &str, mbqi: bool) -> Result<(), Error> {
        let left = if mbqi {
            body
        } else {
            let (left, right) = body
                .split_once(';')
                .ok_or_else(|| self.fail("match lacks explanation boundary"))?;
            if !explanation(right) {
                return Err(self.fail("invalid match explanation"));
            }
            left
        };
        let words: Vec<_> = left.split_ascii_whitespace().collect();
        if words.len() < 3 || !words[1..].iter().all(|id| term(id)) {
            return Err(self.fail("invalid discovery fields"));
        }
        let mut fp = fingerprint(words[0]).ok_or_else(|| self.fail("invalid fingerprint"))?;
        let theory = !mbqi && !words[1].starts_with('#');
        if theory {
            if !matches!(fp, Fingerprint::Null | Fingerprint::Hash(0))
                || !theory_term(words[1])
                || !theory_term(words[2])
                || !words[3..].iter().all(|id| id.starts_with('#'))
            {
                return Err(self.fail("invalid theory match"));
            }
            // Both theory records print the same null pointer. Decimal zero
            // here is not a quantifier's numeric data hash, which may be zero.
            fp = Fingerprint::Null;
        } else if fp == Fingerprint::Null || !words[1..].iter().all(|id| id.starts_with('#')) {
            return Err(self.fail("invalid quantifier match"));
        }
        let owner = if theory {
            Owner::Theory
        } else if let Some(index) = self.terms.get(words[1]).copied() {
            let q = &mut self.quantifiers[index];
            if mbqi {
                q.mbqi_discoveries += 1;
            } else {
                q.matches += 1;
            }
            Owner::Quantifier(index)
        } else {
            self.discoveries += 1;
            Owner::Unknown
        };
        if mbqi && fp == Fingerprint::Null {
            return Err(self.fail("null model-based fingerprint"));
        }
        self.register(fp, owner)
    }
    fn discovered(&mut self, body: &str) -> Result<(), Error> {
        let (kind, rest) = body
            .split_once(' ')
            .ok_or_else(|| self.fail("invalid discovery kind"))?;
        match kind {
            "MBQI" => self.discovery(rest, true),
            "theory-solving" => {
                let (left, right) = rest.split_once(';').unwrap_or((rest, ""));
                let words: Vec<_> = left.split_ascii_whitespace().collect();
                let Some(raw) = words.first() else {
                    return Err(self.fail("missing theory fingerprint"));
                };
                let fp = fingerprint(raw).ok_or_else(|| self.fail("invalid theory fingerprint"))?;
                if !matches!(fp, Fingerprint::Null | Fingerprint::Hash(0))
                    || words.len() < 2
                    || !theory_term(words[1])
                    || !words[2..].iter().all(|id| term(id))
                    || !right.split_ascii_whitespace().all(term)
                {
                    return Err(self.fail("invalid theory discovery"));
                }
                self.register(fp, Owner::Theory)
            }
            _ => Err(self.fail("unsupported discovery kind")),
        }
    }
    fn instance(&mut self, body: &str) -> Result<(), Error> {
        let (head, generation) = body.split_once(';').map_or((body, None), |(a, b)| {
            (a, Some(b.trim_matches([' ', '\t'])))
        });
        let words: Vec<_> = head.split_ascii_whitespace().collect();
        let Some(raw) = words.first() else {
            return Err(self.fail("missing instance fingerprint"));
        };
        // The pinned quantifier producer always emits a generation separator.
        // Theory uses a null pointer, printed as either 0 or 0x0 by the stream.
        let fp = if generation.is_none() && matches!(*raw, "0" | "0x0") {
            Fingerprint::Null
        } else {
            fingerprint(raw).ok_or_else(|| self.fail("invalid instance fingerprint"))?
        };
        let valid = generation.map_or(
            fp == Fingerprint::Null && words.len() == 2 && term(words[1]),
            |value| {
                fp != Fingerprint::Null
                    && number(value).is_some()
                    && (words.len() == 1 || (words.len() == 2 && term(words[1])))
            },
        );
        if !valid || self.depth >= MAX_DEPTH {
            return Err(self.fail("invalid or over-deep instance"));
        }
        self.depth += 1;
        match self
            .fingerprints
            .get(&fp)
            .copied()
            .unwrap_or(Owner::Unknown)
        {
            Owner::Quantifier(index) => {
                self.quantifiers[index].instances += 1;
                self.instances += 1;
            }
            Owner::Theory => self.theory += 1,
            Owner::Unknown => self.unattributed += 1,
        }
        Ok(())
    }
}

fn number(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value: u32 = text.parse().ok()?;
    (value.to_string() == text).then_some(value)
}
fn fingerprint(text: &str) -> Option<Fingerprint> {
    if text == "0x0" {
        Some(Fingerprint::Null)
    } else {
        number(text).map(Fingerprint::Hash)
    }
}
fn term(text: &str) -> bool {
    let Some((prefix, id)) = text.split_once('#') else {
        return false;
    };
    (prefix.is_empty()
        || prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        && number(id).is_some()
}
fn qid(text: &str) -> bool {
    let mut lexer = Lexer::new(text.as_bytes());
    matches!(lexer.next(), Some(Ok(token)) if token.start == 0 && token.end == text.len()
        && matches!(token.kind, TokenKind::Symbol | TokenKind::QuotedSymbol | TokenKind::Numeral))
        && lexer.next().is_none()
}
fn theory_term(text: &str) -> bool {
    let Some((family, id)) = text.split_once('#') else {
        return false;
    };
    !family.is_empty()
        && family
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && (id.is_empty() || number(id).is_some())
}
fn explanation(text: &str) -> bool {
    let Some(tokens) = atoms(text) else {
        return false;
    };
    let mut pair = None;
    for token in tokens {
        match token {
            "(" if pair.is_none() => pair = Some(0),
            ")" if pair == Some(2) => pair = None,
            id if term(id) => {
                if let Some(count) = &mut pair {
                    *count += 1;
                    if *count > 2 {
                        return false;
                    }
                }
            }
            _ => return false,
        }
    }
    pair.is_none()
}

// Z3 trace term IDs (#123) are not SMT-LIB tokens. Only the qid is checked
// with the SMT lexer; this bounded trace cursor preserves its original bars.
fn atoms(mut text: &str) -> Option<Vec<&str>> {
    let mut tokens = Vec::new();
    loop {
        text = text.trim_start_matches([' ', '\t']);
        if text.is_empty() {
            return Some(tokens);
        }
        let end = if text.starts_with('|') {
            let end = text.get(1..)?.find('|')? + 2;
            if text
                .as_bytes()
                .get(end)
                .is_some_and(|b| !matches!(b, b' ' | b'\t' | b'(' | b')'))
            {
                return None;
            }
            end
        } else if text.starts_with(['(', ')']) {
            1
        } else {
            text.find([' ', '\t', '(', ')']).unwrap_or(text.len())
        };
        tokens.push(text.get(..end)?);
        text = text.get(end..)?;
    }
}

#[cfg(test)]
mod tests;
