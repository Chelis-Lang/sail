//! Parser-backed symbol renaming (Mariposa's third mutation), for fragility
//! measurement only. **Spike prototype** (v0.3.0 Track B); experimental.
//!
//! ADR 0018 deferred renaming until it could be driven by a lexer rather
//! than by text matching. This module renames, consistently over a whole
//! session:
//! - every symbol the session itself declares: `declare-sort`,
//!   `define-sort`, `declare-const`, `declare-fun`, `define-fun`,
//!   `define-fun-rec`, `define-funs-rec`, `define-const`, and the sorts,
//!   constructors and selectors of `declare-datatype(s)`;
//! - Z3's derived tester `is-<constructor>` for a renamed constructor;
//! - the values of `:qid` and `:skolemid` attributes.
//!
//! **The rule.** One injective map from old names to fresh names is applied
//! to every simple-symbol token of the session whose text is a key of the
//! map, wherever a term, sort or declaration may stand. A bound variable
//! that shares its name with a declared symbol is therefore renamed
//! together with its binder, which is still an α-renaming. Fresh names are
//! checked against every symbol the session mentions, so nothing is
//! captured.
//!
//! **What is never touched.**
//! - Names the session does not declare: Z3's theory symbols, reserved
//!   words, bound variables with names of their own.
//! - `%%location_label%%…` constants (Verus reads them back from models).
//! - `:named` labels, and any symbol that is also used as one.
//! - The value of every attribute other than `:qid`, `:skolemid`,
//!   `:pattern` and `:no-pattern` (patterns are terms and are renamed).
//! - `set-option`, `set-info`, `set-logic`, `get-info`, `get-option` and
//!   `echo` commands, strings, comments, whitespace and every literal.
//!
//! **Fail closed.** A session with a command this module does not model, a
//! datatype declaration in a form it does not model (`par`, the pre-2.6
//! syntax), or a quoted symbol that aliases a renamed name is refused with
//! a [`RenameError`]. The caller reports the run as not measured; it is
//! never replaced by the unrenamed session.
//!
//! Renamed bytes go to a fresh solver and are thrown away, like every other
//! configuration (see [`crate::config`]).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;

use sail_smtlib::lex::{Lexer, Token, TokenKind};

use crate::transcript::Cmd;

/// Why a session was not renamed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameError {
    /// A command does not lex, or is not a parenthesised list.
    Malformed {
        /// Byte offset of the command in the session.
        offset: u64,
    },
    /// A command this module does not model (it might declare symbols).
    UnsupportedCommand(String),
    /// A declaration in a form this module does not model.
    UnsupportedDeclaration {
        /// Byte offset of the command in the session.
        offset: u64,
        /// What was not understood.
        what: &'static str,
    },
    /// A quoted symbol `|x|` names the same symbol as a renamed `x`.
    QuotedAlias(String),
}

impl fmt::Display for RenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { offset } => write!(f, "malformed command at byte {offset}"),
            Self::UnsupportedCommand(head) => write!(f, "unsupported command `{head}`"),
            Self::UnsupportedDeclaration { offset, what } => {
                write!(f, "unsupported declaration at byte {offset}: {what}")
            }
            Self::QuotedAlias(name) => write!(f, "quoted symbol aliases renamed `{name}`"),
        }
    }
}

impl std::error::Error for RenameError {}

/// How fresh names are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Naming {
    /// Every name maps to itself. The rewrite still goes through the whole
    /// token-replacement path, so a byte-identical result checks the spans.
    Identity,
    /// Fresh names `r<16 hex digits>` from a fixed generator seeded by this
    /// value.
    Seeded(u64),
}

/// Which names a plan renames. Anything but [`Scope::All`] is a diagnostic
/// aid for telling apart what a solver is sensitive to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Declared symbols and `:qid`/`:skolemid` values.
    All,
    /// Declared symbols only.
    Declared,
    /// `:qid` values only (and declared symbols of the same name).
    Qids,
    /// `:skolemid` values only (and declared symbols of the same name).
    Skolemids,
}

