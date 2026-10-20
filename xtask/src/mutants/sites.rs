//! Finds mutation sites in one Verus source file (ADR 0063).
//!
//! Only code inside `verus! { ... }` is considered. Functions are found from
//! their `fn` keyword; their modifiers (`spec`, `proof`, ...), attributes,
//! parameters, return type, `requires`/`ensures` clauses and body are read
//! from the token stream. Every site is one replacement of a byte range of
//! the original file, so a mutant is always the original text with exactly
//! one change.

use super::lexer::{Kind, Tok, lex, match_brackets};

/// The nine mutation classes, in catalogue order.
pub const CLASSES: &[&str] = &[
    "arith",
    "compare",
    "constant",
    "logic",
    "statement",
    "invariant",
    "spec-weaken",
    "spec-strengthen",
    "rare-region",
];

/// One candidate mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// The class (one of [`CLASSES`]).
    pub class: &'static str,
    /// The function it is in.
    pub function: String,
    /// `spec`, `proof` or `exec`.
    pub mode: &'static str,
    /// `body` (a function body) or `spec` (a `requires`/`ensures` clause).
    pub kind: &'static str,
    /// First replaced byte.
    pub start: usize,
    /// One past the last replaced byte.
    pub end: usize,
    /// The replacement text.
    pub replacement: String,
    /// What the mutation does, for the catalogue description.
    pub what: String,
}

/// Integer types a rare-region guard can compare against a literal.
const INT_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

/// Keywords that end a function signature's clause, or start one.
const CLAUSE_KWS: &[&str] = &[
    "requires",
    "ensures",
    "recommends",
    "decreases",
    "returns",
    "opens_invariants",
    "no_unwind",
    "default_ensures",
    "when",
    "via",
    "where",
];

/// Keywords that end a loop's `invariant` clause.
const LOOP_CLAUSE_KWS: &[&str] = &[
    "invariant",
    "invariant_except_break",
    "invariant_ensures",
    "ensures",
    "decreases",
];

/// Identifiers that are keywords, not operands, for deciding whether an
/// operator is binary.
const KEYWORDS: &[&str] = &[
    "return",
    "in",
    "if",
    "else",
    "let",
    "mut",
    "match",
    "while",
    "for",
    "loop",
    "break",
    "continue",
    "by",
    "requires",
    "ensures",
    "invariant",
    "invariant_except_break",
    "decreases",
    "recommends",
    "assert",
    "assume",
    "forall",
    "exists",
    "choose",
    "implies",
    "trigger",
    "move",
    "ref",
    "tracked",
    "ghost",
    "proof",
    "when",
    "via",
    "returns",
];

/// Function modifiers that may precede `fn`.
const MODIFIERS: &[&str] = &[
    "pub",
    "open",
    "closed",
    "spec",
    "proof",
    "exec",
    "broadcast",
    "const",
    "unsafe",
    "uninterp",
    "axiom",
    "async",
    "extern",
    "default",
];

/// A signature clause: `(keyword, keyword token, items as token ranges)`.
type Clause = (String, usize, Vec<(usize, usize)>);

/// A parsed function.
struct Func {
    name: String,
    mode: &'static str,
    external: bool,
    params: Vec<(String, String)>,
    ret: Option<String>,
    clauses: Vec<Clause>,
    /// Token indices of the body's braces.
    body: Option<(usize, usize)>,
}

struct File<'a> {
    src: &'a str,
    toks: Vec<Tok>,
    pair: Vec<Option<usize>>,
}

