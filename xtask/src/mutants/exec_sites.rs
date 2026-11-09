//! Spike prototype: exec-only mutation sites and `cargo xtask exec-mutants`.
//!
//! The sites of a file are the catalogue generator's body classes
//! (`arith`, `compare`, `constant`, `logic`, `statement`, `rare-region`)
//! restricted to tokens the exec mask ([`super::exec_mask`]) makes
//! eligible, plus two new classes:
//!
//! - `loop-bound`: a comparison in a `while` condition flipped between
//!   strict and non-strict, or a `for` range switched between `..` and
//!   `..=`;
//! - `off-by-one`: ` + 1` or ` - 1` appended to an index expression, to a
//!   `for` range's end, or to a single-comparison `while` condition.
//!
//! Every site is re-lexed after it is applied: all tokens outside the
//! replaced range must be unchanged, or the site is dropped.

#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::struct_field_names,
    clippy::nonminimal_bool,
    reason = "spike prototype: token cursors and single-pass scans; tidy when it moves to sail-verus"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use super::exec_mask::{Analysis, ExecFn, Status, UnitFacts, analyse, collect_facts};
use super::lexer::{Kind, Tok, lex};
use super::sites::{self, Site};
use crate::Result;

/// The exec classes, in report order.
pub const EXEC_CLASSES: &[&str] = &[
    "arith",
    "compare",
    "constant",
    "logic",
    "statement",
    "rare-region",
    "loop-bound",
    "off-by-one",
];

/// One file's exec functions and sites.
pub struct FilePlan {
    /// `(name, status, 1-based line of `fn`, byte of the body's `{`)` per
    /// exec function.
    pub fns: Vec<(String, Status, usize, usize)>,
    /// The sites, each with the index of its function in `fns`.
    pub sites: Vec<(usize, Site)>,
    /// Sites dropped because re-lexing the mutant changed other tokens.
    pub dropped_relex: usize,
    /// Functions of any mode whose body the catalogue generator misplaces.
    pub legacy_body_mismatches: usize,
}

/// Whether applying `s` to `src` leaves every token outside the replaced
/// range unchanged.
fn preserved(src: &str, toks: &[Tok], s: &Site) -> bool {
    let Some(before) = src.get(..s.start) else {
        return false;
    };
    let Some(after) = src.get(s.end..) else {
        return false;
    };
    let mutated = format!("{before}{}{after}", s.replacement);
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

/// The exec sites of `src`.
#[allow(
    clippy::too_many_lines,
    reason = "the legacy classes, then each new operator, in one place"
)]
pub fn plan(src: &str, facts: &UnitFacts, key: &dyn Fn(&str, usize) -> u64) -> FilePlan {
    let Analysis {
        file,
        fns,
        eligible,
        legacy_body_mismatches,
    } = analyse(src, facts);
    let ok = |i: usize| eligible.get(i).copied().unwrap_or(false);
    let supported: Vec<(usize, &ExecFn)> = fns
        .iter()
        .enumerate()
        .filter(|(_, f)| f.status == Status::Supported)
        .collect();
    let mut out: Vec<(usize, Site)> = Vec::new();
    // The catalogue generator's body classes over the placed body, masked.
    for &(n, f) in &supported {
        let Some(mut func) = file.function(f.fn_tok, f.region_end) else {
            continue;
        };
        func.body = Some(f.body);
        let mut found = Vec::new();
        sites::sites_of(&file, &func, key, &mut found);
        for mut s in found {
            if s.kind != "body" || !EXEC_CLASSES.contains(&s.class) {
                continue;
            }
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
            } else if first >= last || !(first..last).all(ok) {
                continue;
            }
            if s.class == "compare"
                && f.loops
                    .iter()
                    .any(|l| !l.is_for && (l.cond.0..l.cond.1).contains(&first))
            {
                s.class = "loop-bound";
            }
            s.mode = "exec";
            out.push((n, s));
        }
    }
    let new_site =
        |class: &'static str, f: &ExecFn, at: (usize, usize), r: &str, what: String| Site {
            class,
            function: f.name.clone(),
            mode: "exec",
            kind: "body",
            start: at.0,
            end: at.1,
            replacement: r.to_owned(),
            what,
        };
    let span = |a: usize, b: usize| {
        file.src
            .get(file.start(a)..file.end(b))
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    for &(n, f) in &supported {
        // Loop headers.
        for l in &f.loops {
            let (a, b) = l.cond;
            if a >= b || !(a..b).all(ok) {
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
                    out.push((
                        n,
                        new_site(
                            "loop-bound",
                            f,
                            (file.start(i), file.end(i)),
                            r,
                            format!("`{t}` to `{r}` in `for {}`", span(a, b - 1)),
                        ),
                    ));
                }
                if let [r] = ranges.as_slice()
                    && r + 1 < b
                {
                    for delta in [" + 1", " - 1"] {
                        out.push((
                            n,
                            new_site(
                                "off-by-one",
                                f,
                                (last_end, last_end),
                                delta,
                                format!("range end `{}` to `{0}{delta}`", span(r + 1, b - 1)),
                            ),
                        ));
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
                        out.push((
                            n,
                            new_site(
                                "off-by-one",
                                f,
                                (last_end, last_end),
                                delta,
                                format!("loop condition `{}` to `{0}{delta}`", span(a, b - 1)),
                            ),
                        ));
                    }
                }
            }
        }
        // Index expressions.
        let (o, c) = f.body;
        for i in o + 2..c {
            if !(file.is(i, "[") && ok(i) && ok(i - 1) && file.operand_end(i - 1)) {
                continue;
            }
            if matches!(file.text(i - 1), "as" | "mut" | "dyn" | "impl" | "const") {
                continue;
            }
            let Some(close) = file.partner(i) else {
                continue;
            };
            if close <= i + 1 || close >= c || !(i..=close).all(ok) {
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
                out.push((
                    n,
                    new_site(
                        "off-by-one",
                        f,
                        (at, at),
                        delta,
                        format!("index `{}` to `{0}{delta}`", span(i + 1, close - 1)),
                    ),
                ));
            }
        }
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
            .map(|f| {
                (
                    f.name.clone(),
                    f.status.clone(),
                    line(f.fn_tok),
                    file.start(f.body.0),
                )
            })
            .collect(),
        sites: out,
        dropped_relex,
        legacy_body_mismatches,
    }
}