/// What a plan covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct Stats {
    /// Distinct declared names that are renamed.
    pub declared: usize,
    /// Distinct `:qid`/`:skolemid` values that are renamed (and are not
    /// also declared names).
    pub attribute_names: usize,
    /// Derived `is-<constructor>` testers that are renamed.
    pub testers: usize,
    /// Declared `%%location_label%%` names left alone.
    pub kept_location_labels: usize,
    /// Declared or attribute names left alone because they are builtin or
    /// reserved names.
    pub kept_builtin: usize,
    /// Declared or attribute names left alone because they are also
    /// `:named` labels.
    pub kept_named: usize,
}

/// A renaming of one session.
#[derive(Debug, Clone)]
pub struct Plan {
    map: HashMap<Vec<u8>, Vec<u8>>,
    /// What the plan covers.
    pub stats: Stats,
}

/// Commands whose bytes are copied verbatim.
const OPAQUE: &[&[u8]] = &[
    b"set-option",
    b"set-info",
    b"set-logic",
    b"get-info",
    b"get-option",
    b"echo",
];

/// Commands that declare nothing and whose terms are renamed.
const PLAIN: &[&[u8]] = &[
    b"assert",
    b"check-sat",
    b"check-sat-assuming",
    b"push",
    b"pop",
    b"get-model",
    b"get-value",
    b"eval",
    b"get-unsat-core",
    b"get-proof",
    b"get-assertions",
    b"reset",
    b"reset-assertions",
    b"exit",
    b"simplify",
];

/// Commands whose third token is the declared name.
const SIMPLE_DECLARATIONS: &[&[u8]] = &[
    b"declare-sort",
    b"define-sort",
    b"declare-const",
    b"declare-fun",
    b"define-fun",
    b"define-fun-rec",
    b"define-const",
    b"declare-datatype",
];

/// Names that are never renamed even if a session declares or uses them as
/// attribute values: reserved words and theory symbols of the pinned Z3
/// that a declaration could only overload.
const BUILTIN: &[&[u8]] = &[
    b"!",
    b"_",
    b"as",
    b"let",
    b"forall",
    b"exists",
    b"lambda",
    b"match",
    b"par",
    b"true",
    b"false",
    b"not",
    b"and",
    b"or",
    b"xor",
    b"=>",
    b"=",
    b"distinct",
    b"ite",
    b"Bool",
    b"Int",
    b"Real",
    b"Array",
    b"BitVec",
    b"String",
    b"Seq",
    b"RegLan",
    b"+",
    b"-",
    b"*",
    b"/",
    b"div",
    b"mod",
    b"rem",
    b"abs",
    b"<",
    b"<=",
    b">",
    b">=",
    b"to_real",
    b"to_int",
    b"is_int",
    b"select",
    b"store",
    b"const",
    b"is",
    b"concat",
    b"extract",
    b"zero_extend",
    b"sign_extend",
    b"bv2int",
    b"int2bv",
    b"bv2nat",
    b"nat2bv",
    b"bvnot",
    b"bvneg",
    b"bvand",
    b"bvor",
    b"bvxor",
    b"bvadd",
    b"bvsub",
    b"bvmul",
    b"bvudiv",
    b"bvurem",
    b"bvsdiv",
    b"bvsrem",
    b"bvsmod",
    b"bvshl",
    b"bvlshr",
    b"bvashr",
    b"bvult",
    b"bvule",
    b"bvugt",
    b"bvuge",
    b"bvslt",
    b"bvsle",
    b"bvsgt",
    b"bvsge",
    b"update-field",
    b"partial-order",
    b"linear-order",
    b"tree-order",
    b"piecewise-linear-order",
    b"transitive-closure",
];

const LOCATION_LABEL: &[u8] = b"%%location_label%%";

/// `SplitMix64`, as in [`crate::config`]: fixed and portable.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// A command's tokens and its head symbol.
struct Lexed<'a> {
    src: &'a [u8],
    tokens: Vec<Token>,
    head: &'a [u8],
}

