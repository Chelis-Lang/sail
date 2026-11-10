//! The exec mask: which tokens of an executable function's body are
//! executable code, failing closed.
//!
//! [`analyse`] finds every executable function inside `verus! { ... }` and
//! decides, token by token, which body tokens are executable code. A token
//! is eligible only when it is outside every recognised ghost construct:
//!
//! - `proof { ... }` blocks;
//! - `assert`/`assume` forms (with `by (...)`, `by { ... }`, `requires`),
//!   `reveal`, `reveal_with_fuel`, `hide`, `broadcast use`;
//! - loop `invariant`/`invariant_except_break`/`ensures`/`decreases`
//!   clauses;
//! - `Ghost`/`Tracked` forms: the tuple-struct construction or pattern
//!   `Ghost(...)`, associated calls with any turbofish
//!   (`Ghost::new(...)`, `Ghost::<T>::new(...)`,
//!   `Tracked::<T>::method::<U>(...)`), after any leading path
//!   (`vstd::prelude::Ghost::new(...)`), and `Ghost<T>` types;
//! - `let ghost`/`let tracked` statements;
//! - attributes and macro invocations (never mutated).
//!
//! Spec and proof functions have no eligible tokens at all, and neither do
//! a function's own `requires`/`ensures`/`decreases` clauses, which lie
//! outside its body.
//!
//! The mask fails closed. A function is `unsupported`, and yields no sites,
//! when anything the lexer cannot classify remains: an unknown macro, a
//! brace the loop-header scan cannot place, a spec-only token left outside
//! the mask (`@`, `==>`, `forall`, `int`, ...), a closure with its own
//! `requires`/`ensures` (a spec closure), a struct literal or call that
//! names a ghost field or a function with ghost parameters, a `Ghost` or
//! `Tracked` form the mask cannot bound (a qualified path such as
//! `<Ghost<T>>::new(e)`, or one followed by a method call, field, index,
//! `?` or brace group), an attribute in the body that names a mode,
//! `trigger`, `auto` or `external`, a nested `fn`, or an enclosing item
//! that is not an `impl`/`trait`/`mod`.
//!
//! The mask relies on Verus's own mode checker: in a project that verifies,
//! ghost code in an exec function can only appear inside the constructs
//! above, in positions whose mode is declared elsewhere (ghost struct
//! fields, ghost parameters, a tracked return), or in exec closures'
//! `requires`/`ensures`. The last three are found by name and refuse the
//! function.

#![allow(
    clippy::many_single_char_names,
    reason = "token cursors: i/j/k/t index tokens, o/c a body's braces, f the `fn` token"
)]

use std::collections::BTreeSet;

use super::super::lexer::Kind;
use super::super::sites::{Brace, File, LOOP_CLAUSE_KWS};

/// Macros whose arguments are executable Rust. They are masked (never
/// mutated) but do not make the function unsupported.
const EXEC_MACROS: &[&str] = &[
    "vec",
    "matches",
    "println",
    "print",
    "eprintln",
    "eprint",
    "format",
    "panic",
    "unreachable",
    "unimplemented",
    "todo",
    "write",
    "writeln",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "dbg",
];

/// Ghost-only vstd macros: masked, and the function stays supported.
const GHOST_MACROS: &[&str] = &[
    "assert_seqs_equal",
    "assert_sets_equal",
    "assert_maps_equal",
    "assert_isets_equal",
    "assert_imaps_equal",
    "assert_multisets_equal",
    "assert_by_contradiction",
    "calc",
    "seq",
    "set",
    "map",
    "iset",
    "imap",
    "set_build",
];

/// Statement-like ghost forms that run to their `;`.
const GHOST_STATEMENTS: &[&str] = &[
    "assert",
    "assume",
    "assert_by",
    "assert_forall_by",
    "assert_bit_vector",
    "assert_bitvector_by",
    "assert_nonlinear_by",
    "reveal",
    "reveal_with_fuel",
    "hide",
    "admit",
];

/// Identifiers that only occur in ghost code or in clauses the mask did not
/// place. One of them left outside the mask refuses the function.
const TRIP_IDENTS: &[&str] = &[
    "forall",
    "exists",
    "choose",
    "implies",
    "int",
    "nat",
    "ghost",
    "tracked",
    "proof",
    "spec",
    "requires",
    "ensures",
    "recommends",
    "invariant",
    "invariant_except_break",
    "invariant_ensures",
    "decreases",
    "via",
    "trigger",
    "fn",
    "is",
    "matches",
    "has",
    "by",
    "with",
    "calc",
    "opens_invariants",
];

/// Spec-only operators.
const TRIP_PUNCTS: &[&str] = &[
    "@", "==>", "<==", "<==>", "=~=", "=~~=", "!~=", "!~~=", "===", "&&&", "|||",
];

/// Keywords that introduce a brace group inside an expression, which the
/// loop-header scan cannot tell from the loop body.
const BRACE_KWS: &[&str] = &[
    "if", "match", "unsafe", "loop", "while", "for", "async", "const",
];

