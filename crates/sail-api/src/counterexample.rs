//! Source-named inputs decoded from a failed check's solver model.
//!
//! Verus encodes a function parameter `x` as the SMT constant `x!` and a struct
//! value as a constructor application such as `(krate!Row./Row false 3)`, whose
//! arguments follow the struct's declaration order. A model also defines many
//! internal constants: fuel, type identities, location labels and versioned
//! locals. Decoding keeps only constants Verus names `IDENT!`, which include
//! the parameters and the return binder, renders their values, and names struct fields when the caller supplies the
//! declaration order. The raw model is never replaced: it stays in the
//! obligation's [`crate::output::Counterexample`].
//!
//! Decoded values are what the solver's model assigned for one failed or
//! unknown check. Sail does not evaluate them against the source, so they are
//! not a validated concrete counterexample.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::output::{AnswerName, Counterexample, FunctionReport, ItfValue, OutcomeName};

/// Decoded counterexample contract version.
pub const VERSION: u32 = 1;

const QUALIFIER_MODEL: &str = "values are the solver's model for one failed or unknown check; Sail does not evaluate them against the source, so they are not a validated concrete counterexample";
const QUALIFIER_RAW: &str = "the raw model, including every internal constant, is retained in the obligation's counterexample";
const QUALIFIER_UNKNOWN: &str =
    "the check's answer was unknown, so the model may be partial or spurious";
const QUALIFIER_POSITIONAL: &str =
    "some struct field names were unavailable; those fields are named by declaration position";

/// A decoded value: a rendered scalar or a struct record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Value {
    /// A scalar in ITF's value encoding, or the verbatim text when unreadable.
    Scalar {
        /// The rendered value.
        itf: ItfValue,
    },
    /// A struct value built by its constructor.
    Record {
        /// Source constructor (struct) name.
        constructor: String,
        /// Fields in declaration order.
        fields: Vec<Field>,
    },
}

/// One struct field of a decoded record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    /// Source field name, or its zero-based declaration position.
    pub name: String,
    /// Decoded value.
    pub value: Value,
}

/// One source-named model constant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    /// Source parameter or return-binder name.
    pub name: String,
    /// The SMT constant name, verbatim.
    pub smt_name: String,
    /// The SMT sort, verbatim.
    pub sort: String,
    /// Decoded value.
    pub value: Value,
}

/// Source-named inputs from the first model of one failed or unknown obligation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decoded {
    /// Equals [`VERSION`].
    pub version: u32,
    /// Exact function name as the verifier reports it.
    pub function: String,
    /// Obligation identity whose counterexample was decoded.
    pub obligation: String,
    /// Index of the check that produced the model.
    pub check: u32,
    /// That check's answer, when recorded.
    pub answer: Option<AnswerName>,
    /// Source-named constants, sorted by name.
    pub inputs: Vec<Input>,
    /// Number of internal model constants not decoded.
    pub omitted: usize,
    /// Explicit limits of this evidence, sorted.
    pub qualifiers: Vec<String>,
}

/// Struct field names in declaration order, keyed by struct name.
pub type FieldNames = BTreeMap<String, Vec<String>>;

/// Decode the first counterexample of a failed or unknown obligation of `function`.
///
/// Returns `None` when no such obligation has a model.
#[must_use]
pub fn decode(function: &FunctionReport, fields: &FieldNames) -> Option<Decoded> {
    let (obligation, counterexample) = function
        .obligations
        .iter()
        .flatten()
        .filter(|o| !o.advisory && o.outcome != OutcomeName::Proved)
        .find_map(|o| o.counterexample.as_ref().map(|c| (o, c)))?;
    let answer = obligation
        .checks
        .get(usize::try_from(counterexample.check).ok()?)
        .and_then(|check| check.answer);
    let mut positional = false;
    let (mut inputs, omitted) = inputs(counterexample, fields, &mut positional);
    inputs.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.smt_name.cmp(&b.smt_name))
    });
    let mut qualifiers = vec![QUALIFIER_MODEL, QUALIFIER_RAW];
    if answer != Some(AnswerName::Sat) {
        qualifiers.push(QUALIFIER_UNKNOWN);
    }
    if positional {
        qualifiers.push(QUALIFIER_POSITIONAL);
    }
    qualifiers.sort_unstable();
    Some(Decoded {
        version: VERSION,
        function: function.function.clone(),
        obligation: obligation.id.clone(),
        check: counterexample.check,
        answer,
        inputs,
        omitted,
        qualifiers: qualifiers.into_iter().map(String::from).collect(),
    })
}

fn inputs(
    counterexample: &Counterexample,
    fields: &FieldNames,
    positional: &mut bool,
) -> (Vec<Input>, usize) {
    let mut decoded = Vec::new();
    let mut omitted = 0;
    for assignment in &counterexample.assignments {
        let Some(name) = source_name(&assignment.name) else {
            omitted += 1;
            continue;
        };
        let value = match parse(&assignment.value) {
            Some(Term::List(items)) => record(&items, fields, positional),
            _ => None,
        }
        .unwrap_or_else(|| Value::Scalar {
            itf: assignment.itf.clone(),
        });
        decoded.push(Input {
            name: name.to_owned(),
            smt_name: assignment.name.clone(),
            sort: assignment.sort.clone(),
            value,
        });
    }
    (decoded, omitted)
}