fn lex(cmd: &Cmd) -> Result<Lexed<'_>, RenameError> {
    let malformed = || RenameError::Malformed { offset: cmd.offset };
    let src = cmd.bytes.as_slice();
    let tokens: Vec<Token> = Lexer::new(src)
        .collect::<Result<_, _>>()
        .map_err(|_| malformed())?;
    let (Some(open), Some(head), Some(close)) = (tokens.first(), tokens.get(1), tokens.last())
    else {
        return Err(malformed());
    };
    if open.kind != TokenKind::Open
        || head.kind != TokenKind::Symbol
        || close.kind != TokenKind::Close
    {
        return Err(malformed());
    }
    let head = head.text(src);
    Ok(Lexed { src, tokens, head })
}

/// Names gathered from a session before any exclusion.
#[derive(Default)]
struct Gathered {
    declared: BTreeSet<Vec<u8>>,
    constructors: BTreeSet<Vec<u8>>,
    attributes: BTreeSet<Vec<u8>>,
    qids: HashSet<Vec<u8>>,
    skolemids: HashSet<Vec<u8>>,
    named: HashSet<Vec<u8>>,
    /// Every simple symbol the session mentions.
    symbols: HashSet<Vec<u8>>,
    /// The contents of every quoted symbol outside opaque commands.
    quoted: HashSet<Vec<u8>>,
}

/// Which declared names a datatype-style command introduces, by argument
/// index and by the nesting depth just inside an opening parenthesis.
#[derive(Clone, Copy)]
struct Shape {
    /// `(argument, depth)` whose first symbol is a sort or function name.
    name: Option<(usize, usize)>,
    /// `(argument, depth)` whose first symbol is a constructor.
    constructor: Option<(usize, usize)>,
    /// `(argument, depth)` whose first symbol is a selector.
    selector: Option<(usize, usize)>,
    /// `(argument, depth)` at which a symbol means an unmodelled form.
    refuse: Option<(usize, usize)>,
}

fn shape(head: &[u8]) -> Option<Shape> {
    match head {
        // (declare-datatypes ((T 0) …) (((C (s S) …) …) …))
        b"declare-datatypes" => Some(Shape {
            name: Some((0, 2)),
            constructor: Some((1, 3)),
            selector: Some((1, 4)),
            refuse: Some((1, 2)),
        }),
        // (declare-datatype T ((C (s S) …) …)); T is the simple name.
        b"declare-datatype" => Some(Shape {
            name: None,
            constructor: Some((1, 2)),
            selector: Some((1, 3)),
            refuse: Some((1, 1)),
        }),
        // (define-funs-rec ((f ((x S) …) S) …) (body …))
        b"define-funs-rec" => Some(Shape {
            name: Some((0, 2)),
            constructor: None,
            selector: None,
            refuse: None,
        }),
        _ => None,
    }
}

fn gather_shaped(
    l: &Lexed<'_>,
    s: Shape,
    offset: u64,
    g: &mut Gathered,
) -> Result<(), RenameError> {
    let mut depth = 0usize;
    let mut argument = 0usize;
    let mut started = false;
    let mut after_open = false;
    let body = l
        .tokens
        .get(2..l.tokens.len().saturating_sub(1))
        .unwrap_or_default();
    // For declare-datatype the first argument is the name itself.
    for t in body {
        if depth == 0 {
            if started {
                argument += 1;
            }
            started = true;
        }
        match t.kind {
            TokenKind::Open => {
                depth += 1;
                after_open = true;
                continue;
            }
            TokenKind::Close => depth = depth.saturating_sub(1),
            TokenKind::Symbol | TokenKind::QuotedSymbol => {
                let at = Some((argument, depth));
                if s.refuse == at {
                    return Err(RenameError::UnsupportedDeclaration {
                        offset,
                        what: "parametric, pre-2.6 or bare-constructor datatype",
                    });
                }
                if after_open && (s.name == at || s.constructor == at || s.selector == at) {
                    if t.kind != TokenKind::Symbol {
                        return Err(RenameError::UnsupportedDeclaration {
                            offset,
                            what: "quoted declared name",
                        });
                    }
                    let name = t.text(l.src).to_vec();
                    if s.constructor == at {
                        g.constructors.insert(name.clone());
                    }
                    g.declared.insert(name);
                }
            }
            _ => {}
        }
        after_open = false;
    }
    Ok(())
}

