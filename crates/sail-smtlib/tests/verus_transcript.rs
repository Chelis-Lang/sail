//! Checks against a real transcript: the bytes Verus 0.2026.10.04.426d8b0
//! sent to Z3 4.16.0, and Z3's replies, when verifying
//! `corpus/fixtures/toy/toy.rs`.

use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::sexpr::parse_one;
use sail_smtlib::split::{CommandSplitter, Item, ItemKind, ResponseSplitter};

const INPUT: &[u8] = include_bytes!("fixtures/verus-toy.in.smt2");
const OUTPUT: &[u8] = include_bytes!("fixtures/verus-toy.out.txt");

fn split_commands(chunk: usize) -> Vec<Item> {
    let mut splitter = CommandSplitter::new();
    let mut items = Vec::new();
    for part in INPUT.chunks(chunk) {
        splitter.push(part, &mut items);
    }
    items.extend(splitter.finish());
    items
}

fn split_responses() -> Vec<Item> {
    let mut splitter = ResponseSplitter::new();
    let mut items = Vec::new();
    splitter.push(OUTPUT, &mut items);
    items.extend(splitter.finish());
    items
}

#[test]
fn every_command_is_a_well_formed_list_at_its_offset() {
    let items = split_commands(INPUT.len());
    assert_ne!(items.len(), 0);
    for item in &items {
        assert_eq!(item.kind, ItemKind::List);
        let start = usize::try_from(item.offset).unwrap_or(usize::MAX);
        assert_eq!(
            INPUT.get(start..start + item.bytes.len()),
            Some(&item.bytes[..])
        );
        if let Err(e) = parse_one(&item.bytes) {
            unreachable!("command at {} does not parse: {e}", item.offset);
        }
        if let Err(e) = classify(&item.bytes) {
            unreachable!("command at {} does not classify: {e}", item.offset);
        }
    }
}

#[test]
fn chunking_does_not_change_the_split() {
    let whole = split_commands(INPUT.len());
    for chunk in [1, 2, 3, 7, 64, 4096] {
        assert_eq!(split_commands(chunk), whole, "chunk size {chunk}");
    }
}

#[test]
fn protocol_shape_matches_the_recorded_session() {
    let items = split_commands(INPUT.len());
    let kinds: Vec<CommandKind<'_>> = items
        .iter()
        .filter_map(|i| classify(&i.bytes).ok())
        .map(|c| c.kind)
        .collect();
    let count = |f: fn(&CommandKind<'_>) -> bool| kinds.iter().filter(|k| f(k)).count();
    assert_eq!(count(|k| matches!(k, CommandKind::CheckSat)), 11);
    assert_eq!(count(|k| matches!(k, CommandKind::Echo)), 52);
    assert_eq!(count(|k| matches!(k, CommandKind::Push(1))), 8);
    assert_eq!(count(|k| matches!(k, CommandKind::Pop(1))), 7);
    assert_eq!(count(|k| matches!(k, CommandKind::Other)), 0);
}

#[test]
fn responses_pair_one_to_one_with_output_producing_commands() {
    let commands = split_commands(INPUT.len());
    let producing: Vec<_> = commands
        .iter()
        .filter_map(|i| classify(&i.bytes).ok())
        .filter(|c| c.kind.output() == Output::One)
        .collect();
    let responses = split_responses();
    assert_eq!(responses.len(), producing.len());
    for (command, response) in producing.iter().zip(&responses) {
        match command.kind {
            CommandKind::Echo => assert_eq!(response.bytes, b"<<DONE>>"),
            CommandKind::CheckSat => assert!(
                matches!(&response.bytes[..], b"sat" | b"unsat" | b"unknown"),
                "check-sat answered {:?}",
                String::from_utf8_lossy(&response.bytes)
            ),
            CommandKind::GetInfo {
                keyword: b":version",
            } => {
                assert_eq!(response.bytes, b"(:version \"4.16.0\")");
            }
            _ => assert_eq!(response.kind, ItemKind::List),
        }
    }
}
