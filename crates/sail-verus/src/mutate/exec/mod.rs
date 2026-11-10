//! Exec-only mutation sites.
//!
//! [`plan`] finds the sites of one file whose every replaced token is
//! executable code by the fail-closed exec mask (see `mask.rs`). The classes
//! ([`ExecClass`]) are the catalogue generator's body operators (`arith`,
//! `compare`, `constant`, `logic`, `statement`, `rare-region`) restricted
//! to the mask, plus the exec operators:
//!
//! - `loop-bound`: a comparison in a `while` condition flipped between
//!   strict and non-strict, or a `for` range switched between `..` and
//!   `..=`;
//! - `off-by-one`: ` + 1` or ` - 1` appended to an index expression, to a
//!   `for` range's end, or to a single-comparison `while` condition;
//! - `swapped-args`: two adjacent arguments of a call exchanged;
//! - `dropped-else`: the final `else { .. }` of a statement `if` removed;
//! - `early-return`: `return;` inserted before a statement of a function
//!   that returns `()`.
//!
//! Spec-mutating classes (`invariant`, `spec-weaken`, `spec-strengthen`)
//! have no [`ExecClass`], so a plan cannot contain them.
//!
//! Every site is re-lexed after it is applied: all tokens outside the
//! replaced range must be unchanged, or the site is dropped.

#![allow(
    clippy::many_single_char_names,
    reason = "token cursors: i/j/k index tokens, o/c a body's braces, s/e a byte range"
)]

mod mask;

use std::collections::BTreeSet;
use std::fmt;

use mask::{Analysis, ExecFn, analyse};
pub use mask::{Status, UnitFacts, collect_facts};

use super::lexer::{Kind, Tok, lex};
use super::sites::{self, File};

/// An exec-only mutation class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExecClass {
    /// An arithmetic operator swapped (`+` and `-`, `*` and `/`, `%`).
    Arith,
    /// A comparison flipped (`==`/`!=`, strict/non-strict).
    Compare,
    /// An integer literal changed by one or doubled.
    Constant,
    /// `&&`/`||` swapped, or a `!` dropped.
    Logic,
    /// A statement deleted.
    Statement,
    /// An early wrong return for one input value.
    RareRegion,
    /// A loop condition's comparison or a `for` range's end made
    /// strict/non-strict.
    LoopBound,
    /// ` + 1`/` - 1` appended to an index, range end or loop condition.
    OffByOne,
    /// Two adjacent call arguments exchanged.
    SwappedArgs,
    /// A statement `if`'s final `else` block removed.
    DroppedElse,
    /// `return;` inserted before a statement of a `()` function.
    EarlyReturn,
}

impl ExecClass {
    /// Every class, in report order.
    pub const ALL: &[Self] = &[
        Self::Arith,
        Self::Compare,
        Self::Constant,
        Self::Logic,
        Self::Statement,
        Self::RareRegion,
        Self::LoopBound,
        Self::OffByOne,
        Self::SwappedArgs,
        Self::DroppedElse,
        Self::EarlyReturn,
    ];

    /// The class's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Arith => "arith",
            Self::Compare => "compare",
            Self::Constant => "constant",
            Self::Logic => "logic",
            Self::Statement => "statement",
            Self::RareRegion => "rare-region",
            Self::LoopBound => "loop-bound",
            Self::OffByOne => "off-by-one",
            Self::SwappedArgs => "swapped-args",
            Self::DroppedElse => "dropped-else",
            Self::EarlyReturn => "early-return",
        }
    }

    /// The exec class of a catalogue body class; `None` for the classes
    /// that mutate specifications (`invariant`, `spec-weaken`,
    /// `spec-strengthen`) and for unknown names.
    #[must_use]
    pub fn from_catalogue(class: &str) -> Option<Self> {
        match class {
            "arith" => Some(Self::Arith),
            "compare" => Some(Self::Compare),
            "constant" => Some(Self::Constant),
            "logic" => Some(Self::Logic),
            "statement" => Some(Self::Statement),
            "rare-region" => Some(Self::RareRegion),
            _ => None,
        }
    }
}