/// `x!` names source parameter `x`; every other spelling is internal.
fn source_name(smt: &str) -> Option<&str> {
    let name = smt.strip_suffix('!')?;
    let mut chars = name.chars();
    let first = chars.next()?;
    ((first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name != "_")
        .then_some(name)
}

#[derive(Debug, PartialEq, Eq)]
enum Term {
    Atom(String),
    List(Vec<Self>),
}

/// Parse one s-expression; `None` for anything malformed or with trailing text.
fn parse(text: &str) -> Option<Term> {
    let mut tokens = Vec::new();
    let mut atom = String::new();
    for c in text.chars() {
        if c == '(' || c == ')' || c.is_whitespace() {
            if !atom.is_empty() {
                tokens.push(std::mem::take(&mut atom));
            }
            if !c.is_whitespace() {
                tokens.push(c.to_string());
            }
        } else if c == '|' || c == '"' {
            return None;
        } else {
            atom.push(c);
        }
    }
    if !atom.is_empty() {
        tokens.push(atom);
    }
    let mut stack: Vec<Vec<Term>> = vec![Vec::new()];
    for token in tokens {
        match token.as_str() {
            "(" => stack.push(Vec::new()),
            ")" => {
                let items = stack.pop()?;
                stack.last_mut()?.push(Term::List(items));
            }
            _ => stack.last_mut()?.push(Term::Atom(token)),
        }
    }
    let mut top = stack.pop()?;
    (stack.is_empty() && top.len() == 1).then(|| top.remove(0))
}

/// `(krate!Path./Ctor a b)` becomes a record when every argument decodes.
fn record(items: &[Term], fields: &FieldNames, positional: &mut bool) -> Option<Value> {
    let (Term::Atom(head), arguments) = items.split_first()? else {
        return None;
    };
    let (path, constructor) = head.rsplit_once("./")?;
    let (_, path) = path.split_once('!')?;
    let structure = path.rsplit('.').next()?;
    if structure != constructor || constructor.is_empty() {
        return None;
    }
    let names = fields
        .get(constructor)
        .filter(|names| names.len() == arguments.len());
    *positional |= names.is_none() && !arguments.is_empty();
    let fields = arguments
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            Some(Field {
                name: names.map_or_else(|| index.to_string(), |names| names[index].clone()),
                value: argument_value(argument, fields, positional)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Value::Record {
        constructor: constructor.to_owned(),
        fields,
    })
}

fn argument_value(term: &Term, fields: &FieldNames, positional: &mut bool) -> Option<Value> {
    match term {
        Term::Atom(atom) if atom == "true" || atom == "false" => Some(Value::Scalar {
            itf: ItfValue::render("Bool", atom),
        }),
        Term::Atom(atom) => Some(Value::Scalar {
            itf: ItfValue::render("Int", atom),
        }),
        Term::List(items) => match items.as_slice() {
            [Term::Atom(minus), Term::Atom(digits)] if minus == "-" => Some(Value::Scalar {
                itf: ItfValue::render("Int", &format!("(- {digits})")),
            }),
            _ => record(items, fields, positional),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_identifiers_followed_by_a_bang_are_source_names() {
        for (smt, expected) in [
            ("x!", Some("x")),
            ("ret!", Some("ret")),
            ("_tmp1!", Some("_tmp1")),
            ("fuel%vstd!seq.group_seq_lemmas.", None),
            ("TYPE%probe!Row.", None),
            ("i@0", None),
            ("$", None),
            ("SZ", None),
            ("%%location_label%%0", None),
            ("_!", None),
            ("1x!", None),
            ("a.b!", None),
        ] {
            assert_eq!(source_name(smt), expected, "{smt}");
        }
    }

    #[test]
    fn constructor_terms_become_named_or_positional_records() {
        let fields = FieldNames::from([
            ("Row".into(), vec!["flag".into(), "count".into()]),
            ("Outer".into(), vec!["row".into(), "delta".into()]),
        ]);
        let scalar = |itf: ItfValue| Value::Scalar { itf };
        let int = |n: &str| {
            scalar(ItfValue::BigInt {
                bigint: n.to_owned(),
            })
        };
        let row = |flag: bool, count: &str, names: [&str; 2]| Value::Record {
            constructor: "Row".into(),
            fields: vec![
                Field {
                    name: names[0].into(),
                    value: scalar(ItfValue::Bool(flag)),
                },
                Field {
                    name: names[1].into(),
                    value: int(count),
                },
            ],
        };
        let mut positional = false;
        let decode = |text: &str, fields: &FieldNames, positional: &mut bool| match parse(text) {
            Some(Term::List(items)) => record(&items, fields, positional),
            _ => None,
        };
        assert_eq!(
            decode("(probe!Row./Row false 3)", &fields, &mut positional),
            Some(row(false, "3", ["flag", "count"]))
        );
        assert!(!positional);
        assert_eq!(
            decode(
                "(probe!m.Outer./Outer (probe!Row./Row true 0) (- 2))",
                &fields,
                &mut positional
            ),
            Some(Value::Record {
                constructor: "Outer".into(),
                fields: vec![
                    Field {
                        name: "row".into(),
                        value: row(true, "0", ["flag", "count"]),
                    },
                    Field {
                        name: "delta".into(),
                        value: int("-2"),
                    },
                ],
            })
        );
        assert_eq!(
            decode(
                "(probe!Row./Row false 3)",
                &FieldNames::new(),
                &mut positional
            ),
            Some(row(false, "3", ["0", "1"]))
        );
        assert!(positional);
        for malformed in [
            "(probe!Row./Other false 3)",
            "(probe!Row./Row false 3",
            "(probe!Row./Row false 3) extra",
            "(Row false 3)",
            "(|probe!Row./Row| false)",
            "(probe!Row./Row (f 1))",
        ] {
            assert_eq!(
                decode(malformed, &fields, &mut positional),
                None,
                "{malformed}"
            );
        }
    }
}