/// Tokens after a brace group in a loop's clauses that mean the group
/// belongs to a clause expression, not the body.
const AFTER_CLAUSE_GROUP: &[&str] = &[
    ",", "else", "{", ".", "?", "==>", "<==", "<==>", "&&", "||", "==", "!=", "<=", ">=", ">",
    "&&&", "|||", "=~=", "=~~=", "!~=", "!~~=", "===", "+", "/", "%", "implies", "as", "is",
    "matches", "by",
];

/// Tokens after a brace group in a loop's clauses that could continue the
/// clause or start the statement after the loop.
const AFTER_GROUP_AMBIGUOUS: &[&str] = &["&", "*", "-", "|", "<", "(", "[", "!"];

/// Names collected over a whole unit that mark ghost positions the lexer
/// cannot see at their use.
#[derive(Debug, Default)]
pub struct UnitFacts {
    /// Fields declared `ghost`/`tracked` in a struct or enum variant.
    pub ghost_fields: BTreeSet<String>,
    /// Tuple structs and variants with a `ghost`/`tracked` positional field.
    pub ghost_ctors: BTreeSet<String>,
    /// Exec functions with a `ghost`/`tracked` parameter (or a mode
    /// attribute on a parameter).
    pub ghost_param_fns: BTreeSet<String>,
}

/// Why a function was left out, or that it is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Its eligible tokens are known.
    Supported,
    /// `external`/`external_body` (itself or an enclosing item): not
    /// verified, so not a target.
    External,
    /// The mask could not classify something; the reason.
    Unsupported(String),
}

/// A loop header in a supported function.
#[derive(Debug, Clone, Copy)]
pub(super) struct LoopHeader {
    /// Tokens `[from, to)` of the condition or iterator: everything
    /// between the keyword and the first clause keyword or the body.
    pub cond: (usize, usize),
    /// `for` (else `while` or `loop`).
    pub is_for: bool,
}

/// One executable function inside `verus!`.
#[derive(Debug, Clone)]
pub(super) struct ExecFn {
    /// Its name.
    pub name: String,
    /// The `fn` token.
    pub fn_tok: usize,
    /// The end of its `verus!` region.
    pub region_end: usize,
    /// Token indices of the body's braces.
    pub body: (usize, usize),
    /// Supported, external or unsupported.
    pub status: Status,
    /// Loop headers (supported functions only).
    pub loops: Vec<LoopHeader>,
}

/// A file's exec functions and its eligible tokens.
pub(super) struct Analysis<'a> {
    /// The lexed file.
    pub file: File<'a>,
    /// Exec functions with a body, in source order.
    pub fns: Vec<ExecFn>,
    /// Per token: executable code of a supported function.
    pub eligible: Vec<bool>,
}

/// The body braces of the function whose `fn` keyword is token `f`: the
/// first top-level brace group after the parameters that is followed by the
/// start of another item or the end of the enclosing one
/// ([`File::brace_role`]). Earlier groups must visibly continue a clause,
/// or the body is unplaced.
fn body_of(file: &File, f: usize, rb: usize) -> Result<Option<(usize, usize)>, String> {
    let mut i = f + 2;
    if file.is(i, "<") {
        i = file.skip_angles(i, rb);
    }
    if !file.is(i, "(") {
        return Ok(None);
    }
    while i < rb {
        let t = file.text(i);
        if t == ";" {
            return Ok(None);
        }
        if file.is_open(i) {
            let Some(close) = file.partner(i).filter(|&x| x < rb) else {
                return Err("unbalanced signature".to_owned());
            };
            if t == "{" {
                match file.brace_role(i, rb) {
                    Brace::Body(c) => return Ok(Some((i, c))),
                    Brace::Unplaced => return Err("unplaced function body".to_owned()),
                    Brace::Continues => {}
                }
            }
            i = close + 1;
            continue;
        }
        if file.kind(i) == Some(Kind::Punct) && matches!(t, ")" | "]" | "}") {
            return Ok(None);
        }
        i += 1;
    }
    Ok(None)
}

/// Rust keywords that can directly precede a `!(...)` negation, which is
/// not a macro invocation.
const NOT_MACRO: &[&str] = &[
    "if", "while", "return", "match", "in", "else", "break", "let", "move", "mut", "ref", "as",
];

/// For every token, its innermost enclosing open bracket.
fn parents(file: &File) -> Vec<Option<usize>> {
    let mut out = vec![None; file.toks.len()];
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..file.toks.len() {
        if file.kind(i) == Some(Kind::Punct)
            && matches!(file.text(i), ")" | "]" | "}")
            && file.partner(i).is_some()
            && stack.last() == file.partner(i).as_ref()
        {
            stack.pop();
        }
        if let Some(slot) = out.get_mut(i) {
            *slot = stack.last().copied();
        }
        if file.is_open(i) && file.partner(i).is_some() {
            stack.push(i);
        }
    }
    out
}