fn gather(commands: &[Cmd]) -> Result<Gathered, RenameError> {
    let mut g = Gathered::default();
    for cmd in commands {
        let l = lex(cmd)?;
        for t in &l.tokens {
            if t.kind == TokenKind::Symbol {
                let text = t.text(l.src);
                if !g.symbols.contains(text) {
                    g.symbols.insert(text.to_vec());
                }
            }
        }
        if OPAQUE.contains(&l.head) {
            continue;
        }
        if SIMPLE_DECLARATIONS.contains(&l.head) {
            match l.tokens.get(2) {
                Some(t) if t.kind == TokenKind::Symbol => {
                    g.declared.insert(t.text(l.src).to_vec());
                }
                _ => {
                    return Err(RenameError::UnsupportedDeclaration {
                        offset: cmd.offset,
                        what: "declared name is not a simple symbol",
                    });
                }
            }
        }
        if let Some(s) = shape(l.head) {
            gather_shaped(&l, s, cmd.offset, &mut g)?;
        } else if !SIMPLE_DECLARATIONS.contains(&l.head) && !PLAIN.contains(&l.head) {
            return Err(RenameError::UnsupportedCommand(
                String::from_utf8_lossy(l.head).into_owned(),
            ));
        }
        for pair in l.tokens.windows(2) {
            let [k, v] = pair else { continue };
            if v.kind == TokenKind::QuotedSymbol {
                let text = v.text(l.src);
                if let Some(inner) = text.get(1..text.len().saturating_sub(1)) {
                    g.quoted.insert(inner.to_vec());
                }
            }
            if k.kind != TokenKind::Keyword || v.kind != TokenKind::Symbol {
                continue;
            }
            match k.text(l.src) {
                b":qid" => {
                    g.attributes.insert(v.text(l.src).to_vec());
                    g.qids.insert(v.text(l.src).to_vec());
                }
                b":skolemid" => {
                    g.attributes.insert(v.text(l.src).to_vec());
                    g.skolemids.insert(v.text(l.src).to_vec());
                }
                b":named" => {
                    g.named.insert(v.text(l.src).to_vec());
                }
                _ => {}
            }
        }
    }
    Ok(g)
}

/// Plans a renaming of the session split into `commands`.
///
/// # Errors
///
/// [`RenameError`] if the session cannot be renamed safely; see the module
/// documentation.
pub fn plan(commands: &[Cmd], naming: Naming) -> Result<Plan, RenameError> {
    plan_scoped(commands, naming, Scope::All)
}

/// [`plan`], restricted to the names `scope` selects.
///
/// # Errors
///
/// As [`plan`].
pub fn plan_scoped(commands: &[Cmd], naming: Naming, scope: Scope) -> Result<Plan, RenameError> {
    plan_filtered(commands, naming, scope, None)
}