impl File<'_> {
    fn text(&self, i: usize) -> &str {
        self.toks
            .get(i)
            .and_then(|t| self.src.get(t.start..t.end))
            .unwrap_or_default()
    }

    fn is(&self, i: usize, s: &str) -> bool {
        self.text(i) == s
    }

    fn kind(&self, i: usize) -> Option<Kind> {
        self.toks.get(i).map(|t| t.kind)
    }

    fn start(&self, i: usize) -> usize {
        self.toks.get(i).map_or(self.src.len(), |t| t.start)
    }

    fn end(&self, i: usize) -> usize {
        self.toks.get(i).map_or(self.src.len(), |t| t.end)
    }

    fn partner(&self, i: usize) -> Option<usize> {
        self.pair.get(i).copied().flatten()
    }

    fn is_open(&self, i: usize) -> bool {
        self.kind(i) == Some(Kind::Punct) && matches!(self.text(i), "(" | "[" | "{")
    }

    /// Whether token `i` ends an operand, so that an operator after it is
    /// binary.
    fn operand_end(&self, i: usize) -> bool {
        match self.kind(i) {
            Some(Kind::Ident) => !KEYWORDS.contains(&self.text(i)),
            Some(Kind::Int | Kind::Float | Kind::Str | Kind::Char) => true,
            Some(Kind::Punct) => matches!(self.text(i), ")" | "]"),
            _ => false,
        }
    }

    /// The token ranges of `verus! { ... }` bodies.
    fn verus_regions(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut i = 0;
        while i + 2 < self.toks.len() {
            if self.kind(i) == Some(Kind::Ident)
                && self.is(i, "verus")
                && self.is(i + 1, "!")
                && self.is_open(i + 2)
                && let Some(close) = self.partner(i + 2)
            {
                out.push((i + 3, close));
                i = close;
            }
            i += 1;
        }
        out
    }

    /// Skips a generic parameter list starting at `i` (`<`); returns the
    /// index after it.
    fn skip_angles(&self, mut i: usize, limit: usize) -> usize {
        let mut depth = 0i32;
        while i < limit {
            match self.text(i) {
                "<" => depth += 1,
                ">" => depth -= 1,
                ">>" => depth -= 2,
                "(" | "[" | "{" => {
                    if let Some(c) = self.partner(i) {
                        i = c;
                    }
                }
                _ => {}
            }
            i += 1;
            if depth <= 0 {
                break;
            }
        }
        i
    }

    /// Splits tokens `[from, to)` at top-level commas (skipping bracket
    /// groups and, when `angles`, `<...>` generics).
    fn split_commas(&self, from: usize, to: usize, angles: bool) -> Vec<(usize, usize)> {
        let mut items = Vec::new();
        let mut start = from;
        let mut i = from;
        let mut depth = 0i32;
        while i < to {
            if self.is_open(i) {
                i = self.partner(i).map_or(i, |c| c.min(to));
            } else if self.is(i, "|") && !(i > from && self.operand_end(i - 1)) {
                // Closure parameters (`forall|a: A, b: B|`): one group.
                i += 1;
                while i < to && !self.is(i, "|") {
                    i += 1;
                }
            } else if angles && self.is(i, "<") {
                depth += 1;
            } else if angles && self.is(i, ">") {
                depth -= 1;
            } else if angles && self.is(i, ">>") {
                depth -= 2;
            } else if self.is(i, ",") && depth <= 0 {
                if i > start {
                    items.push((start, i));
                }
                start = i + 1;
            }
            i += 1;
        }
        if to > start {
            items.push((start, to));
        }
        items
    }

    /// The function whose `fn` keyword is token `f`, if it parses.
    #[allow(
        clippy::too_many_lines,
        clippy::many_single_char_names,
        reason = "one walk over the signature; f/i/j/p are token cursors"
    )]
    fn function(&self, f: usize, region_end: usize) -> Option<Func> {
        if self.kind(f + 1) != Some(Kind::Ident) {
            return None;
        }
        let name = self.text(f + 1).to_owned();
        // Modifiers and attributes, walking backwards.
        let mut mode = "exec";
        let mut external = false;
        let mut j = f;
        while j > 0 {
            let p = j - 1;
            if self.kind(p) == Some(Kind::Ident) && MODIFIERS.contains(&self.text(p)) {
                match self.text(p) {
                    "spec" => mode = "spec",
                    "proof" => mode = "proof",
                    _ => {}
                }
                j = p;
            } else if self.is(p, ")") {
                // `pub(crate)`
                let open = self.partner(p)?;
                if open > 0 && self.is(open - 1, "pub") {
                    j = open - 1;
                } else {
                    break;
                }
            } else if self.is(p, "]") {
                let open = self.partner(p)?;
                let hash = if open > 0 && self.is(open - 1, "#") {
                    open - 1
                } else {
                    break;
                };
                let attr = self
                    .src
                    .get(self.start(open)..self.end(p))
                    .unwrap_or_default();
                if attr.contains("external") {
                    external = true;
                }
                j = hash;
            } else {
                break;
            }
        }
        let mut i = f + 2;
        if self.is(i, "<") {
            i = self.skip_angles(i, region_end);
        }
        if !self.is(i, "(") {
            return None;
        }
        let pclose = self.partner(i)?;
        let params = self
            .split_commas(i + 1, pclose, true)
            .into_iter()
            .filter_map(|(a, b)| self.param(a, b))
            .collect();
        i = pclose + 1;
        let mut ret = None;
        if self.is(i, "->") {
            i += 1;
            if self.is(i, "(")
                && self.kind(i + 1) == Some(Kind::Ident)
                && self.is(i + 2, ":")
                && let Some(close) = self.partner(i)
            {
                ret = self
                    .src
                    .get(self.end(i + 2)..self.start(close))
                    .map(|s| s.trim().to_owned());
                i = close + 1;
            } else {
                let from = i;
                let mut depth = 0i32;
                while i < region_end {
                    let t = self.text(i);
                    if depth <= 0 && (t == "{" || t == ";" || CLAUSE_KWS.contains(&t)) {
                        break;
                    }
                    match t {
                        "<" => depth += 1,
                        ">" => depth -= 1,
                        ">>" => depth -= 2,
                        "(" | "[" => i = self.partner(i).unwrap_or(i),
                        _ => {}
                    }
                    i += 1;
                }
                ret = self
                    .src
                    .get(self.start(from)..self.start(i))
                    .map(|s| s.trim().to_owned());
            }
        }
        let mut clauses = Vec::new();
        let mut body = None;
        while i < region_end {
            let t = self.text(i);
            if t == ";" {
                break;
            }
            if t == "{" {
                body = self.partner(i).map(|c| (i, c));
                break;
            }
            if self.kind(i) == Some(Kind::Ident) && CLAUSE_KWS.contains(&t) {
                let kw = i;
                i += 1;
                let from = i;
                while i < region_end {
                    let u = self.text(i);
                    if u == "{"
                        || u == ";"
                        || (CLAUSE_KWS.contains(&u) && self.kind(i) == Some(Kind::Ident))
                    {
                        break;
                    }
                    if self.is_open(i) {
                        i = self.partner(i).unwrap_or(i);
                    }
                    i += 1;
                }
                clauses.push((t.to_owned(), kw, self.split_commas(from, i, false)));
                continue;
            }
            if self.is_open(i) {
                i = self.partner(i).unwrap_or(i);
            }
            i += 1;
        }
        Some(Func {
            name,
            mode,
            external,
            params,
            ret,
            clauses,
            body,
        })
    }

    /// A by-value parameter `[mut] name: Type`.
    fn param(&self, a: usize, b: usize) -> Option<(String, String)> {
        let mut i = a;
        if self.is(i, "mut") {
            i += 1;
        }
        if self.kind(i) != Some(Kind::Ident) || !self.is(i + 1, ":") || i + 2 >= b {
            return None;
        }
        let ty = self.src.get(self.start(i + 2)..self.end(b - 1))?.trim();
        Some((self.text(i).to_owned(), ty.to_owned()))
    }

    /// Token ranges of `#[...]` / `#![...]` attributes inside `[from, to)`.
    fn attributes(&self, from: usize, to: usize) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for i in from..to {
            if self.is(i, "#") {
                let open = if self.is(i + 1, "!") { i + 2 } else { i + 1 };
                if self.is(open, "[")
                    && let Some(close) = self.partner(open)
                {
                    out.push((i, close));
                }
            }
        }
        out
    }

    /// Byte range `[s, e)` grown to whole lines when only whitespace
    /// surrounds it on its first and last lines.
    fn whole_lines(&self, s: usize, e: usize) -> (usize, usize) {
        let ls = self
            .src
            .get(..s)
            .and_then(|t| t.rfind('\n'))
            .map_or(0, |p| p + 1);
        let le = self
            .src
            .get(e..)
            .and_then(|t| t.find('\n'))
            .map_or(self.src.len(), |p| e + p);
        let before = self.src.get(ls..s).unwrap_or_default();
        let after = self.src.get(e..le).unwrap_or_default();
        if before.trim().is_empty() && after.trim().is_empty() {
            (ls, (le + 1).min(self.src.len()))
        } else {
            (s, e)
        }
    }

    /// The indentation of the line holding byte `p`.
    fn indent_at(&self, p: usize) -> String {
        let ls = self
            .src
            .get(..p)
            .and_then(|t| t.rfind('\n'))
            .map_or(0, |q| q + 1);
        self.src
            .get(ls..)
            .unwrap_or_default()
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect()
    }

    /// Statements ending in `;` in the block whose braces are tokens
    /// `open`/`close`, and in nested blocks: `(first token, the `;`)`.
    fn statements(&self, open: usize, close: usize, out: &mut Vec<(usize, usize)>) {
        let mut i = open + 1;
        while i < close {
            let first = i;
            let is_let = self.is(first, "let");
            let mut nested = Vec::new();
            let mut end = None;
            while i < close {
                if self.is(i, ";") {
                    end = Some(i);
                    i += 1;
                    break;
                }
                if self.is(i, "{") {
                    let c = self.partner(i).unwrap_or(i).min(close);
                    nested.push((i, c));
                    i = c + 1;
                    if !is_let
                        && !self.is(i, "else")
                        && !self.is(i, ";")
                        && !self.is(i, ".")
                        && !self.is(i, "?")
                    {
                        break;
                    }
                    continue;
                }
                if self.is_open(i) {
                    i = self.partner(i).unwrap_or(i).min(close);
                }
                i += 1;
            }
            for (o, c) in nested {
                self.statements(o, c, out);
            }
            if let Some(e) = end
                && !matches!(
                    self.text(first),
                    "let" | "return" | "break" | "continue" | ";" | "}"
                )
                && e > first
            {
                out.push((first, e));
            }
            if i == first {
                i += 1;
            }
        }
    }
}