/// Checks the items enclosing token `f` up to the `verus!` brace
/// `region_open`: each must be an `impl`, `trait` or `mod` body.
fn enclosure(file: &File, parent: &[Option<usize>], f: usize, region_open: usize) -> Status {
    let mut p = parent.get(f).copied().flatten();
    while let Some(open) = p {
        if open == region_open {
            return Status::Supported;
        }
        if !file.is(open, "{") {
            return Status::Unsupported("enclosed by a non-brace group".to_owned());
        }
        if open > 0 && file.is(open - 1, "!") {
            return Status::Unsupported("inside a macro invocation".to_owned());
        }
        let outer = parent.get(open).copied().flatten();
        let lo = outer.map_or(0, |o| o + 1);
        // The item header: back to the previous item's end.
        let mut k = open;
        while k > lo {
            let t = k - 1;
            if file.is(t, ";") || file.is(t, "}") {
                break;
            }
            if file.is(t, ")") || file.is(t, "]") {
                match file.partner(t) {
                    Some(o) if o >= lo => k = o,
                    _ => return Status::Unsupported("unbalanced item header".to_owned()),
                }
                continue;
            }
            k = t;
        }
        let mut item = false;
        let mut i = k;
        while i < open {
            if file.is(i, "#") && file.is(i + 1, "[") {
                let close = file.partner(i + 1).unwrap_or(i + 1);
                let attr = file
                    .src
                    .get(file.start(i)..file.end(close))
                    .unwrap_or_default();
                if attr.contains("external") {
                    return Status::External;
                }
                i = close + 1;
                continue;
            }
            if file.kind(i) == Some(Kind::Ident) && matches!(file.text(i), "impl" | "trait" | "mod")
            {
                item = true;
            }
            i += 1;
        }
        if !item {
            return Status::Unsupported("enclosing item is not impl/trait/mod".to_owned());
        }
        // Inner attributes: `#![verifier::external]`.
        let mut i = open + 1;
        while file.is(i, "#") && file.is(i + 1, "!") && file.is(i + 2, "[") {
            let close = file.partner(i + 2).unwrap_or(i + 2);
            let attr = file
                .src
                .get(file.start(i)..file.end(close))
                .unwrap_or_default();
            if attr.contains("external") {
                return Status::External;
            }
            i = close + 1;
        }
        p = outer;
    }
    Status::Unsupported("not inside the verus! region".to_owned())
}