const USAGE: &str = "usage: cargo xtask exec-mutants --project NAME [--unit UNIT] [--source DIR]
                        [--seed N] [--sample N] [--audit FILE] [--functions FILE]
                        [--sites FILE]

  Reports the exec functions of one corpus unit, the share the exec mask
  supports, and the exec-only mutation sites per class.
  --sample N     write N sites for manual audit (round-robin over the
                 classes, each class in seeded hash order) to --audit
  --functions    write one line per exec function: file, line, name, status
  --sites        write one line per site: location, class, function, replaced
                 text, replacement, then what introduces each enclosing
                 bracket and what precedes the site in its statement";

/// For the audit: what introduces each bracket enclosing byte `p`, from the
/// function body inwards, and last what precedes `p` in its own statement.
/// Each entry is the text from the previous statement boundary to the
/// bracket (or to `p`). It is computed from bracket matching alone, not from
/// the mask, and it over-collects when a boundary is unclear, so that a
/// checker reading it errs towards flagging.
fn chain(
    src: &str,
    toks: &[Tok],
    pair: &[Option<usize>],
    body_open: usize,
    p: usize,
) -> Vec<String> {
    let text = |i: usize| {
        toks.get(i)
            .and_then(|t| src.get(t.start..t.end))
            .unwrap_or_default()
    };
    let mut stack: Vec<usize> = Vec::new();
    let mut site = toks.len();
    for (i, t) in toks.iter().enumerate().skip(body_open + 1) {
        if t.start >= p {
            site = i;
            break;
        }
        match pair.get(i).copied().flatten() {
            Some(c) if c > i => stack.push(i),
            Some(_) => {
                stack.pop();
            }
            None => {}
        }
    }
    // The first token of the statement or item that holds token `at`.
    let head_start = |at: usize, lo: usize| {
        let mut k = at;
        while k > lo + 1 {
            let t = k - 1;
            let tt = text(t);
            if tt == ";" || matches!(tt, "(" | "[" | "{") {
                break;
            }
            if matches!(tt, ")" | "]" | "}") {
                // A block that plainly ends a statement is a boundary;
                // anything else is skipped as part of this statement.
                let next = text(k);
                let next_is_word = next
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_alphabetic() || ch == '_');
                let closes_proof = pair
                    .get(t)
                    .copied()
                    .flatten()
                    .is_some_and(|o| o > 0 && text(o - 1) == "proof");
                if tt == "}" && closes_proof {
                    break;
                }
                if tt == "}"
                    && ((next_is_word
                        && !matches!(
                            next,
                            "else"
                                | "as"
                                | "is"
                                | "matches"
                                | "implies"
                                | "by"
                                | "invariant"
                                | "invariant_except_break"
                                | "invariant_ensures"
                                | "ensures"
                                | "decreases"
                                | "requires"
                        ))
                        || next == "#")
                {
                    break;
                }
                match pair.get(t).copied().flatten() {
                    Some(o) if o > lo => {
                        k = o;
                        continue;
                    }
                    _ => break,
                }
            }
            k = t;
        }
        k
    };
    // Tokens `[from, to)`, with every complete bracket group inside
    // shown as its brackets only.
    let span = |from: usize, to: usize| {
        let mut words: Vec<&str> = Vec::new();
        let mut i = from;
        while i < to {
            words.push(text(i));
            match pair.get(i).copied().flatten() {
                Some(c) if c > i && c < to => {
                    words.push("…");
                    i = c;
                }
                _ => i += 1,
            }
        }
        words.join(" ")
    };
    let mut out = Vec::new();
    let mut lo = body_open;
    for open in stack {
        let from = head_start(open, lo);
        // With the token after the bracket's close, which tells a loop
        // body from a brace group inside a clause.
        let after = pair
            .get(open)
            .copied()
            .flatten()
            .map_or("", |c| text(c + 1));
        out.push(format!("{} ⟹ {after}", span(from, open + 1)));
        lo = open;
    }
    let from = head_start(site, lo);
    out.push(span(from, site));
    out
}

