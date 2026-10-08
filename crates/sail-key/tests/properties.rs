//! Charter fast-tier properties: equal states give equal keys, and any
//! meaningful difference gives a different key.

use proptest::prelude::*;
use sail_key::{Mode, QueryKey, SolverIdentity, query_key};
use sail_session::{Event, Session, Snapshot};

fn z3() -> SolverIdentity {
    SolverIdentity {
        name: "z3".to_owned(),
        version: "4.16.0".to_owned(),
        binary_sha256: [7; 32],
    }
}

/// Runs `commands` and returns the snapshot at the last check.
fn last_check(commands: &[String]) -> Option<Snapshot> {
    let mut session = Session::new();
    let mut last = None;
    for c in commands {
        if let Event::Check(s) = session.apply(c.as_bytes()) {
            last = Some(*s);
        }
    }
    last
}

fn key(commands: &[String]) -> Option<QueryKey> {
    query_key(&last_check(commands)?, &z3(), &Mode::Default)
}

/// A random well-formed session that ends in a check.
fn session() -> impl Strategy<Value = Vec<String>> {
    let command = prop_oneof![
        "[a-z]{1,3}".prop_map(|v| format!("(declare-const {v} Int)")),
        ("[a-z]{1,3}", 0u32..100).prop_map(|(v, n)| format!("(assert (> {v} {n}))")),
        (0u32..5).prop_map(|n| format!("(set-option :smt.random_seed {n})")),
        Just("(push)".to_owned()),
        Just("(pop)".to_owned()),
        Just("(get-info :all-statistics)".to_owned()),
        Just("(echo \"<<DONE>>\")".to_owned()),
        Just("(check-sat)".to_owned()),
    ];
    prop::collection::vec(command, 0..24).prop_map(|mut cs| {
        // Keep pops balanced so the model stays exact.
        let mut depth = 0i32;
        cs.retain(|c| match c.as_str() {
            "(push)" => {
                depth += 1;
                true
            }
            "(pop)" if depth == 0 => false,
            "(pop)" => {
                depth -= 1;
                true
            }
            _ => true,
        });
        cs.push("(check-sat)".to_owned());
        cs
    })
}

proptest! {
    #[test]
    fn equal_states_give_equal_keys(cs in session()) {
        let a = key(&cs);
        prop_assert!(a.is_some());
        prop_assert_eq!(a, key(&cs));
        // Queries and echoes do not change state, so removing them keeps the key.
        let quiet: Vec<String> = cs
            .iter()
            .enumerate()
            .filter(|(i, c)| *i + 1 == cs.len() || (!c.starts_with("(get-info") && !c.starts_with("(echo") && *c != "(check-sat)"))
            .map(|(_, c)| c.clone())
            .collect();
        prop_assert_eq!(a, key(&quiet));
    }

    #[test]
    fn any_change_to_an_assertion_changes_the_key(cs in session(), n in 0u32..100) {
        let base = key(&cs);
        let mut changed = cs.clone();
        let last = changed.len() - 1;
        changed.insert(last, format!("(assert (> zz {n}))"));
        prop_assert_ne!(base, key(&changed));
    }

    #[test]
    fn option_changes_change_the_key(cs in session(), seed in 5u32..1000) {
        let mut changed = cs.clone();
        let last = changed.len() - 1;
        changed.insert(last, format!("(set-option :smt.random_seed {seed})"));
        prop_assert_ne!(key(&cs), key(&changed));
    }

    #[test]
    fn level_boundaries_are_part_of_the_key(cs in session()) {
        let mut changed = cs.clone();
        let last = changed.len() - 1;
        changed.insert(last, "(push)".to_owned());
        prop_assert_ne!(key(&cs), key(&changed));
    }
}

#[test]
fn solver_identity_and_mode_are_part_of_the_key() {
    let cs = vec!["(assert true)".to_owned(), "(check-sat)".to_owned()];
    let Some(snap) = last_check(&cs) else {
        unreachable!("session ends in a check")
    };
    let base = query_key(&snap, &z3(), &Mode::Default);
    let mut other_version = z3();
    other_version.version = "4.16.1".to_owned();
    let mut other_binary = z3();
    other_binary.binary_sha256[0] ^= 1;
    for solver in [other_version, other_binary] {
        assert_ne!(base, query_key(&snap, &solver, &Mode::Default));
    }
    let portfolio = query_key(&snap, &z3(), &Mode::OptIn("portfolio".to_owned()));
    assert_ne!(base, portfolio);
}

#[test]
fn inexact_snapshots_have_no_key() {
    let cs = vec!["(pop)".to_owned(), "(check-sat)".to_owned()];
    let Some(snap) = last_check(&cs) else {
        unreachable!("session ends in a check")
    };
    assert_eq!(query_key(&snap, &z3(), &Mode::Default), None);
}

#[test]
fn length_prefixing_prevents_concatenation_collisions() {
    let a = vec!["(assert ab)".to_owned(), "(check-sat)".to_owned()];
    let b = vec![
        "(assert a)".to_owned(),
        "(assert b)".to_owned(),
        "(check-sat)".to_owned(),
    ];
    assert_ne!(key(&a), key(&b));
}