/// Collects a file's ghost fields, ghost constructors and exec functions
/// with ghost parameters into `facts`.
pub fn collect_facts(src: &str, facts: &mut UnitFacts) {
    let file = File::new(src);
    for (ra, rb) in file.verus_regions() {
        let mut i = ra;
        while i < rb {
            if file.kind(i) != Some(Kind::Ident) {
                i += 1;
                continue;
            }
            match file.text(i) {
                "struct" | "enum" | "union" if file.kind(i + 1) == Some(Kind::Ident) => {
                    // Up to the definition's `;` or its brace group.
                    let mut j = i + 2;
                    while j < rb && !file.is(j, ";") {
                        if file.is_open(j) {
                            let close = file.partner(j).unwrap_or(j).min(rb);
                            type_group(&file, j, close, facts);
                            if file.is(j, "{") {
                                break;
                            }
                            j = close;
                        }
                        j += 1;
                    }
                }
                "fn" if file.kind(i + 1) == Some(Kind::Ident) => {
                    let (mode, _) = file.mode_of(i);
                    let mut j = i + 2;
                    if file.is(j, "<") {
                        j = file.skip_angles(j, rb);
                    }
                    if mode == "exec"
                        && file.is(j, "(")
                        && let Some(close) = file.partner(j)
                    {
                        let ghost = (j + 1..close).any(|t| {
                            (file.kind(t) == Some(Kind::Ident)
                                && matches!(file.text(t), "ghost" | "tracked"))
                                || (file.is(t, "#") && file.is(t + 1, "["))
                        });
                        if ghost {
                            facts.ghost_param_fns.insert(file.text(i + 1).to_owned());
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

/// Records the ghost fields of one struct/enum definition group.
fn type_group(file: &File, open: usize, close: usize, facts: &mut UnitFacts) {
    for t in open + 1..close {
        if !(file.kind(t) == Some(Kind::Ident) && matches!(file.text(t), "ghost" | "tracked")) {
            continue;
        }
        if file.kind(t + 1) == Some(Kind::Ident) && file.is(t + 2, ":") {
            facts.ghost_fields.insert(file.text(t + 1).to_owned());
            continue;
        }
        // A positional field: the constructor is the identifier before the
        // enclosing parenthesis.
        let mut k = t;
        let mut ctor = None;
        while k > 0 {
            k -= 1;
            if file.is(k, ")") || file.is(k, "]") || file.is(k, "}") {
                match file.partner(k) {
                    Some(o) => k = o,
                    None => break,
                }
                continue;
            }
            if file.is(k, "(") {
                if k > 0 && file.kind(k - 1) == Some(Kind::Ident) {
                    ctor = Some(file.text(k - 1).to_owned());
                }
                break;
            }
            if file.is(k, "{") {
                break;
            }
        }
        match ctor {
            Some(c) => {
                facts.ghost_ctors.insert(c);
            }
            // Unplaced: refuse every use of the word as a field.
            None => {
                facts.ghost_fields.insert(file.text(t + 1).to_owned());
            }
        }
    }
}

/// A function body's masked tokens.
struct Mask {
    base: usize,
    bits: Vec<bool>,
}

impl Mask {
    fn set(&mut self, from: usize, to_inclusive: usize) {
        for i in from..=to_inclusive {
            if let Some(b) = i.checked_sub(self.base).and_then(|k| self.bits.get_mut(k)) {
                *b = true;
            }
        }
    }

    fn get(&self, i: usize) -> bool {
        i.checked_sub(self.base)
            .and_then(|k| self.bits.get(k))
            .copied()
            .unwrap_or(true)
    }
}

/// The token after a ghost statement starting at `i`: past its `;`, or past
/// the first brace group after `by`, or at the closer of the enclosing
/// group.
fn ghost_statement_end(file: &File, i: usize, c: usize) -> Result<usize, String> {
    let mut j = i + 1;
    let mut by = false;
    while j < c {
        let t = file.text(j);
        if t == ";" {
            return Ok(j + 1);
        }
        if file.kind(j) == Some(Kind::Punct) && matches!(t, ")" | "]" | "}") {
            return Ok(j);
        }
        if file.kind(j) == Some(Kind::Ident) && t == "by" {
            by = true;
        }
        if file.is_open(j) {
            let brace = t == "{";
            let close = file
                .partner(j)
                .filter(|&x| x < c)
                .ok_or_else(|| "unbalanced bracket in a ghost statement".to_owned())?;
            j = close + 1;
            if brace && by {
                return Ok(if file.is(j, ";") { j + 1 } else { j });
            }
            continue;
        }
        j += 1;
    }
    Ok(c)
}

/// The token after the `Ghost`/`Tracked` form starting at token `i`, before
/// the body's closing brace `c`: the whole path, with every generic argument
/// list and every turbofish (`Ghost<T>`, `Ghost::<T>::new`,
/// `Tracked::method::<U>`), and its argument group when one follows
/// (`Ghost(e)`, `Ghost::<T>::new(e)`, `Tracked::<T>::method(e)`). A leading
/// path (`vstd::prelude::Ghost::new(e)`) does not matter: everything from
/// the `Ghost`/`Tracked` token on is ghost.
///
/// Forms the mask cannot bound refuse the function: a qualified path
/// (`<Ghost<T>>::new(e)`, `<Tracked<T> as Tr>::f(e)`), whose arguments come
/// after the type's closing `>`, and a wrapper followed by a method call,
/// field, index, `?` or brace group (`Tracked(t).borrow()`), whose operands
/// are ghost too.
fn ghost_wrapper_end(file: &File, i: usize, c: usize) -> Result<usize, String> {
    let word = file.text(i);
    if i > 0
        && file.is(i - 1, "<")
        && !(i > 1 && (file.kind(i - 2) == Some(Kind::Ident) || file.is(i - 2, "::")))
    {
        return Err(format!("qualified `{word}` path"));
    }
    let mut j = i + 1;
    if file.is(j, "<") {
        j = file.skip_angles(j, c);
    }
    while file.is(j, "::") {
        if file.is(j + 1, "<") {
            j = file.skip_angles(j + 1, c);
        } else if file.kind(j + 1) == Some(Kind::Ident) {
            j += 2;
        } else {
            return Err(format!("unplaced `{word}` path"));
        }
    }
    if file.is(j, "(") {
        j = file
            .partner(j)
            .filter(|&x| x < c)
            .ok_or_else(|| "unbalanced bracket".to_owned())?
            + 1;
    }
    if j < c && matches!(file.text(j), "." | "[" | "{" | "?") {
        return Err(format!("`{word}` form followed by `{}`", file.text(j)));
    }
    Ok(j)
}

/// Places the header of the loop whose keyword is token `i`: masks its
/// clauses and records its condition.
fn loop_header(
    file: &File,
    i: usize,
    c: usize,
    mask: &mut Mask,
    loops: &mut Vec<LoopHeader>,
) -> Result<(), String> {
    let is_for = file.is(i, "for");
    let closer = |j: usize| {
        file.kind(j) == Some(Kind::Punct) && matches!(file.text(j), ")" | "]" | "}" | ";")
    };
    let group_end = |j: usize| {
        file.partner(j)
            .filter(|&x| x < c)
            .ok_or_else(|| "unbalanced bracket in a loop header".to_owned())
    };
    let mut j = i + 1;
    let mut clause = None;
    loop {
        if j >= c || closer(j) {
            return Err("loop header without a body".to_owned());
        }
        let t = file.text(j);
        if t == "{" {
            break;
        }
        if file.kind(j) == Some(Kind::Ident) {
            if LOOP_CLAUSE_KWS.contains(&t) {
                clause = Some(j);
                break;
            }
            if BRACE_KWS.contains(&t) || t == "move" {
                return Err(format!("`{t}` in a loop condition"));
            }
        }
        if (t == "|" || t == "||") && !file.operand_end(j - 1) {
            return Err("closure in a loop condition".to_owned());
        }
        if file.is_open(j) {
            j = group_end(j)? + 1;
            continue;
        }
        j += 1;
    }
    let cond = (i + 1, j);
    if let Some(k) = clause {
        // The body is the first top-level brace group that nothing
        // continues. A group followed by `,`, an operator or another
        // clause belongs to a clause expression (`forall|x| { .. } ==> p`,
        // `match x { .. },`); a group followed by a token that could either
        // continue it or start a statement refuses the function.
        loop {
            if j >= c || closer(j) {
                return Err("loop clauses without a body".to_owned());
            }
            if file.is(j, "{") {
                let close = group_end(j)?;
                let n = close + 1;
                let next = file.text(n);
                let continues = AFTER_CLAUSE_GROUP.contains(&next)
                    || (file.kind(n) == Some(Kind::Ident) && LOOP_CLAUSE_KWS.contains(&next));
                if !continues {
                    if AFTER_GROUP_AMBIGUOUS.contains(&next) {
                        return Err("ambiguous loop body brace".to_owned());
                    }
                    break;
                }
                j = n;
                continue;
            }
            if file.is_open(j) {
                j = group_end(j)? + 1;
                continue;
            }
            j += 1;
        }
        mask.set(k, j - 1);
    }
    loops.push(LoopHeader { cond, is_for });
    Ok(())
}

/// Masks the ghost constructs of the body whose braces are tokens `o`/`c`.
#[allow(
    clippy::too_many_lines,
    reason = "one scan over the body, construct by construct"
)]
fn mask_body(
    file: &File,
    facts: &UnitFacts,
    o: usize,
    c: usize,
    loops: &mut Vec<LoopHeader>,
) -> Result<Mask, String> {
    let mut mask = Mask {
        base: o,
        bits: vec![false; c - o + 1],
    };
    let group_end = |j: usize| {
        file.partner(j)
            .filter(|&x| x < c)
            .ok_or_else(|| "unbalanced bracket".to_owned())
    };
    let mut i = o + 1;
    while i < c {
        if mask.get(i) {
            i += 1;
            continue;
        }
        let t = file.text(i);
        let after_dot = file.is(i - 1, ".");
        if file.kind(i) == Some(Kind::Ident) {
            let next = file.text(i + 1);
            if next == "!" && file.is_open(i + 2) && !after_dot && !NOT_MACRO.contains(&t) {
                let close = group_end(i + 2)?;
                mask.set(i, close);
                if !(EXEC_MACROS.contains(&t) || GHOST_MACROS.contains(&t)) {
                    return Err(format!("macro `{t}!`"));
                }
                i = close + 1;
                continue;
            }
            match t {
                "proof" => {
                    if next != "{" {
                        return Err("`proof` outside a block".to_owned());
                    }
                    let close = group_end(i + 1)?;
                    mask.set(i, close);
                    i = close + 1;
                    continue;
                }
                _ if (GHOST_STATEMENTS.contains(&t) && !after_dot)
                    || (t == "broadcast" && next == "use")
                    || (t == "let" && matches!(next, "ghost" | "tracked")) =>
                {
                    let end = ghost_statement_end(file, i, c)?;
                    mask.set(i, end - 1);
                    i = end;
                    continue;
                }
                "Ghost" | "Tracked" => {
                    let end = ghost_wrapper_end(file, i, c)?;
                    mask.set(i, end - 1);
                    i = end;
                    continue;
                }
                "while" | "for" | "loop" if !after_dot && (t != "for" || next != "<") => {
                    loop_header(file, i, c, &mut mask, loops)?;
                }
                _ => {}
            }
        } else if t == "#" && (file.is(i + 1, "[") || (file.is(i + 1, "!") && file.is(i + 2, "[")))
        {
            let open = if file.is(i + 1, "[") { i + 1 } else { i + 2 };
            let close = group_end(open)?;
            let attr = file
                .src
                .get(file.start(i)..file.end(close))
                .unwrap_or_default();
            mask.set(i, close);
            if [
                "proof", "spec", "ghost", "tracked", "trigger", "auto", "external",
            ]
            .iter()
            .any(|w| attr.contains(w))
            {
                return Err("mode, trigger or external attribute in the body".to_owned());
            }
            i = close + 1;
            continue;
        }
        i += 1;
    }
    // Tripwires: nothing ghost may remain outside the mask.
    for i in o + 1..c {
        if mask.get(i) {
            continue;
        }
        let t = file.text(i);
        let next = file.text(i + 1);
        match file.kind(i) {
            Some(Kind::Ident) => {
                if TRIP_IDENTS.contains(&t) || (t == "old" && next == "(") {
                    return Err(format!("`{t}` outside the mask"));
                }
                let after_dot = file.is(i - 1, ".");
                if facts.ghost_fields.contains(t)
                    && !after_dot
                    && (next == ":"
                        || (matches!(file.text(i - 1), "{" | ",") && matches!(next, "," | "}")))
                {
                    return Err(format!("ghost field `{t}` in a literal or pattern"));
                }
                if facts.ghost_ctors.contains(t) && next == "(" {
                    return Err(format!("constructor `{t}` with a ghost field"));
                }
                if facts.ghost_param_fns.contains(t) && (next == "(" || next == "::") {
                    return Err(format!("call of `{t}`, which has ghost parameters"));
                }
            }
            Some(Kind::Punct) if TRIP_PUNCTS.contains(&t) => {
                return Err(format!("`{t}` outside the mask"));
            }
            _ => {}
        }
    }
    Ok(mask)
}

/// Finds the exec functions of `src` and their eligible tokens.
#[must_use]
pub(super) fn analyse<'a>(src: &'a str, facts: &UnitFacts) -> Analysis<'a> {
    let file = File::new(src);
    let parent = parents(&file);
    let mut eligible = vec![false; file.toks.len()];
    let mut fns = Vec::new();
    for (ra, rb) in file.verus_regions() {
        let mut i = ra;
        while i < rb {
            if !(file.kind(i) == Some(Kind::Ident) && file.is(i, "fn")) {
                i += 1;
                continue;
            }
            if file.kind(i + 1) != Some(Kind::Ident) {
                i += 1;
                continue;
            }
            let name = file.text(i + 1).to_owned();
            let (mode, external) = file.mode_of(i);
            let refused = |reason: String| ExecFn {
                name: name.clone(),
                fn_tok: i,
                region_end: rb,
                body: (i, i),
                status: if external {
                    Status::External
                } else {
                    Status::Unsupported(reason)
                },
                loops: Vec::new(),
            };
            let (o, c) = match body_of(&file, i, rb) {
                Ok(Some(body)) => body,
                Ok(None) => {
                    i += 1;
                    continue;
                }
                Err(reason) => {
                    if mode == "exec" {
                        fns.push(refused(reason));
                    }
                    i += 1;
                    continue;
                }
            };
            // The catalogue generator's signature walk must place the same
            // body, since the planner reuses its operator classes.
            if mode == "exec" && file.function(i, rb).and_then(|f| f.body) != Some((o, c)) {
                fns.push(refused("unparsed signature".to_owned()));
                i = c + 1;
                continue;
            }
            if mode != "exec" {
                i = c + 1;
                continue;
            }
            let mut loops = Vec::new();
            let mut status = if external {
                Status::External
            } else {
                enclosure(&file, &parent, i, ra - 1)
            };
            if status == Status::Supported {
                // A ghost or tracked return makes the tail expression ghost.
                let ghost_return = (i..o).any(|t| {
                    file.is(t, "->")
                        && file.is(t + 1, "(")
                        && matches!(file.text(t + 2), "tracked" | "ghost")
                });
                if ghost_return {
                    status = Status::Unsupported("ghost or tracked return".to_owned());
                }
            }
            if status == Status::Supported {
                match mask_body(&file, facts, o, c, &mut loops) {
                    Ok(mask) => {
                        for t in o + 1..c {
                            if let Some(e) = eligible.get_mut(t) {
                                *e = !mask.get(t);
                            }
                        }
                    }
                    Err(reason) => {
                        loops.clear();
                        status = Status::Unsupported(reason);
                    }
                }
            }
            fns.push(ExecFn {
                name,
                fn_tok: i,
                region_end: rb,
                body: (o, c),
                status,
                loops,
            });
            i = c + 1;
        }
    }
    Analysis {
        file,
        fns,
        eligible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eligible_text(src: &str, facts: &UnitFacts) -> (Vec<Status>, String) {
        let a = analyse(src, facts);
        let text = (0..a.file.toks.len())
            .filter(|&i| a.eligible.get(i).copied().unwrap_or(false))
            .map(|i| a.file.text(i))
            .collect::<Vec<_>>()
            .join(" ");
        (a.fns.iter().map(|f| f.status.clone()).collect(), text)
    }

    #[test]
    fn masks_every_ghost_construct() {
        let src = "verus! {
fn f(a: u64, g: Ghost<int>) -> (r: u64)
    requires a < 10,
    ensures r == a + 1,
{
    let ghost old_a = a as int;
    let tracked t = make();
    let mut i: u64 = 0;
    let x: Ghost<Seq<int>> = Ghost(Seq::empty());
    while i < a
        invariant
            i <= a,
            forall|k: int| 0 <= k < i ==> k < a,
        decreases a - i,
    {
        proof { lemma(i as int); }
        assert(i < a) by { reveal(foo); };
        assert forall|k: int| k < i implies k < a by { }
        i = i + 1;
    }
    for j in 0..a
        invariant j <= a,
    {
        assume(j < 5);
    }
    broadcast use group_a, group_b;
    assert(i == a) by (nonlinear_arith) requires i >= a, i <= a;
    foo(i, Ghost(old_a + 1), Tracked(t));
    i + 1
}
}";
        let (status, text) = eligible_text(src, &UnitFacts::default());
        assert_eq!(status, [Status::Supported]);
        let braces = "verus! { fn f(a: u64) -> u64 {
            let mut i = 0;
            while i < a
                invariant
                    forall|k: int| { &&& k > 0 &&& k < 9 } ==> k + a > 0,
                    match a { 0 => true, _ => i <= a },
                decreases if a > i { a - i } else { 0 }
            {
                i += 1;
            }
            i } }";
        assert_eq!(
            eligible_text(braces, &UnitFacts::default()).1,
            "let mut i = 0 ; while i < a { i += 1 ; } i"
        );
        assert_eq!(
            text,
            "let mut i : u64 = 0 ; let x : = ; while i < a { i = i + 1 ; } for j in 0 .. a { } \
             foo ( i , , ) ; i + 1"
        );
    }

    #[test]
    fn fails_closed() {
        let refuse = |body: &str, facts: &UnitFacts| {
            let src = format!("verus! {{ fn f(a: u64) -> u64 {{ {body} }} }}");
            match eligible_text(&src, facts).0.first() {
                Some(Status::Unsupported(_)) => true,
                other => {
                    assert!(other.is_some(), "no function in {src}");
                    false
                }
            }
        };
        let none = UnitFacts::default();
        assert!(!refuse("a + 1", &none));
        assert!(refuse("my_macro!(a); a", &none));
        assert!(refuse("let s = a@; a", &none));
        assert!(refuse(
            "let f = |x: u64| -> (r: u64) ensures r == x { x }; a",
            &none
        ));
        assert!(refuse("fn inner() {} a", &none));
        // A struct literal in a clause is placed by what follows it.
        assert!(!refuse(
            "while a < 3 invariant a == S { x: 1 }.x, { } a",
            &none
        ));
        assert!(!refuse(
            "while a < 3 invariant if a > 1 { true } else { false } { } a",
            &none
        ));
        assert!(!refuse(
            "while a < 3 invariant forall|k: int| { k > 0 } ==> k + a > 0, { } a",
            &none
        ));
        assert!(refuse("while a < 3 invariant a < 9 { } *p = 1; a", &none));
        assert!(refuse("open_atomic_invariant!(i => g => { }); a", &none));
        let mut facts = UnitFacts::default();
        collect_facts(
            "verus! { struct S { ghost g: int, v: u64 } struct T(ghost int, u64);
              fn takes(tracked t: P, x: u64) {} }",
            &mut facts,
        );
        assert!(refuse("let s = S { g: a + 1, v: a }; a", &facts));
        assert!(refuse("let s = T(a + 1, a); a", &facts));
        assert!(refuse("takes(p, a + 1); a", &facts));
        assert!(!refuse("let s = U { v: a }; a", &facts));
    }

    /// The eligible text of `body` in a one-function exec body, or `None`
    /// when the function is refused.
    fn eligible_body(body: &str) -> Option<String> {
        let src = format!("verus! {{ fn f(a: u64) -> u64 {{ {body} }} }}");
        let (status, text) = eligible_text(&src, &UnitFacts::default());
        (status == [Status::Supported]).then_some(text)
    }

    #[test]
    fn masks_every_ghost_wrapper_form() {
        let masked = |body: &str, want: &str| {
            assert_eq!(eligible_body(body).as_deref(), Some(want), "{body}");
        };
        // Tuple-struct construction, in calls and as patterns.
        masked("foo(a, Ghost(a + 1), Tracked(t)); a", "foo ( a , , ) ; a");
        masked("let Tracked(p) = t; a", "let = t ; a");
        masked("let Ghost(g) = h; a", "let = h ; a");
        masked("match t { Tracked(p) => a + 1 }", "match t { => a + 1 }");
        // Associated calls, with and without turbofish on the type or the
        // function.
        masked("let g = Ghost::new(a + 1); a", "let g = ; a");
        masked("let g = Ghost::<u64>::new(a + 1); a", "let g = ; a");
        masked("let g = Ghost::new::<u64>(a + 1); a", "let g = ; a");
        masked("let t = Tracked::<P>::method(a + 1); a", "let t = ; a");
        masked(
            "let t = Tracked::<Vec<P>>::method::<u64>(a + 1, a); a",
            "let t = ; a",
        );
        masked("let t = Tracked::assume_new(); a", "let t = ; a");
        // A leading path does not hide the wrapper.
        masked(
            "let g = vstd::prelude::Ghost::new(a + 1); a",
            "let g = vstd :: prelude :: ; a",
        );
        masked(
            "let g = vstd::prelude::Ghost::<u64>::new(a + 1); a",
            "let g = vstd :: prelude :: ; a",
        );
        masked(
            "let t = vstd::prelude::Tracked(a + 1); a",
            "let t = vstd :: prelude :: ; a",
        );
        // Types, including nested in other generics.
        masked("let g: Ghost<Seq<u64>> = h; a", "let g : = h ; a");
        masked("let v: Vec<Tracked<P>> = w; a", "let v : Vec < = w ; a");
        // `>>` closes both lists at once, so the rest of the path is masked
        // too: more than needed, never less.
        masked(
            "let v = Vec::<Ghost<u64>>::new(); a",
            "let v = Vec :: < ; a",
        );
        // Forms the mask cannot bound refuse the function.
        for refused in [
            "let g = <Ghost<u64>>::new(a + 1); a",
            "let g = <Ghost<u64> as From<u64>>::from(a + 1); a",
            "let p = Tracked(t).borrow(a + 1); a",
            "let p = Tracked::<P>::new(t).get(a + 1); a",
            "let p = Ghost(g).0; a",
            "let p = Tracked(t)[a + 1]; a",
            "let p = Tracked(t)?; a",
            "let p = Ghost:: (a + 1); a",
        ] {
            assert_eq!(eligible_body(refused), None, "{refused}");
        }
    }

    #[test]
    fn ghost_patterns_and_attributes() {
        // `let ghost`/`let tracked` statements, whatever their pattern.
        assert_eq!(
            eligible_body("let ghost (x, y) = (a, a + 1); let tracked Q { p } = q; a").as_deref(),
            Some("a")
        );
        // `ghost`/`tracked` in any other pattern position trips the mask.
        for refused in [
            "match o { Some(tracked p) => a, None => a + 1 }",
            "let f = |tracked p: P| a + 1; f(q)",
            "let (ghost x, y) = (a, a + 1); y",
            "let tracked_ok = 1; ghost_fn(ghost a)",
        ] {
            assert_eq!(eligible_body(refused), None, "{refused}");
        }
        // Attributes are masked; ones that name a mode, a trigger or
        // externality refuse the function.
        assert_eq!(
            eligible_body("#[verifier::truncate] let x = (a + 1) as u8; a").as_deref(),
            Some("let x = ( a + 1 ) as u8 ; a")
        );
        assert_eq!(
            eligible_body("#[verifier::loop_isolation(false)] while a < 3 { } a").as_deref(),
            Some("while a < 3 { } a")
        );
        for refused in [
            "#[verifier::proof_block] { lemma(a); } a",
            "#[verifier::proof] let x = a + 1; a",
            "#[verifier::spec] let x = a + 1; a",
            "#[verifier(ghost)] let x = a + 1; a",
            "#[verifier::tracked] let x = a + 1; a",
            "#[verus::internal(proof)] let x = a + 1; a",
            "#[verifier::external] let x = a + 1; a",
            "#[verifier::external_body] let x = a + 1; a",
        ] {
            assert_eq!(eligible_body(refused), None, "{refused}");
        }
    }

    #[test]
    fn reads_modes_and_externality() {
        let src = "verus! {
pub open spec(checked) fn s(a: u64) -> u64 { a + 1 }
proof fn p(a: u64) { }
#[verifier::external_body]
fn e(a: u64) -> u64 { a + 1 }
#[verifier::external]
impl X { fn in_external(a: u64) -> u64 { a + 1 } }
impl Y { pub fn method(&self, a: u64) -> u64 { a + 1 } }
some_macro! { fn in_macro(a: u64) -> u64 { a + 1 } }
fn tracked_ret() -> (tracked r: T) { make() }
fn clause_braces(a: u64) -> (r: Option<u64>)
    ensures
        match r { Some(x) => x > a, None => true },
        if a > 1 { r is Some } else { true },
{ if !(a > 2) { None } else { Some(a + 1) } }
}
fn outside(a: u64) -> u64 { a + 1 }";
        let a = analyse(src, &UnitFacts::default());
        let got: Vec<(&str, bool, bool)> = a
            .fns
            .iter()
            .map(|f| {
                (
                    f.name.as_str(),
                    f.status == Status::Supported,
                    f.status == Status::External,
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                ("e", false, true),
                ("in_external", false, true),
                ("method", true, false),
                ("in_macro", false, false),
                ("tracked_ret", false, false),
                ("clause_braces", true, false),
            ]
        );
        let body = a.fns.last().map_or((0, 0), |f| f.body);
        assert!(a.file.is(body.0 + 1, "if") && a.file.is(body.0 + 2, "!"));
    }
}
