//! Shared s-expression terms for the pre-solver spike.
//!
//! [`Node`]s are reference counted, so let-expansion and axiom instantiation
//! share subterms instead of copying them. Canonical text (atoms verbatim,
//! one space between list elements) identifies structurally equal terms.

use std::fmt::Write as _;
use std::rc::Rc;

use sail_smtlib::sexpr::{SExpr, SExprKind, parse_one};

/// A shared term.
pub type T = Rc<Node>;

/// An atom or a list.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Node {
    /// A token's exact text.
    Atom(Rc<str>),
    /// A parenthesised list.
    List(Vec<T>),
}

impl Node {
    /// The atom's text, if this is an atom.
    #[must_use]
    pub fn atom(&self) -> Option<&str> {
        match self {
            Node::Atom(a) => Some(a),
            Node::List(_) => None,
        }
    }

    /// The list's items, if this is a list.
    #[must_use]
    pub fn list(&self) -> Option<&[T]> {
        match self {
            Node::List(items) => Some(items),
            Node::Atom(_) => None,
        }
    }

    /// The head symbol of a list, if it is an atom.
    #[must_use]
    pub fn head(&self) -> Option<&str> {
        self.list()?.first()?.atom()
    }

    /// Appends the canonical text, giving up (returning `false`) once
    /// `out` would exceed `limit` bytes.
    pub fn write(&self, out: &mut String, limit: usize) -> bool {
        if out.len() > limit {
            return false;
        }
        match self {
            Node::Atom(a) => out.push_str(a),
            Node::List(items) => {
                out.push('(');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    if !item.write(out, limit) {
                        return false;
                    }
                }
                out.push(')');
            }
        }
        out.len() <= limit
    }

    /// The canonical text, if it is at most `limit` bytes.
    #[must_use]
    pub fn text(&self, limit: usize) -> Option<String> {
        let mut s = String::new();
        self.write(&mut s, limit).then_some(s)
    }
}

/// An atom.
#[must_use]
pub fn atom(s: &str) -> T {
    Rc::new(Node::Atom(Rc::from(s)))
}

/// A list.
#[must_use]
pub fn list(items: Vec<T>) -> T {
    Rc::new(Node::List(items))
}

fn convert(e: &SExpr, src: &[u8]) -> T {
    match &e.kind {
        SExprKind::Atom(_) => atom(&String::from_utf8_lossy(e.text(src))),
        SExprKind::List(items) => list(items.iter().map(|i| convert(i, src)).collect()),
    }
}

/// Parses one command's bytes.
#[must_use]
pub fn parse(bytes: &[u8]) -> Option<T> {
    parse_one(bytes).ok().map(|e| convert(&e, bytes))
}

/// Whether `t` mentions any atom for which `f` holds.
pub fn mentions(t: &Node, f: &mut impl FnMut(&str) -> bool) -> bool {
    match t {
        Node::Atom(a) => f(a),
        Node::List(items) => items.iter().any(|i| mentions(i, f)),
    }
}

/// Renders a list of terms for diagnostics.
#[must_use]
pub fn show(t: &Node) -> String {
    let mut s = String::new();
    if !t.write(&mut s, 400) {
        let _ = write!(s, "…");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_text_and_limits() {
        let t = parse(b"(assert  (=> a ; c\n b))");
        let Some(t) = t else { unreachable!() };
        assert_eq!(t.text(100).as_deref(), Some("(assert (=> a b))"));
        assert_eq!(t.text(5), None);
        assert_eq!(t.head(), Some("assert"));
        assert!(mentions(&t, &mut |a| a == "b"));
        assert!(!mentions(&t, &mut |a| a == "c"));
    }
}