/// [`plan_scoped`], further restricted to the names in `only` when given
/// (a diagnostic aid for bisecting a solver's sensitivity).
///
/// # Errors
///
/// As [`plan`].
pub fn plan_filtered(
    commands: &[Cmd],
    naming: Naming,
    scope: Scope,
    only: Option<&HashSet<Vec<u8>>>,
) -> Result<Plan, RenameError> {
    let g = gather(commands)?;
    let mut stats = Stats::default();
    let mut names: Vec<(&Vec<u8>, bool)> = Vec::new();
    for name in g
        .declared
        .iter()
        .chain(g.attributes.difference(&g.declared))
    {
        let declared = g.declared.contains(name);
        let selected = match scope {
            Scope::All => true,
            Scope::Declared => declared,
            Scope::Qids => g.qids.contains(name),
            Scope::Skolemids => g.skolemids.contains(name),
        };
        if !selected || only.is_some_and(|only| !only.contains(name)) {
            continue;
        }
        if name.starts_with(LOCATION_LABEL) {
            stats.kept_location_labels += 1;
        } else if BUILTIN.contains(&name.as_slice()) {
            stats.kept_builtin += 1;
        } else if g.named.contains(name) {
            stats.kept_named += 1;
        } else {
            if g.quoted.contains(name) {
                return Err(RenameError::QuotedAlias(
                    String::from_utf8_lossy(name).into_owned(),
                ));
            }
            if declared {
                stats.declared += 1;
            } else {
                stats.attribute_names += 1;
            }
            names.push((name, g.constructors.contains(name)));
        }
    }
    let mut rng = match naming {
        Naming::Identity => None,
        Naming::Seeded(seed) => Some(SplitMix64(seed ^ 0x5341_494C_524E_414D)), // "SAILRNAM"
    };
    let mut map: HashMap<Vec<u8>, Vec<u8>> = HashMap::with_capacity(names.len());
    let mut used: HashSet<Vec<u8>> = HashSet::with_capacity(names.len());
    for (name, constructor) in names {
        let fresh = match &mut rng {
            None => name.clone(),
            Some(rng) => loop {
                let candidate = format!("r{:016x}", rng.next()).into_bytes();
                let tester = [b"is-".as_slice(), &candidate].concat();
                if !g.symbols.contains(&candidate)
                    && !g.quoted.contains(&candidate)
                    && !g.symbols.contains(&tester)
                    && !used.contains(&candidate)
                {
                    break candidate;
                }
            },
        };
        used.insert(fresh.clone());
        if constructor {
            let tester = [b"is-".as_slice(), name].concat();
            if g.symbols.contains(&tester) && !g.declared.contains(&tester) {
                if g.quoted.contains(&tester) {
                    return Err(RenameError::QuotedAlias(
                        String::from_utf8_lossy(&tester).into_owned(),
                    ));
                }
                stats.testers += 1;
                map.insert(tester, [b"is-".as_slice(), &fresh].concat());
            }
        }
        map.insert(name.clone(), fresh);
    }
    Ok(Plan { map, stats })
}

impl Plan {
    /// The new name of `old`, if the plan renames it.
    #[must_use]
    pub fn get(&self, old: &[u8]) -> Option<&[u8]> {
        self.map.get(old).map(Vec::as_slice)
    }

    /// The number of names the plan renames.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the plan renames nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Rewrites `input` (split into `commands`, the same ones the plan was
    /// made from). Returns the new bytes and the number of tokens replaced.
    /// Every byte that is not a replaced token is copied from `input`.
    ///
    /// # Errors
    ///
    /// [`RenameError::Malformed`] if a command does not lex.
    pub fn rewrite(&self, input: &[u8], commands: &[Cmd]) -> Result<(Vec<u8>, usize), RenameError> {
        let mut out = Vec::with_capacity(input.len() + input.len() / 8);
        let mut replaced = 0usize;
        let mut cursor = 0usize;
        for cmd in commands {
            let at = usize::try_from(cmd.offset).unwrap_or(usize::MAX);
            out.extend_from_slice(input.get(cursor..at).unwrap_or_default());
            cursor = at.saturating_add(cmd.bytes.len());
            let l = lex(cmd)?;
            if OPAQUE.contains(&l.head) {
                out.extend_from_slice(l.src);
                continue;
            }
            replaced += self.rewrite_command(&l, &mut out);
        }
        out.extend_from_slice(input.get(cursor..).unwrap_or_default());
        Ok((out, replaced))
    }

