//! Property tests: round-tripping, chunking invariance and panic freedom.

use proptest::prelude::*;
use sail_smtlib::sexpr::{SExpr, parse_all};
use sail_smtlib::split::{CommandSplitter, ItemKind};

/// A generated s-expression, rendered with arbitrary layout.
#[derive(Debug, Clone)]
enum Tree {
    Atom(String),
    List(Vec<Tree>),
}

fn atom() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z~!@$%^&*_+=<>.?/-][a-zA-Z0-9~!@$%^&*_+=<>.?/-]{0,8}",
        ":[a-z][a-z.-]{0,8}",
        "[0-9]{1,6}",
        "[0-9]{1,3}\\.[0-9]{1,3}",
        "#x[0-9a-fA-F]{1,4}",
        "#b[01]{1,8}",
        // Strings with escaped quotes, parentheses and newlines inside.
        "[a-z()\n ;|]{0,6}".prop_map(|s| format!("\"{s}\"\"{s}\"")),
        // Quoted symbols containing parentheses and quotes.
        "[a-z() \";]{0,6}".prop_map(|s| format!("|{s}|")),
    ]
}

fn tree() -> impl Strategy<Value = Tree> {
    atom()
        .prop_map(Tree::Atom)
        .prop_recursive(5, 64, 6, |inner| {
            prop::collection::vec(inner, 0..6).prop_map(Tree::List)
        })
}

fn gap() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(" ".to_owned()),
        Just("\n".to_owned()),
        Just("  \t".to_owned()),
        Just("\r\n".to_owned()),
        Just(" ; comment (with \"parens\n".to_owned()),
    ]
}

fn render(tree: &Tree, gaps: &mut impl Iterator<Item = String>, out: &mut String) {
    match tree {
        Tree::Atom(a) => out.push_str(a),
        Tree::List(items) => {
            out.push('(');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(&gaps.next().unwrap_or_else(|| " ".to_owned()));
                }
                render(item, gaps, out);
            }
            out.push(')');
        }
    }
}

fn canonical(tree: &Tree, out: &mut String) {
    match tree {
        Tree::Atom(a) => out.push_str(a),
        Tree::List(items) => {
            out.push('(');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                canonical(item, out);
            }
            out.push(')');
        }
    }
}

fn commands() -> impl Strategy<Value = (Vec<Tree>, Vec<String>)> {
    (
        prop::collection::vec(
            tree().prop_map(|t| Tree::List(vec![Tree::Atom("cmd".to_owned()), t])),
            1..8,
        ),
        prop::collection::vec(gap(), 64),
    )
}

proptest! {
    #[test]
    fn parse_preserves_bytes_and_structure((trees, gaps) in commands()) {
        let mut gaps = gaps.into_iter().cycle();
        let mut src = String::new();
        let mut expected = Vec::new();
        for t in &trees {
            src.push_str(&gaps.next().unwrap_or_default());
            let begin = src.len();
            render(t, &mut gaps, &mut src);
            expected.push(src[begin..].to_owned());
        }
        let parsed = parse_all(src.as_bytes());
        prop_assert!(parsed.is_ok(), "{:?}: {:?}", src, parsed.err());
        let parsed: Vec<SExpr> = parsed.unwrap_or_default();
        prop_assert_eq!(parsed.len(), trees.len());
        for ((expr, t), text) in parsed.iter().zip(&trees).zip(&expected) {
            prop_assert_eq!(expr.text(src.as_bytes()), text.as_bytes());
            let mut want = String::new();
            canonical(t, &mut want);
            prop_assert_eq!(expr.canonical(src.as_bytes()), want.into_bytes());
        }
    }

    #[test]
    fn splitter_agrees_with_parser_under_any_chunking((trees, gaps) in commands(), chunk in 1usize..40) {
        let mut gaps = gaps.into_iter().cycle();
        let mut src = String::new();
        for t in &trees {
            src.push_str(&gaps.next().unwrap_or_default());
            render(t, &mut gaps, &mut src);
        }
        let parsed = parse_all(src.as_bytes()).unwrap_or_default();
        let mut splitter = CommandSplitter::new();
        let mut items = Vec::new();
        for part in src.as_bytes().chunks(chunk) {
            splitter.push(part, &mut items);
        }
        prop_assert!(splitter.finish().is_none());
        prop_assert_eq!(items.len(), parsed.len());
        for (item, expr) in items.iter().zip(&parsed) {
            prop_assert_eq!(item.kind, ItemKind::List);
            prop_assert_eq!(&item.bytes[..], expr.text(src.as_bytes()));
            prop_assert_eq!(item.offset, expr.start as u64);
        }
    }

    #[test]
    fn nothing_panics_on_arbitrary_bytes(bytes in prop::collection::vec(any::<u8>(), 0..512), chunk in 1usize..32) {
        let _ = parse_all(&bytes);
        let _ = sail_smtlib::command::classify(&bytes);
        let mut splitter = CommandSplitter::new();
        let mut responses = sail_smtlib::split::ResponseSplitter::new();
        let mut items = Vec::new();
        for part in bytes.chunks(chunk) {
            splitter.push(part, &mut items);
            responses.push(part, &mut items);
        }
        let _ = splitter.finish();
        let _ = responses.finish();
    }
}