/// A small deterministic choice from `n` options, keyed by `key`.
fn pick(key: u64, n: usize) -> usize {
    usize::try_from(key % u64::try_from(n.max(1)).unwrap_or(1)).unwrap_or(0)
}

/// Every candidate site in `src`. `key` gives a deterministic hash for a
/// site's identity (used to choose among a site's variants).
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one pass over each function, class by class, reads best in one place"
)]
pub fn find(src: &str, key: &dyn Fn(&str, usize) -> u64) -> Vec<Site> {
    let toks = lex(src);
    let pair = match_brackets(src, &toks);
    let file = File { src, toks, pair };
    let mut out = Vec::new();
    for (ra, rb) in file.verus_regions() {
        let mut i = ra;
        while i < rb {
            if !(file.kind(i) == Some(Kind::Ident) && file.is(i, "fn")) {
                i += 1;
                continue;
            }
            let Some(func) = file.function(i, rb) else {
                i += 1;
                continue;
            };
            let next = func.body.map_or(i + 1, |(_, c)| c + 1);
            sites_of(&file, &func, key, &mut out);
            i = next;
        }
    }
    out
}

#[allow(
    clippy::too_many_lines,
    reason = "every class's sites for one function, in class order"
)]
fn sites_of(file: &File, func: &Func, key: &dyn Fn(&str, usize) -> u64, out: &mut Vec<Site>) {
    let site =
        |class: &'static str, kind: &'static str, s: usize, e: usize, r: String, what: String| {
            Site {
                class,
                function: func.name.clone(),
                mode: func.mode,
                kind,
                start: s,
                end: e,
                replacement: r,
                what,
            }
        };
    // Token ranges for operator classes: the body (unless external) and
    // the requires/ensures items.
    let mut ranges: Vec<(usize, usize, &'static str)> = Vec::new();
    if let Some((o, c)) = func.body
        && !func.external
    {
        ranges.push((o + 1, c, "body"));
    }
    for (kw, _, items) in &func.clauses {
        if kw == "requires" || kw == "ensures" {
            for (a, b) in items {
                ranges.push((*a, *b, "spec"));
            }
        }
    }
    for &(a, b, kind) in &ranges {
        let attrs = file.attributes(a, b);
        let in_attr = |i: usize| attrs.iter().any(|(x, y)| (*x..=*y).contains(&i));
        for i in a..b {
            if in_attr(i) || file.kind(i) == Some(Kind::Lifetime) {
                continue;
            }
            let t = file.text(i);
            let (s, e) = (file.start(i), file.end(i));
            let prev_operand = i > a && file.operand_end(i - 1);
            match (file.kind(i), t) {
                (Some(Kind::Punct), "+" | "-" | "*" | "/" | "%") if prev_operand && i + 1 < b => {
                    let r = match t {
                        "+" => "-",
                        "-" => "+",
                        "*" => "/",
                        _ => "*",
                    };
                    let r = if t == "%" { "/" } else { r };
                    out.push(site(
                        "arith",
                        kind,
                        s,
                        e,
                        r.to_owned(),
                        format!("`{t}` to `{r}`"),
                    ));
                }
                (Some(Kind::Punct), "==" | "!=" | "<=" | ">=" | "<" | ">") => {
                    let spaced = |p: usize| {
                        file.src
                            .as_bytes()
                            .get(p)
                            .is_some_and(u8::is_ascii_whitespace)
                    };
                    if matches!(t, "<" | ">") && !(s > 0 && spaced(s - 1) && spaced(e)) {
                        continue;
                    }
                    let r = match t {
                        "==" => "!=",
                        "!=" => "==",
                        "<=" => "<",
                        ">=" => ">",
                        "<" => "<=",
                        _ => ">=",
                    };
                    out.push(site(
                        "compare",
                        kind,
                        s,
                        e,
                        r.to_owned(),
                        format!("`{t}` to `{r}`"),
                    ));
                }
                (Some(Kind::Punct), "&&" | "||") if prev_operand => {
                    let r = if t == "&&" { "||" } else { "&&" };
                    out.push(site(
                        "logic",
                        kind,
                        s,
                        e,
                        r.to_owned(),
                        format!("`{t}` to `{r}`"),
                    ));
                }
                (Some(Kind::Punct), "!")
                    if !prev_operand
                        && (file.kind(i + 1) == Some(Kind::Ident) || file.is(i + 1, "("))
                        && !(file.kind(i + 1) == Some(Kind::Ident)
                            && KEYWORDS.contains(&file.text(i + 1))) =>
                {
                    out.push(site(
                        "logic",
                        kind,
                        s,
                        e,
                        String::new(),
                        "drop a `!`".to_owned(),
                    ));
                }
                (Some(Kind::Int), _) if !(i > a && file.is(i - 1, ".")) => {
                    if let Some((r, what)) = constant(t, key("constant", s)) {
                        out.push(site("constant", kind, s, e, r, what));
                    }
                }
                _ => {}
            }
        }
    }
    // statement and invariant: executable and proof bodies.
    if let Some((o, c)) = func.body
        && !func.external
        && func.mode != "spec"
    {
        let mut stmts = Vec::new();
        file.statements(o, c, &mut stmts);
        for (a, b) in stmts {
            let (s, e) = file.whole_lines(file.start(a), file.end(b));
            let text = file.src.get(file.start(a)..file.end(b)).unwrap_or_default();
            out.push(site(
                "statement",
                "body",
                s,
                e,
                String::new(),
                format!("delete `{}`", short(text)),
            ));
        }
        for i in o + 1..c {
            if !(file.kind(i) == Some(Kind::Ident)
                && matches!(file.text(i), "invariant" | "invariant_except_break"))
            {
                continue;
            }
            let mut j = i + 1;
            while j < c {
                let u = file.text(j);
                if u == "{" || (file.kind(j) == Some(Kind::Ident) && LOOP_CLAUSE_KWS.contains(&u)) {
                    break;
                }
                if file.is_open(j) {
                    j = file.partner(j).unwrap_or(j);
                }
                j += 1;
            }
            let items = file.split_commas(i + 1, j, false);
            for (n, &(a, b)) in items.iter().enumerate() {
                let (s, e) = clause_item_range(file, i, &items, n, a, b);
                let text = file
                    .src
                    .get(file.start(a)..file.end(b - 1))
                    .unwrap_or_default();
                out.push(site(
                    "invariant",
                    "body",
                    s,
                    e,
                    String::new(),
                    format!("delete loop invariant `{}`", short(text)),
                ));
            }
        }
    }
    // spec-weaken and spec-strengthen: requires/ensures items.
    for (kw, kwi, items) in &func.clauses {
        let class = match kw.as_str() {
            "requires" => "spec-weaken",
            "ensures" => "spec-strengthen",
            _ => continue,
        };
        for (n, &(a, b)) in items.iter().enumerate() {
            let text = file
                .src
                .get(file.start(a)..file.end(b - 1))
                .unwrap_or_default();
            if class == "spec-strengthen" {
                let bounds: Vec<usize> = (a..b)
                    .filter(|&i| file.is(i, "<=") || file.is(i, ">="))
                    .collect();
                if !bounds.is_empty() && pick(key("tighten", file.start(a)), 2) == 0 {
                    let i = bounds
                        .get(pick(key("bound", file.start(a)), bounds.len()))
                        .copied()
                        .unwrap_or(a);
                    let r = if file.is(i, "<=") { "<" } else { ">" };
                    out.push(site(
                        class,
                        "spec",
                        file.start(i),
                        file.end(i),
                        r.to_owned(),
                        format!(
                            "tighten `{}` to `{r}` in ensures `{}`",
                            file.text(i),
                            short(text)
                        ),
                    ));
                    continue;
                }
            }
            let (s, e) = clause_item_range(file, *kwi, items, n, a, b);
            out.push(site(
                class,
                "spec",
                s,
                e,
                String::new(),
                format!("delete {kw} `{}`", short(text)),
            ));
        }
    }
    // rare-region: an early wrong return for one input value.
    let ret = func.ret.as_deref().unwrap_or("()");
    if let Some((o, c)) = func.body
        && !func.external
        && func.mode == "exec"
        && func.clauses.iter().any(|(k, _, _)| k == "ensures")
        && (INT_TYPES.contains(&ret) || ret == "bool" || ret == "()")
    {
        let ints: Vec<&(String, String)> = func
            .params
            .iter()
            .filter(|(_, ty)| INT_TYPES.contains(&ty.as_str()))
            .collect();
        if !ints.is_empty() {
            let k0 = key("rare-region", file.start(o));
            let (p, _) = ints.get(pick(k0, ints.len())).copied().unwrap_or(ints[0]);
            let k = [1u64, 2, 3, 7, 10]
                .get(pick(k0 >> 8, 5))
                .copied()
                .unwrap_or(7);
            let w = if ret == "()" {
                String::new()
            } else if ret == "bool" {
                if pick(k0 >> 16, 2) == 0 {
                    "false"
                } else {
                    "true"
                }
                .to_owned()
            } else {
                [0, 1, k + 1]
                    .get(pick(k0 >> 16, 3))
                    .copied()
                    .unwrap_or(0)
                    .to_string()
            };
            let indent = if o + 1 < c {
                file.indent_at(file.start(o + 1))
            } else {
                format!("{}    ", file.indent_at(file.start(o)))
            };
            let at = file.end(o);
            let (ret_stmt, what) = if w.is_empty() {
                (
                    "return;".to_owned(),
                    format!("return early when `{p} == {k}`"),
                )
            } else {
                (
                    format!("return {w};"),
                    format!("return `{w}` early when `{p} == {k}`"),
                )
            };
            let text = format!("\n{indent}if {p} == {k} {{\n{indent}    {ret_stmt}\n{indent}}}");
            out.push(site("rare-region", "body", at, at, text, what));
        }
    }
}

/// The byte range deleting item `n` of a comma-separated clause whose
/// keyword is token `kw`: the item and its trailing comma, or the keyword
/// too when it is the only item; grown to whole lines where it can be.
fn clause_item_range(
    file: &File,
    kw: usize,
    items: &[(usize, usize)],
    n: usize,
    a: usize,
    b: usize,
) -> (usize, usize) {
    let end_tok = if file.is(b, ",") { b } else { b - 1 };
    let start = if items.len() == 1 {
        file.start(kw)
    } else {
        file.start(a)
    };
    let _ = n;
    file.whole_lines(start, file.end(end_tok))
}

/// A literal's mutated value: `k + 1`, `k - 1` or `2k`, keeping its suffix.
fn constant(t: &str, key: u64) -> Option<(String, String)> {
    if t.starts_with("0x") || t.starts_with("0o") || t.starts_with("0b") {
        return None;
    }
    let digits_end = t
        .find(|c: char| !(c.is_ascii_digit() || c == '_'))
        .unwrap_or(t.len());
    let (num, suffix) = t.split_at(digits_end);
    let k: u128 = num.replace('_', "").parse().ok()?;
    let mut variants = vec![k.checked_add(1)?];
    if k >= 1 {
        variants.push(k - 1);
    }
    if k >= 2 {
        variants.push(k.checked_mul(2)?);
    }
    let v = variants.get(pick(key, variants.len())).copied()?;
    let r = format!("{v}{suffix}");
    Some((r.clone(), format!("`{t}` to `{r}`")))
}

/// The first line of `text`, shortened for a description.
fn short(text: &str) -> String {
    let line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > 60 {
        let cut: String = line.chars().take(57).collect();
        format!("{cut}...")
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "use vstd::prelude::*;
verus! {
spec fn double(x: nat) -> nat { x * 2 }

fn max(a: u64, b: u64) -> (r: u64)
    requires
        a < 100,
        b < 100,
    ensures
        r >= a,
        r == a || r == b,
{
    let mut i: u64 = 0;
    while i < a
        invariant
            i <= a,
            a < 100,
        decreases a - i,
    {
        i = i + 1;
    }
    assert(i == a);
    if a > b { a } else { b }
}

#[verifier::external_body]
fn ext(v: Vec<u8>) -> u8 { v[0] + 1 }
} // verus!
fn not_verus(x: u8) -> u8 { x + 1 }
";

    fn sites() -> Vec<Site> {
        find(SRC, &|_, s| u64::try_from(s).unwrap_or(0))
    }

    fn apply(s: &Site) -> String {
        format!(
            "{}{}{}",
            SRC.get(..s.start).unwrap_or_default(),
            s.replacement,
            SRC.get(s.end..).unwrap_or_default()
        )
    }

    #[test]
    fn finds_every_class() {
        let all = sites();
        for class in CLASSES {
            assert!(
                all.iter().any(|s| s.class == *class),
                "no {class} site in {all:#?}"
            );
        }
        // Nothing outside verus!, and no body sites in external functions.
        assert!(all.iter().all(|s| s.function != "not_verus"));
        assert!(all.iter().all(|s| s.function != "ext" || s.kind == "spec"));
    }

    #[test]
    fn mutations_are_well_formed() {
        let all = sites();
        let deleted: Vec<String> = all
            .iter()
            .filter(|s| s.class == "statement")
            .map(apply)
            .collect();
        assert!(deleted.iter().any(|m| !m.contains("assert(i == a);")));
        assert!(deleted.iter().any(|m| !m.contains("i = i + 1;")));
        // `let` statements are never deleted.
        assert!(deleted.iter().all(|m| m.contains("let mut i: u64 = 0;")));
        let inv: Vec<String> = all
            .iter()
            .filter(|s| s.class == "invariant")
            .map(apply)
            .collect();
        assert_eq!(inv.len(), 2);
        assert!(
            inv.iter()
                .any(|m| !m.contains("i <= a,") && m.contains("a < 100,\n        decreases"))
        );
        let weak: Vec<String> = all
            .iter()
            .filter(|s| s.class == "spec-weaken")
            .map(apply)
            .collect();
        assert!(
            weak.iter()
                .any(|m| m.contains("requires\n        b < 100,\n    ensures"))
        );
        // `<` in `Vec<u8>` is a generic, never a comparison.
        assert!(
            all.iter()
                .all(|s| s.class != "compare" || s.start != SRC.find("<u8>").unwrap_or(0))
        );
        let rare: Vec<String> = all
            .iter()
            .filter(|s| s.class == "rare-region")
            .map(apply)
            .collect();
        assert_eq!(rare.len(), 1);
        assert!(rare.iter().all(|m| m.contains("{\n    if ")));
    }

    #[test]
    fn mutates_constants() {
        assert_eq!(constant("0", 0).map(|c| c.0).as_deref(), Some("1"));
        assert_eq!(constant("5u64", 1).map(|c| c.0).as_deref(), Some("4u64"));
        assert_eq!(constant("1_000", 2).map(|c| c.0).as_deref(), Some("2000"));
        assert!(constant("0xff", 0).is_none());
    }
}
