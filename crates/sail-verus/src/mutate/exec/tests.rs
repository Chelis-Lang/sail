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

fn sites_of(src: &str) -> FilePlan {
    plan(src, &UnitFacts::default(), &|_, s| {
        u64::try_from(s).unwrap_or(0)
    })
}

fn line_of(src: &str, p: usize) -> usize {
    src.get(..p).unwrap_or_default().matches('\n').count() + 1
}

fn descriptions(p: &FilePlan) -> Vec<String> {
    p.sites
        .iter()
        .map(|(_, s)| format!("{} {}", s.class, s.what))
        .collect()
}

#[test]
fn sites_touch_only_exec_tokens() {
    let p = sites_of(SRC);
    assert_eq!(p.fns.len(), 1);
    assert_eq!(p.dropped_relex, 0);
    let texts = descriptions(&p);
    // Nothing from the requires, the invariants, the proof block, the
    // assert, or the spec and proof functions.
    for (_, s) in &p.sites {
        let line = line_of(SRC, s.start);
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
fn spec_classes_are_excluded_by_type() {
    // Every catalogue class maps to an exec class only if it mutates
    // executable code.
    for class in super::super::CLASSES {
        let exec = ExecClass::from_catalogue(class);
        assert_eq!(
            exec.is_none(),
            matches!(*class, "invariant" | "spec-weaken" | "spec-strengthen"),
            "{class}"
        );
        if let Some(e) = exec {
            assert_eq!(e.name(), *class);
        }
    }
    let names: BTreeSet<&str> = ExecClass::ALL.iter().map(|c| c.name()).collect();
    assert_eq!(names.len(), ExecClass::ALL.len());
}

#[test]
fn a_mutant_that_opens_a_comment_is_dropped() {
    // `a**b` with `*` to `/` would read as `a/*b`: a block comment.
    let src = "verus! { fn f(a: u64, b: &u64) -> u64 { a**b } }";
    let p = plan(src, &UnitFacts::default(), &|_, _| 0);
    assert_eq!(p.dropped_relex, 1);
    assert!(p.sites.iter().all(|(_, s)| s.class != ExecClass::Arith));
}

#[test]
fn swaps_adjacent_arguments() {
    let src = "verus! {
fn f(a: u64, b: u64) -> u64 {
    let x = g(a, b + 1, Ghost(a as int));
    let y = h(a, a);
    x.max(y, b)
}
}";
    let p = sites_of(src);
    let swaps: Vec<String> = p
        .sites
        .iter()
        .filter(|(_, s)| s.class == ExecClass::SwappedArgs)
        .filter_map(|(_, s)| s.apply(src))
        .collect();
    // `g`'s list holds a ghost argument, so it is not wholly executable;
    // `h(a, a)` swaps to itself.
    assert_eq!(swaps.len(), 1, "{swaps:#?}");
    assert!(swaps.iter().all(|m| m.contains("x.max(b, y)")));
}

#[test]
fn drops_only_a_final_statement_else() {
    let src = "verus! {
fn f(a: u64) -> (r: u64) {
    let mut r = 0;
    if a > 1 {
        r = 1;
    } else if a > 0 {
        r = 2;
    } else {
        r = 3;
    }
    let v = if a > 5 { 1 } else { 2 };
    r + v
}
}";
    let p = sites_of(src);
    let dropped: Vec<String> = p
        .sites
        .iter()
        .filter(|(_, s)| s.class == ExecClass::DroppedElse)
        .filter_map(|(_, s)| s.apply(src))
        .collect();
    // The value-producing `if` keeps its else; the chain loses only its
    // last arm.
    assert_eq!(dropped.len(), 1, "{dropped:#?}");
    assert!(
        dropped
            .iter()
            .all(|m| m.contains("r = 2;\n    }\n    let v")
                && !m.contains("r = 3;")
                && m.contains("else { 2 }"))
    );
}

#[test]
fn returns_early_only_from_unit_functions() {
    let src = "verus! {
fn push_two(v: &mut Vec<u64>, a: u64)
    ensures v.len() == old(v).len() + 2,
{
    v.push(a);
    proof { lemma(); }
    v.push(a);
}
fn value(a: u64) -> u64 {
    foo(a);
    a
}
}";
    let p = sites_of(src);
    let returns: Vec<(usize, String)> = p
        .sites
        .iter()
        .filter(|(_, s)| s.class == ExecClass::EarlyReturn)
        .map(|(_, s)| (line_of(src, s.start), s.apply(src).unwrap_or_default()))
        .collect();
    assert_eq!(
        returns.iter().map(|r| r.0).collect::<Vec<_>>(),
        [5, 7],
        "{returns:#?}"
    );
    assert!(
        returns
            .iter()
            .any(|r| r.1.contains("{\n    return;\n    v.push(a);\n    proof"))
    );
}

#[test]
fn every_site_is_one_well_formed_replacement() {
    let p = sites_of(SRC);
    for (_, s) in &p.sites {
        let m = s.apply(SRC).unwrap_or_default();
        assert_ne!(m, SRC, "{s:?}");
        // Brackets stay balanced.
        let balance = |t: &str, o: char, c: char| {
            t.chars().filter(|x| *x == o).count() == t.chars().filter(|x| *x == c).count()
        };
        for (o, c) in [('(', ')'), ('{', '}'), ('[', ']')] {
            assert!(balance(&m, o, c), "{s:?}");
        }
    }
}

#[test]
fn no_site_inside_a_ghost_wrapper_call() {
    // Each line's only arithmetic, comparison and constant are ghost
    // arguments of a `Ghost`/`Tracked` call; only `b + 1` is exec.
    let src = "verus! {
fn f(a: u64, b: u64) -> u64 {
    let g = Ghost::<u64>::new(a + 1);
    let t = Tracked::<u64>::method(a - 2);
    let h = vstd::prelude::Ghost::new(a * 3);
    let u = Tracked::<Vec<u64>>::method::<u64>(a < 4);
    b + 1
}
}";
    let p = sites_of(src);
    assert_eq!(p.fns.len(), 1);
    for (_, s) in &p.sites {
        assert_eq!(line_of(src, s.start), 7, "site in a ghost argument: {s:?}");
    }
    assert!(p.sites.iter().any(|(_, s)| s.class == ExecClass::Arith));
}