/// A site with its seeded order, file and function `(name, line, body byte)`.
type Picked<'a> = (u64, &'a String, (String, usize, usize), Site);

/// Entry point.
///
/// # Errors
///
/// A message for bad arguments, a missing source tree or unwritable output.
#[allow(
    clippy::too_many_lines,
    reason = "argument parsing, the scan and the three reports of a spike tool"
)]
pub fn run(args: &[String]) -> Result<()> {
    let mut a = super::Args {
        project: String::new(),
        unit: None,
        seed: 20_261_109,
        per_class: 1,
        source: None,
        name: None,
        out: PathBuf::new(),
        file: None,
    };
    let mut sample = 0usize;
    let mut audit: Option<PathBuf> = None;
    let mut functions: Option<PathBuf> = None;
    let mut sites_out: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--project" => a.project = value()?,
            "--unit" => a.unit = Some(value()?),
            "--source" => a.source = Some(value()?.into()),
            "--seed" => {
                a.seed = value()?
                    .parse()
                    .map_err(|_| "--seed needs an unsigned integer".to_owned())?;
            }
            "--sample" => {
                sample = value()?
                    .parse()
                    .map_err(|_| "--sample needs a number".to_owned())?;
            }
            "--audit" => audit = Some(value()?.into()),
            "--functions" => functions = Some(value()?.into()),
            "--sites" => sites_out = Some(value()?.into()),
            _ => return Err(format!("unknown argument `{arg}`\n{USAGE}")),
        }
    }
    if a.project.is_empty() {
        return Err(USAGE.to_owned());
    }
    let t = super::corpus_target(&a)?;
    let files = super::crate_files(&t.source, &t.crate_root);
    let mut texts = BTreeMap::new();
    let mut facts = UnitFacts::default();
    for file in &files {
        let path = t.source.join(file);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        collect_facts(&text, &mut facts);
        texts.insert(file.clone(), text);
    }
    let mut total = 0usize;
    let mut external = 0usize;
    let mut supported = 0usize;
    let mut with_sites = 0usize;
    let mut dropped = 0usize;
    let mut legacy = 0usize;
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_class: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut all: Vec<Picked> = Vec::new();
    let mut fn_lines = String::new();
    for (file, text) in &texts {
        let key = |label: &str, offset: usize| {
            super::hash(a.seed, &[&t.project, file, label, &offset.to_string()])
        };
        let p = plan(text, &facts, &key);
        dropped += p.dropped_relex;
        legacy += p.legacy_body_mismatches;
        let mut has: BTreeSet<usize> = BTreeSet::new();
        for (n, s) in p.sites {
            has.insert(n);
            *per_class.entry(s.class).or_insert(0) += 1;
            let order = super::hash(
                a.seed,
                &[
                    &t.project,
                    file,
                    s.class,
                    &s.start.to_string(),
                    &s.replacement,
                ],
            );
            let (name, _, line, body) = p
                .fns
                .get(n)
                .cloned()
                .unwrap_or_else(|| (String::new(), Status::Supported, 0, 0));
            all.push((order, file, (name, line, body), s));
        }
        for (n, (name, status, line, _)) in p.fns.iter().enumerate() {
            total += 1;
            let label = match status {
                Status::Supported => {
                    supported += 1;
                    if has.contains(&n) {
                        with_sites += 1;
                    }
                    "supported".to_owned()
                }
                Status::External => {
                    external += 1;
                    "external".to_owned()
                }
                Status::Unsupported(r) => {
                    // Group reasons by their kind, not their operand.
                    let kind = r.split('`').next().unwrap_or(r).trim().to_owned();
                    let kind = if kind.is_empty() {
                        let rest: Vec<&str> = r.split('`').collect();
                        format!(
                            "`{}`{}",
                            rest.get(1).copied().unwrap_or_default(),
                            rest.get(2).copied().unwrap_or_default()
                        )
                    } else if r.starts_with("macro") {
                        r.clone()
                    } else {
                        kind
                    };
                    *reasons.entry(kind).or_insert(0) += 1;
                    format!("unsupported: {r}")
                }
            };
            let _ = writeln!(fn_lines, "{file}:{line}\t{name}\t{label}");
        }
    }
    let verified = total - external;
    #[allow(
        clippy::cast_precision_loss,
        reason = "function counts are far below 2^52"
    )]
    let pct = |n: usize, d: usize| {
        if d == 0 {
            0.0
        } else {
            100.0 * n as f64 / d as f64
        }
    };
    println!(
        "{}: {} files\nexec functions with a body in verus!: {total} ({external} external, {verified} verified)\n\
         supported: {supported} of {verified} ({:.1}%); with at least one site: {with_sites} ({:.1}%)\n\
         sites: {} ({}); dropped by the re-lex check: {dropped}\n\
         functions (any mode) whose body the catalogue generator misplaces: {legacy}",
        t.unit,
        files.len(),
        pct(supported, verified),
        pct(with_sites, verified),
        all.len(),
        EXEC_CLASSES
            .iter()
            .map(|c| format!("{c} {}", per_class.get(c).unwrap_or(&0)))
            .collect::<Vec<_>>()
            .join(", "),
    );
    let mut by_count: Vec<(&String, &usize)> = reasons.iter().collect();
    by_count.sort_by(|x, y| y.1.cmp(x.1).then_with(|| x.0.cmp(y.0)));
    println!("unsupported reasons:");
    for (r, n) in by_count {
        println!("  {n:5}  {r}");
    }
    if let Some(path) = functions {
        std::fs::write(&path, fn_lines)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    if let Some(path) = sites_out {
        // Every site with its context, for a mechanical cross-check.
        let mut lexed: BTreeMap<&String, (Vec<Tok>, Vec<Option<usize>>)> = BTreeMap::new();
        let mut text = String::new();
        for (_, file, (name, _, body), s) in &all {
            let src = texts.get(*file).map_or("", String::as_str);
            let (toks, pair) = lexed.entry(*file).or_insert_with(|| {
                let toks = lex(src);
                let pair = super::lexer::match_brackets(src, &toks);
                (toks, pair)
            });
            let (line, col) = super::line_col(src, s.start);
            let enclosing = toks.partition_point(|t| t.start < *body);
            let _ = writeln!(
                text,
                "{file}:{line}:{col}\t{}\t{name}\t{}\t{}\t{}",
                s.class,
                src.get(s.start..s.end)
                    .unwrap_or_default()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                s.replacement
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                chain(src, toks, pair, enclosing, s.start).join("\t"),
            );
        }
        std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    if sample > 0 {
        let path = audit.ok_or_else(|| format!("--sample needs --audit\n{USAGE}"))?;
        all.sort_by(|x, y| x.0.cmp(&y.0).then_with(|| x.1.cmp(y.1)));
        let mut queues: Vec<Vec<&Picked>> = EXEC_CLASSES
            .iter()
            .map(|c| all.iter().filter(|x| x.3.class == *c).rev().collect())
            .collect();
        let mut picked = Vec::new();
        while picked.len() < sample && queues.iter().any(|q| !q.is_empty()) {
            for q in &mut queues {
                if picked.len() < sample
                    && let Some(x) = q.pop()
                {
                    picked.push(x);
                }
            }
        }
        let mut text = String::new();
        for (n, (_, file, (_, fn_line, body), s)) in picked.iter().enumerate() {
            let src = texts.get(*file).map_or("", String::as_str);
            let toks = lex(src);
            let pair = super::lexer::match_brackets(src, &toks);
            let (line, col) = super::line_col(src, s.start);
            let enclosing = toks.partition_point(|t| t.start < *body);
            let ls = src
                .get(..s.start)
                .and_then(|x| x.rfind('\n'))
                .map_or(0, |p| p + 1);
            let ctx_start = (0..4).fold(ls, |p, _| {
                src.get(..p.saturating_sub(1))
                    .and_then(|x| x.rfind('\n'))
                    .map_or(0, |q| q + 1)
            });
            let le = src
                .get(s.end..)
                .and_then(|x| x.find('\n'))
                .map_or(src.len(), |p| s.end + p);
            let ctx_end = (0..2).fold(le, |p, _| {
                src.get(p + 1..)
                    .and_then(|x| x.find('\n'))
                    .map_or(src.len(), |q| p + 1 + q)
            });
            let sig = src
                .lines()
                .nth(fn_line.saturating_sub(1))
                .unwrap_or_default()
                .trim();
            let heads = chain(src, &toks, &pair, enclosing, s.start)
                .iter()
                .fold(String::new(), |acc, h| format!("{acc}\n      in: {h}"));
            let _ = writeln!(
                text,
                "#{:03} {} {file}:{line}:{col} [{}] {}\n      fn (line {fn_line}): {sig}{heads}\n{}⟦{}⟧→⟦{}⟧{}\n",
                n + 1,
                t.project,
                s.class,
                s.what,
                src.get(ctx_start..s.start).unwrap_or_default(),
                src.get(s.start..s.end).unwrap_or_default(),
                s.replacement,
                src.get(s.end..ctx_end).unwrap_or_default(),
            );
        }
        std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("wrote {} sampled sites to {}", picked.len(), path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "verus! {
fn sum(v: &Vec<u64>, n: usize) -> (r: u64)
    requires n <= v.len(),
{
    let mut i: usize = 0;
    let mut s: u64 = 0;
    while i < n
        invariant
            i <= n,
            n <= v.len(),
        decreases n - i,
    {
        proof { lemma(i as int + 1); }
        assert(i + 1 <= n);
        s = s.wrapping_add(v[i]);
        i = i + 1;
    }
    for j in 0..n
        invariant j <= n,
    {
        s = s ^ v[j];
    }
    s
}
spec fn sp(a: int) -> int { a + 1 }
proof fn pf(a: int) ensures a + 0 == a { }
}";

    #[test]
    fn sites_touch_only_exec_tokens() {
        let p = plan(SRC, &UnitFacts::default(), &|_, s| {
            u64::try_from(s).unwrap_or(0)
        });
        assert_eq!(p.fns.len(), 1);
        assert_eq!(p.dropped_relex, 0);
        let texts: Vec<String> = p
            .sites
            .iter()
            .map(|(_, s)| format!("{} {}", s.class, s.what))
            .collect();
        // Nothing from the invariants, the proof block or the assert.
        for (_, s) in &p.sites {
            let line = SRC.get(..s.start).unwrap_or_default().matches('\n').count() + 1;
            assert!(
                !matches!(line, 3 | 8..=11 | 13 | 14 | 19 | 25 | 26),
                "site on ghost line {line}: {s:?}"
            );
        }
        for want in [
            "loop-bound `<` to `<=`",
            "loop-bound `..` to `..=` in `for j in 0..n`",
            "off-by-one index `i` to `i + 1`",
            "off-by-one index `j` to `j - 1`",
            "off-by-one range end `n` to `n + 1`",
            "off-by-one loop condition `i < n` to `i < n - 1`",
            "arith `+` to `-`",
            "statement delete `i = i + 1;`",
        ] {
            assert!(texts.iter().any(|t| t == want), "no `{want}` in {texts:#?}");
        }
        // The `while` comparison is a loop-bound site, not also a compare.
        assert!(!texts.iter().any(|t| t.starts_with("compare")));
    }

    #[test]
    fn a_mutant_that_opens_a_comment_is_dropped() {
        // `a**b` with `*` to `/` would read as `a/*b`: a block comment.
        let src = "verus! { fn f(a: u64, b: &u64) -> u64 { a**b } }";
        let p = plan(src, &UnitFacts::default(), &|_, _| 0);
        assert_eq!(p.dropped_relex, 1);
        assert!(p.sites.iter().all(|(_, s)| s.class != "arith"));
    }
}