    fn rewrite_command(&self, l: &Lexed<'_>, out: &mut Vec<u8>) -> usize {
        let mut replaced = 0usize;
        let mut pos = 0usize;
        // Tokens up to (not including) this index are an attribute value
        // that is copied verbatim.
        let mut verbatim_until = 0usize;
        // The head (index 1) is never a renamable symbol.
        for (i, t) in l.tokens.iter().enumerate().skip(2) {
            if i < verbatim_until {
                continue;
            }
            match t.kind {
                TokenKind::Keyword => {
                    let renamed_value = matches!(
                        t.text(l.src),
                        b":qid" | b":skolemid" | b":pattern" | b":no-pattern"
                    );
                    if !renamed_value {
                        verbatim_until = value_end(&l.tokens, i + 1);
                    }
                }
                TokenKind::Symbol => {
                    if let Some(new) = self.map.get(t.text(l.src)) {
                        out.extend_from_slice(l.src.get(pos..t.start).unwrap_or_default());
                        out.extend_from_slice(new);
                        pos = t.end;
                        replaced += 1;
                    }
                }
                _ => {}
            }
        }
        out.extend_from_slice(l.src.get(pos..).unwrap_or_default());
        replaced
    }
}

/// The index one past the attribute value that starts at `start`: one atom,
/// one balanced list, or nothing if `start` is a keyword or a `)`.
fn value_end(tokens: &[Token], start: usize) -> usize {
    match tokens.get(start).map(|t| t.kind) {
        None | Some(TokenKind::Keyword | TokenKind::Close) => start,
        Some(TokenKind::Open) => {
            let mut depth = 0usize;
            for (i, t) in tokens.iter().enumerate().skip(start) {
                match t.kind {
                    TokenKind::Open => depth += 1,
                    TokenKind::Close => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return i + 1;
                        }
                    }
                    _ => {}
                }
            }
            tokens.len()
        }
        Some(_) => start + 1,
    }
}

