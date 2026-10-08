//! A small JSON Schema checker for the subset sail's schemas use.
//!
//! Supported keywords: `type` (a name or a list of names), `const`, `enum`,
//! `properties`, `required`, `additionalProperties: false`, `items`,
//! `oneOf`, `minimum`, and `$ref` to `#/$defs/<name>`. Annotation keywords
//! (`$schema`, `$id`, `title`, `description`) are ignored. Any other keyword
//! is reported as unsupported, so a schema can never silently check less
//! than it says.

use serde_json::Value;

const ANNOTATIONS: &[&str] = &["$schema", "$id", "title", "description", "$defs"];

fn type_matches(name: &str, v: &Value) -> bool {
    match name {
        "null" => v.is_null(),
        "boolean" => v.is_boolean(),
        "string" => v.is_string(),
        "integer" => v.is_i64() || v.is_u64(),
        "number" => v.is_number(),
        "array" => v.is_array(),
        "object" => v.is_object(),
        _ => false,
    }
}

struct Checker<'a> {
    root: &'a Value,
    errors: Vec<String>,
}

fn resolve<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let name = reference.strip_prefix("#/$defs/")?;
    root.get("$defs")?.get(name)
}

impl<'a> Checker<'a> {
    fn fail(&mut self, path: &str, msg: impl std::fmt::Display) {
        self.errors.push(format!(
            "{}: {msg}",
            if path.is_empty() { "$" } else { path }
        ));
    }

    fn check(&mut self, schema: &'a Value, v: &Value, path: &str) {
        let Some(obj) = schema.as_object() else {
            self.fail(path, "schema is not an object");
            return;
        };
        for (key, rule) in obj {
            match key.as_str() {
                k if ANNOTATIONS.contains(&k) => {}
                "$ref" => match rule.as_str().and_then(|r| resolve(self.root, r)) {
                    Some(target) => self.check(target, v, path),
                    None => self.fail(path, format!("unresolvable $ref {rule}")),
                },
                "type" => {
                    let names: Vec<&str> = match rule {
                        Value::String(s) => vec![s.as_str()],
                        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
                        _ => Vec::new(),
                    };
                    if !names.iter().any(|n| type_matches(n, v)) {
                        self.fail(path, format!("expected type {rule}, got {v}"));
                    }
                }
                "const" => {
                    if rule != v {
                        self.fail(path, format!("expected {rule}, got {v}"));
                    }
                }
                "enum" => {
                    if !rule.as_array().is_some_and(|a| a.contains(v)) {
                        self.fail(path, format!("{v} is not one of {rule}"));
                    }
                }
                "minimum" => {
                    if let (Some(min), Some(n)) = (rule.as_f64(), v.as_f64())
                        && n < min
                    {
                        self.fail(path, format!("{n} is below {min}"));
                    }
                }
                "required" => {
                    if let Some(o) = v.as_object() {
                        for name in rule
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                        {
                            if !o.contains_key(name) {
                                self.fail(path, format!("missing required field {name}"));
                            }
                        }
                    }
                }
                "properties" => {
                    if let (Some(props), Some(o)) = (rule.as_object(), v.as_object()) {
                        for (name, sub) in props {
                            if let Some(value) = o.get(name) {
                                self.check(sub, value, &format!("{path}.{name}"));
                            }
                        }
                    }
                }
                "additionalProperties" => {
                    if rule == &Value::Bool(false)
                        && let Some(o) = v.as_object()
                    {
                        let known = obj.get("properties").and_then(Value::as_object);
                        for name in o.keys() {
                            if !known.is_some_and(|k| k.contains_key(name)) {
                                self.fail(path, format!("unexpected field {name}"));
                            }
                        }
                    } else if rule != &Value::Bool(false) && rule != &Value::Bool(true) {
                        self.fail(path, "unsupported additionalProperties schema");
                    }
                }
                "items" => {
                    if let Some(a) = v.as_array() {
                        for (i, item) in a.iter().enumerate() {
                            self.check(rule, item, &format!("{path}[{i}]"));
                        }
                    }
                }
                "oneOf" => self.one_of(rule, v, path),
                other => self.fail(path, format!("unsupported schema keyword {other}")),
            }
        }
    }

    fn one_of(&mut self, rule: &'a Value, v: &Value, path: &str) {
        let options = rule.as_array().map_or(&[][..], Vec::as_slice);
        let matching = options
            .iter()
            .filter(|o| {
                let mut sub = Checker {
                    root: self.root,
                    errors: Vec::new(),
                };
                sub.check(o, v, path);
                sub.errors.is_empty()
            })
            .count();
        if matching != 1 {
            self.fail(
                path,
                format!("matches {matching} of the oneOf options, not 1"),
            );
        }
    }
}

/// Checks `value` against `schema`. Returns every violation, with its path.
#[must_use]
pub fn validate(schema: &Value, value: &Value) -> Vec<String> {
    let mut checker = Checker {
        root: schema,
        errors: Vec::new(),
    };
    checker.check(schema, value, "");
    checker.errors
}

#[cfg(test)]
mod tests {
    use super::validate;
    use serde_json::json;

    #[test]
    fn checks_the_supported_keywords() {
        let schema = json!({
            "type": "object",
            "required": ["a", "b"],
            "additionalProperties": false,
            "properties": {
                "a": {"type": ["integer", "null"], "minimum": 0},
                "b": {"enum": ["x", "y"]},
                "c": {"type": "array", "items": {"$ref": "#/$defs/d"}},
                "e": {"oneOf": [{"type": "null"}, {"const": 1}]}
            },
            "$defs": {"d": {"type": "string"}}
        });
        assert_eq!(
            validate(&schema, &json!({"a": 1, "b": "x", "c": ["s"], "e": 1})),
            Vec::<String>::new()
        );
        assert_eq!(
            validate(&schema, &json!({"a": null, "b": "y", "e": null})),
            Vec::<String>::new()
        );
        let errors = validate(
            &schema,
            &json!({"a": -1, "b": "z", "c": [3], "e": 2, "f": 0}),
        );
        assert_eq!(errors.len(), 5, "{errors:?}");
        assert_eq!(validate(&schema, &json!({})).len(), 2);
        assert_eq!(validate(&json!({"format": "x"}), &json!(1)).len(), 1);
        assert_eq!(
            validate(&json!({"$ref": "#/$defs/missing"}), &json!(1)).len(),
            1
        );
    }
}