impl fmt::Display for ExecClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One exec-only mutation: a replacement of one byte range of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecSite {
    /// The class.
    pub class: ExecClass,
    /// The function it is in.
    pub function: String,
    /// First replaced byte.
    pub start: usize,
    /// One past the last replaced byte (equal to `start` for an insertion).
    pub end: usize,
    /// The replacement text.
    pub replacement: String,
    /// What the mutation does.
    pub what: String,
}

impl ExecSite {
    /// The mutated file.
    #[must_use]
    pub fn apply(&self, src: &str) -> Option<String> {
        Some(format!(
            "{}{}{}",
            src.get(..self.start)?,
            self.replacement,
            src.get(self.end..)?
        ))
    }
}

/// One exec function of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnReport {
    /// Its name.
    pub name: String,
    /// Supported, external or unsupported (with the reason).
    pub status: Status,
    /// 1-based line of its `fn` keyword.
    pub line: usize,
}

/// One file's exec functions and sites.
#[derive(Debug, Clone)]
pub struct FilePlan {
    /// The exec functions with a body, in source order.
    pub fns: Vec<FnReport>,
    /// The sites, each with the index of its function in `fns`.
    pub sites: Vec<(usize, ExecSite)>,
    /// Sites dropped because re-lexing the mutant changed other tokens.
    pub dropped_relex: usize,
}

/// Whether applying `s` to `src` leaves every token outside the replaced
/// range unchanged.
fn preserved(src: &str, toks: &[Tok], s: &ExecSite) -> bool {
    let Some(mutated) = s.apply(src) else {
        return false;
    };
    let new = lex(&mutated);
    let pre = toks.partition_point(|t| t.end <= s.start);
    let post = toks.partition_point(|t| t.start < s.end);
    // No token straddles the replaced range's edges.
    if toks
        .get(pre..post)
        .unwrap_or_default()
        .iter()
        .any(|t| t.start < s.start || t.end > s.end)
    {
        return false;
    }
    let n_post = toks.len() - post;
    if new.len() < pre + n_post {
        return false;
    }
    let new_post = new.len() - n_post;
    let grown = s.replacement.len();
    let removed = s.end - s.start;
    let same_pre = new.get(..pre) == toks.get(..pre);
    let same_post = new
        .get(new_post..)
        .unwrap_or_default()
        .iter()
        .zip(toks.get(post..).unwrap_or_default())
        .all(|(n, o)| {
            n.kind == o.kind
                && n.start + removed == o.start + grown
                && n.end + removed == o.end + grown
        });
    let middle_inside = new
        .get(pre..new_post)
        .unwrap_or_default()
        .iter()
        .all(|t| t.start >= s.start && t.end <= s.start + grown);
    same_pre && same_post && middle_inside
}

/// Builds sites for one function.
struct Ops<'f, 'a> {
    file: &'f File<'a>,
    eligible: &'f [bool],
    f: &'f ExecFn,
    n: usize,
    out: &'f mut Vec<(usize, ExecSite)>,
}

impl Ops<'_, '_> {
    fn ok(&self, i: usize) -> bool {
        self.eligible.get(i).copied().unwrap_or(false)
    }

    fn all_ok(&self, from: usize, to_inclusive: usize) -> bool {
        (from..=to_inclusive).all(|i| self.ok(i))
    }

    fn push(&mut self, class: ExecClass, at: (usize, usize), r: &str, what: String) {
        self.out.push((
            self.n,
            ExecSite {
                class,
                function: self.f.name.clone(),
                start: at.0,
                end: at.1,
                replacement: r.to_owned(),
                what,
            },
        ));
    }