/// Renames the session `input` (split into `commands`) with fresh names
/// drawn from `seed`.
///
/// # Errors
///
/// [`RenameError`] if the session cannot be renamed safely.
pub fn rename(input: &[u8], commands: &[Cmd], seed: u64) -> Result<Vec<u8>, RenameError> {
    let plan = plan(commands, Naming::Seeded(seed))?;
    plan.rewrite(input, commands).map(|(bytes, _)| bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::split_commands;

    const SESSION: &[u8] = b"(set-option :auto_config false)\n\
(declare-sort Poly 0) ; a comment mentioning Poly\n\
(declare-datatypes ((opt 0) (pair 0)) (((none) (some (val Poly))) ((mk (fst Int) (snd opt)))))\n\
(declare-fun f (Poly Int) Bool)\n\
(declare-const x! Poly)\n\
(declare-const %%location_label%%0 Bool)\n\
(define-fun g ((x! Int)) Int (+ x! 1))\n\
(assert (forall ((x! Poly) (i Int)) (!\n\
   (=> (f x! i) (is-some (some x!)))\n\
   :pattern ((f x! i))\n\
   :qid user_f_0\n\
   :skolemid skolem_user_f_0\n\
)))\n\
(assert (! (f x! 0) :named f_holds))\n\
(assert (=> %%location_label%%0 (and f_holds ((_ is none) none) (= (g 1) (fst (mk 2 none))))))\n\
(echo \"f x! Poly\")\n\
(set-option :rlimit 30000000)\n\
(check-sat)\n\
(get-info :reason-unknown)\n\
(get-model)\n";

    fn text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    #[test]
    fn identity_is_byte_identical_and_replaces_tokens() -> Result<(), RenameError> {
        let commands = split_commands(SESSION);
        let plan = plan(&commands, Naming::Identity)?;
        let (out, replaced) = plan.rewrite(SESSION, &commands)?;
        assert_eq!(text(&out), text(SESSION));
        assert!(replaced > 20, "{replaced}");
        assert_eq!(plan.stats.kept_location_labels, 1);
        assert_eq!(plan.stats.testers, 1);
        assert_eq!(plan.stats.attribute_names, 2);
        // Poly opt pair none some val mk fst snd f x! g
        assert_eq!(plan.stats.declared, 12);
        Ok(())
    }

    #[test]
    fn renames_declared_symbols_and_nothing_else() -> Result<(), RenameError> {
        let commands = split_commands(SESSION);
        let plan = plan(&commands, Naming::Seeded(1))?;
        let (out, _) = plan.rewrite(SESSION, &commands)?;
        let out = text(&out);
        let new = |old: &str| text(plan.get(old.as_bytes()).unwrap_or(b"<not renamed>"));
        // Untouched: options, comment, echo, labels, named label, builtins,
        // the undeclared bound variable.
        for kept in [
            "(set-option :auto_config false)",
            "; a comment mentioning Poly",
            "(echo \"f x! Poly\")",
            "%%location_label%%0",
            ":named f_holds",
            "(and f_holds ((_ is ",
            "(i Int)",
            "(get-info :reason-unknown)",
        ] {
            assert!(out.contains(kept), "{kept} in {out}");
        }
        for gone in [
            " Poly ", "(opt ", " opt)", "some", " f ", "x!", "user_f_0", "(g ", "fst", "is-some",
        ] {
            let body: String = out
                .lines()
                .filter(|l| !l.contains("echo") && !l.contains("comment"))
                .collect();
            assert!(!body.contains(gone), "{gone} in {body}");
        }
        assert!(out.contains(&format!(":qid {}", new("user_f_0"))));
        assert!(out.contains(&format!(":skolemid {}", new("skolem_user_f_0"))));
        assert!(out.contains(&format!("(is-{} ({} ", new("some"), new("some"))));
        assert_eq!(new("is-some"), format!("is-{}", new("some")));
        // The bound variable that shares a declared name moves with it.
        assert!(out.contains(&format!(
            "(define-fun {} (({} Int)) Int (+ {} 1))",
            new("g"),
            new("x!"),
            new("x!")
        )));
        // Injective, deterministic, seed-dependent.
        let values: HashSet<&Vec<u8>> = plan.map.values().collect();
        assert_eq!(values.len(), plan.len());
        assert_eq!(rename(SESSION, &commands, 1)?, out.as_bytes());
        assert_ne!(rename(SESSION, &commands, 2)?, out.as_bytes());
        // Same command structure.
        assert_eq!(split_commands(out.as_bytes()).len(), commands.len());
        Ok(())
    }

    #[test]
    fn refuses_what_it_does_not_model() {
        for (session, expected) in [
            ("(declare-var x Int)", "unsupported command `declare-var`"),
            (
                "(declare-datatypes () ((T (A) (B))))",
                "unsupported declaration at byte 0",
            ),
            (
                "(declare-datatypes ((L 1)) ((par (T) ((nil) (cons (hd T))))))",
                "unsupported declaration at byte 0",
            ),
            (
                "(declare-const |a b| Int)",
                "unsupported declaration at byte 0",
            ),
            (
                "(declare-const a Int)(assert (> |a| 0))",
                "quoted symbol aliases renamed `a`",
            ),
        ] {
            let commands = split_commands(session.as_bytes());
            let got = plan(&commands, Naming::Seeded(1)).map(|p| p.len());
            match got {
                Err(e) => assert!(e.to_string().starts_with(expected), "{session}: {e}"),
                Ok(n) => unreachable!("{session}: renamed {n}"),
            }
        }
    }

    #[test]
    fn toy_fixture_identity_and_fresh_names() -> Result<(), RenameError> {
        let input =
            include_bytes!("../resources/crates/sail-smtlib/tests/fixtures/verus-toy.in.smt2");
        let commands = split_commands(input);
        let identity = plan(&commands, Naming::Identity)?;
        let (same, replaced) = identity.rewrite(input, &commands)?;
        assert_eq!(same.as_slice(), input.as_slice());
        assert!(replaced > 1000, "{replaced}");
        let renamed = rename(input, &commands, 3)?;
        assert_ne!(renamed.as_slice(), input.as_slice());
        let renamed = text(&renamed);
        assert!(renamed.contains("%%location_label%%0"));
        assert!(!renamed.contains("fuel_defaults"));
        assert!(!renamed.contains("has_type"));
        assert_eq!(split_commands(renamed.as_bytes()).len(), commands.len());
        Ok(())
    }
}