    /// Tokens `[a, b]` as one line of text.
    fn span(&self, a: usize, b: usize) -> String {
        self.file
            .src
            .get(self.file.start(a)..self.file.end(b))
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The catalogue generator's body classes over the placed body, masked.
    fn legacy(&mut self, key: &dyn Fn(&str, usize) -> u64) {
        let (file, f) = (self.file, self.f);
        let Some(mut func) = file.function(f.fn_tok, f.region_end) else {
            return;
        };
        func.body = Some(f.body);
        let mut found = Vec::new();
        sites::sites_of(file, &func, key, &mut found);
        for s in found {
            if s.kind != "body" {
                continue;
            }
            let Some(mut class) = ExecClass::from_catalogue(s.class) else {
                continue;
            };
            if s.start < file.end(f.body.0) || s.end > file.start(f.body.1) {
                continue;
            }
            let first = file.toks.partition_point(|t| t.end <= s.start);
            let last = file.toks.partition_point(|t| t.start < s.end);
            if s.start == s.end {
                // An insertion: only the generator's early return, directly
                // after the body's opening brace.
                if s.start != file.end(f.body.0) {
                    continue;
                }
            } else if first >= last || !(first..last).all(|i| self.ok(i)) {
                continue;
            }
            if class == ExecClass::Compare
                && f.loops
                    .iter()
                    .any(|l| !l.is_for && (l.cond.0..l.cond.1).contains(&first))
            {
                class = ExecClass::LoopBound;
            }
            self.push(class, (s.start, s.end), &s.replacement, s.what);
        }
    }

    /// Loop-bound and off-by-one sites of the loop headers.
    fn loops(&mut self) {
        let file = self.file;
        let loops = self.f.loops.clone();
        for l in &loops {
            let (a, b) = l.cond;
            if a >= b || !self.all_ok(a, b - 1) {
                continue;
            }
            // Top-level tokens of the condition.
            let mut top = Vec::new();
            let mut i = a;
            while i < b {
                top.push(i);
                if file.is_open(i) {
                    i = file.partner(i).map_or(b, |c| c + 1);
                } else {
                    i += 1;
                }
            }
            let last_end = file.end(b - 1);
            if l.is_for {
                let ranges: Vec<usize> = top
                    .iter()
                    .copied()
                    .filter(|&i| file.is(i, "..") || file.is(i, "..="))
                    .collect();
                for &i in &ranges {
                    let (r, t) = if file.is(i, "..") {
                        ("..=", "..")
                    } else {
                        ("..", "..=")
                    };
                    let what = format!("`{t}` to `{r}` in `for {}`", self.span(a, b - 1));
                    self.push(ExecClass::LoopBound, (file.start(i), file.end(i)), r, what);
                }
                if let [r] = ranges.as_slice()
                    && r + 1 < b
                {
                    for delta in [" + 1", " - 1"] {
                        let what =
                            format!("range end `{}` to `{0}{delta}`", self.span(r + 1, b - 1));
                        self.push(ExecClass::OffByOne, (last_end, last_end), delta, what);
                    }
                }
            } else {
                let compares = top
                    .iter()
                    .filter(|&&i| {
                        file.kind(i) == Some(Kind::Punct)
                            && matches!(file.text(i), "<" | ">" | "<=" | ">=" | "==" | "!=")
                    })
                    .count();
                let simple = !top.iter().any(|&i| {
                    matches!(
                        file.text(i),
                        "&&" | "||" | "let" | "=" | "|" | "&" | "^" | "<<" | ">>"
                    )
                });
                if compares == 1 && simple {
                    for delta in [" + 1", " - 1"] {
                        let what =
                            format!("loop condition `{}` to `{0}{delta}`", self.span(a, b - 1));
                        self.push(ExecClass::OffByOne, (last_end, last_end), delta, what);
                    }
                }
            }
        }
    }

    /// Off-by-one sites of plain index expressions `x[i]`.
    fn indexes(&mut self) {
        let file = self.file;
        let (o, c) = self.f.body;
        for i in o + 2..c {
            if !(file.is(i, "[") && self.ok(i) && self.ok(i - 1) && file.operand_end(i - 1)) {
                continue;
            }
            if matches!(file.text(i - 1), "as" | "mut" | "dyn" | "impl" | "const") {
                continue;
            }
            let Some(close) = file.partner(i) else {
                continue;
            };
            if close <= i + 1 || close >= c || !self.all_ok(i, close) {
                continue;
            }
            let mut plain = true;
            let mut j = i + 1;
            while j < close {
                if matches!(file.text(j), ".." | "..=" | "," | ";") {
                    plain = false;
                }
                if file.is_open(j) {
                    j = file.partner(j).map_or(close, |x| x + 1);
                } else {
                    j += 1;
                }
            }
            if !plain {
                continue;
            }
            let at = file.end(close - 1);
            for delta in [" + 1", " - 1"] {
                let what = format!("index `{}` to `{0}{delta}`", self.span(i + 1, close - 1));
                self.push(ExecClass::OffByOne, (at, at), delta, what);
            }
        }
    }

    /// Swapped-argument sites: in every call `f(a, b, ..)` (or method
    /// call) whose argument list is executable code, one adjacent pair of
    /// textually different arguments, chosen by `key`, exchanged.
    fn swapped_args(&mut self, key: &dyn Fn(&str, usize) -> u64) {
        let file = self.file;
        let (o, c) = self.f.body;
        for i in o + 2..c {
            if !(file.is(i, "(") && self.ok(i) && self.ok(i - 1)) {
                continue;
            }
            let callee = i - 1;
            if file.kind(callee) != Some(Kind::Ident)
                || super::sites::is_keyword(file.text(callee))
                || file.is(callee.saturating_sub(1), "fn")
            {
                continue;
            }
            let Some(close) = file.partner(i) else {
                continue;
            };
            if close >= c || !self.all_ok(i, close) {
                continue;
            }
            let args = file.split_commas(i + 1, close, false);
            if args.len() < 2 {
                continue;
            }
            // A single literal's kind, for refusing swaps that cannot type.
            let literal = |(a, b): (usize, usize)| {
                (b == a + 1)
                    .then(|| file.kind(a))
                    .flatten()
                    .filter(|k| matches!(k, Kind::Int | Kind::Float | Kind::Str | Kind::Char))
            };
            let pairs: Vec<usize> = (0..args.len() - 1)
                .filter(|&k| {
                    let (Some(&x), Some(&y)) = (args.get(k), args.get(k + 1)) else {
                        return false;
                    };
                    let kinds_clash =
                        matches!((literal(x), literal(y)), (Some(p), Some(q)) if p != q);
                    !kinds_clash && self.span(x.0, x.1 - 1) != self.span(y.0, y.1 - 1)
                })
                .collect();
            if pairs.is_empty() {
                continue;
            }
            let pick = usize::try_from(
                key("swapped-args", file.start(i)) % u64::try_from(pairs.len()).unwrap_or(1),
            )
            .unwrap_or(0);
            let Some(&k) = pairs.get(pick) else {
                continue;
            };
            let (Some(&(xa, xb)), Some(&(ya, yb))) = (args.get(k), args.get(k + 1)) else {
                continue;
            };
            let src = file.src;
            let (s, e) = (file.start(xa), file.end(yb - 1));
            let first = src.get(s..file.end(xb - 1)).unwrap_or_default();
            let between = src
                .get(file.end(xb - 1)..file.start(ya))
                .unwrap_or_default();
            let second = src.get(file.start(ya)..e).unwrap_or_default();
            let what = format!(
                "swap arguments `{}` and `{}` of `{}`",
                self.span(xa, xb - 1),
                self.span(ya, yb - 1),
                file.text(callee)
            );
            self.push(
                ExecClass::SwappedArgs,
                (s, e),
                &format!("{second}{between}{first}"),
                what,
            );
        }
    }

    /// Dropped-else sites: `if c { .. } else { .. }` whose then-block is
    /// empty or ends in `;` (so the `if` has type `()`) loses its final `else`
    /// block. `else if` chains keep their `else if` arms; only a final
    /// `else { .. }` is removed.
    fn dropped_else(&mut self) {
        let file = self.file;
        let (o, c) = self.f.body;
        for i in o + 2..c {
            if !(file.kind(i) == Some(Kind::Ident) && file.is(i, "else") && file.is(i + 1, "{")) {
                continue;
            }
            let then_close = i - 1;
            if !file.is(then_close, "}") {
                continue;
            }
            let Some(else_close) = file.partner(i + 1) else {
                continue;
            };
            if else_close >= c || !self.all_ok(then_close, else_close) {
                continue;
            }
            // The then-block's value is `()`: it is empty or ends in `;`. (A
            // block ending in `}` may end in a struct literal or a nested
            // `if` that produces the value.)
            let Some(then_open) = file.partner(then_close) else {
                continue;
            };
            if !(then_open + 1 == then_close || file.is(then_close - 1, ";")) {
                continue;
            }
            let what = format!("drop `else {}`", short(&self.span(i + 1, else_close)));
            self.push(
                ExecClass::DroppedElse,
                (file.end(then_close), file.end(else_close)),
                "",
                what,
            );
        }
    }

    /// Early-return sites: `return;` before each executable statement of a
    /// function that returns `()`.
    fn early_return(&mut self, ret: Option<&str>) {
        if !matches!(ret, None | Some("()")) {
            return;
        }
        let file = self.file;
        let (o, c) = self.f.body;
        let mut stmts = Vec::new();
        file.statements(o, c, &mut stmts);
        for (a, b) in stmts {
            if !self.all_ok(a, b) {
                continue;
            }
            let at = file.start(a);
            let indent = file.indent_at(at);
            let line_start = file
                .src
                .get(..at)
                .and_then(|t| t.rfind('\n'))
                .map_or(0, |q| q + 1);
            // On its own line when the statement starts one.
            let r = if file
                .src
                .get(line_start..at)
                .is_some_and(|t| t.trim().is_empty())
            {
                format!("return;\n{indent}")
            } else {
                "return; ".to_owned()
            };
            let what = format!("return before `{}`", short(&self.span(a, b)));
            self.push(ExecClass::EarlyReturn, (at, at), &r, what);
        }
    }
}

/// `text` shortened for a description.
fn short(text: &str) -> String {
    if text.chars().count() > 60 {
        let cut: String = text.chars().take(57).collect();
        format!("{cut}...")
    } else {
        text.to_owned()
    }
}

/// The exec sites of `src`. `facts` are the unit's ghost fields,
/// constructors and functions with ghost parameters ([`collect_facts`]
/// over every file of the unit); `key` gives a deterministic hash for a
/// site's identity, used to choose among a site's variants.
#[must_use]
pub fn plan(src: &str, facts: &UnitFacts, key: &dyn Fn(&str, usize) -> u64) -> FilePlan {
    let Analysis {
        file,
        fns,
        eligible,
    } = analyse(src, facts);
    let mut out: Vec<(usize, ExecSite)> = Vec::new();
    for (n, f) in fns.iter().enumerate() {
        if f.status != Status::Supported {
            continue;
        }
        let ret = file
            .function(f.fn_tok, f.region_end)
            .and_then(|func| func.ret);
        let mut ops = Ops {
            file: &file,
            eligible: &eligible,
            f,
            n,
            out: &mut out,
        };
        ops.legacy(key);
        ops.loops();
        ops.indexes();
        ops.swapped_args(key);
        ops.dropped_else();
        ops.early_return(ret.as_deref());
    }
    // One site per resulting text, then the preservation check.
    let mut seen = BTreeSet::new();
    let mut dropped_relex = 0;
    out.retain(|(_, s)| {
        if src.get(s.start..s.end) == Some(s.replacement.as_str())
            || !seen.insert((s.start, s.end, s.replacement.clone()))
        {
            return false;
        }
        if preserved(src, &file.toks, s) {
            true
        } else {
            dropped_relex += 1;
            false
        }
    });
    let line = |tok: usize| {
        file.src
            .get(..file.start(tok))
            .unwrap_or_default()
            .matches('\n')
            .count()
            + 1
    };
    FilePlan {
        fns: fns
            .iter()
            .map(|f| FnReport {
                name: f.name.clone(),
                status: f.status.clone(),
                line: line(f.fn_tok),
            })
            .collect(),
        sites: out,
        dropped_relex,
    }
}

#[cfg(test)]
mod tests;
