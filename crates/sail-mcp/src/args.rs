//! Tool argument validation.
//!
//! Arguments are checked strictly: unknown keys, wrong types and values that
//! could be read as flags are rejected with a message the model can act on
//! (an MCP tool execution error, `isError: true`).

use serde_json::{Map, Value};

/// Verus flags that limit a run to part of a crate, and Cargo flags that
/// pick a package or manifest. A run using any of them is scoped.
const SCOPING_FLAGS: &[&str] = &[
    "-p",
    "--package",
    "--manifest-path",
    "--workspace",
    "--verify-module",
    "--verify-function",
    "--verify-root",
    "--verify-only-module",
];

fn check_keys(args: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    match args.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(k) => Err(format!(
            "unknown argument `{k}` (expected: {})",
            allowed.join(", ")
        )),
        None => Ok(()),
    }
}

fn opt_str(args: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Err(format!("`{key}` must not be empty")),
        Some(Value::String(s)) if s.contains('\0') => Err(format!("`{key}` must not contain NUL")),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("`{key}` must be a string")),
    }
}

/// A string that becomes a flag's value: it must not look like a flag.
fn opt_value(args: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    let v = opt_str(args, key)?;
    if v.as_deref().is_some_and(|s| s.starts_with('-')) {
        return Err(format!("`{key}` must not start with `-`"));
    }
    Ok(v)
}

fn str_list(args: &Map<String, Value>, key: &str) -> Result<Vec<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| match v {
                Value::String(s) if !s.contains('\0') => Ok(s.clone()),
                _ => Err(format!("`{key}` must be an array of strings")),
            })
            .collect(),
        Some(_) => Err(format!("`{key}` must be an array of strings")),
    }
}

/// Arguments of `verify` (and `fragility`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunArgs {
    /// The project directory (default: the server's working directory).
    pub project_dir: Option<String>,
    /// `-p <package>`.
    pub package: Option<String>,
    /// Verus `--verify-module <module>`.
    pub module: Option<String>,
    /// Verus `--verify-function <function>` (with `--verify-root` when no
    /// module is given).
    pub function: Option<String>,
    /// Explicit solver replay measurement for per-function fragility.
    pub measure_fragility: bool,
    /// Explicit complete-run portfolio mode; never enabled by default.
    pub portfolio: Option<String>,
    /// Further `cargo verus verify` arguments, before `--`.
    pub cargo_args: Vec<String>,
    /// Further Verus arguments, after `--`.
    pub verus_args: Vec<String>,
}

impl RunArgs {
    /// The keys `verify` accepts.
    pub const KEYS: &[&str] = &[
        "project_dir",
        "package",
        "module",
        "function",
        "cargo_args",
        "portfolio",
        "measure_fragility",
        "verus_args",
    ];

    /// Validates `args`.
    ///
    /// # Errors
    ///
    /// A message for the model when an argument is unknown or invalid.
    pub fn parse(args: &Map<String, Value>) -> Result<Self, String> {
        check_keys(args, Self::KEYS)?;
        let parsed = Self {
            project_dir: opt_str(args, "project_dir")?,
            package: opt_value(args, "package")?,
            module: opt_value(args, "module")?,
            function: opt_value(args, "function")?,
            measure_fragility: args
                .get("measure_fragility")
                .map(|v| {
                    v.as_bool()
                        .ok_or_else(|| "measure_fragility must be boolean".to_owned())
                })
                .transpose()?
                .unwrap_or(false),
            portfolio: opt_value(args, "portfolio")?,
            cargo_args: str_list(args, "cargo_args")?,
            verus_args: str_list(args, "verus_args")?,
        };
        if parsed
            .portfolio
            .as_deref()
            .is_some_and(|p| p != "accept-first-proof")
        {
            return Err("`portfolio` must be `accept-first-proof`".to_owned());
        }
        if parsed.cargo_args.iter().any(|a| a == "--") {
            return Err(
                "`cargo_args` must not contain `--`; put Verus arguments in `verus_args`"
                    .to_owned(),
            );
        }
        if parsed
            .cargo_args
            .iter()
            .any(|a| a == "--output" || a.starts_with("--output="))
        {
            return Err(
                "`cargo_args` must not contain `--output`: the tool always uses `--output json`"
                    .to_owned(),
            );
        }
        Ok(parsed)
    }

    /// The `sail` arguments: `prefix` (for example `verify --output json`),
    /// the Cargo arguments, then `--` and the Verus arguments.
    #[must_use]
    pub fn sail_args(&self, prefix: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = prefix.iter().map(|s| (*s).to_owned()).collect();
        if self.measure_fragility && prefix.first() == Some(&"verify") {
            out.push("--measure-fragility".to_owned());
        }
        if let Some(mode) = &self.portfolio {
            out.extend(["--portfolio".to_owned(), mode.clone()]);
        }
        out.extend(self.cargo_args.iter().cloned());
        if let Some(p) = &self.package {
            out.extend(["-p".to_owned(), p.clone()]);
        }
        let mut verus = Vec::new();
        if let Some(m) = &self.module {
            verus.extend(["--verify-module".to_owned(), m.clone()]);
        }
        if let Some(f) = &self.function {
            if self.module.is_none() {
                verus.push("--verify-root".to_owned());
            }
            verus.extend(["--verify-function".to_owned(), f.clone()]);
        }
        verus.extend(self.verus_args.iter().cloned());
        if !verus.is_empty() {
            out.push("--".to_owned());
            out.extend(verus);
        }
        out
    }

    /// Whether the run covers only part of the project.
    #[must_use]
    pub fn scoped(&self) -> bool {
        self.package.is_some()
            || self.module.is_some()
            || self.function.is_some()
            || self.cargo_args.iter().chain(&self.verus_args).any(|a| {
                SCOPING_FLAGS
                    .iter()
                    .any(|f| a == f || a.starts_with(&format!("{f}=")))
            })
    }
}

/// Arguments of `counterexample` and `cached_result`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LookupArgs {
    /// The project directory (default: the server's working directory).
    pub project_dir: Option<String>,
    /// A function path, or a unique `::`-suffix of one.
    pub function: Option<String>,
    /// An obligation id (`<function>#<kind>/<n>`).
    pub obligation: Option<String>,
}

impl LookupArgs {
    /// The keys the lookups accept.
    pub const KEYS: &[&str] = &["project_dir", "function", "obligation"];

    /// Validates `args`. With `need_target`, one of `function` and
    /// `obligation` is required.
    ///
    /// # Errors
    ///
    /// A message for the model when an argument is unknown or invalid.
    pub fn parse(args: &Map<String, Value>, need_target: bool) -> Result<Self, String> {
        check_keys(args, Self::KEYS)?;
        let parsed = Self {
            project_dir: opt_str(args, "project_dir")?,
            function: opt_str(args, "function")?,
            obligation: opt_str(args, "obligation")?,
        };
        if let Some(o) = &parsed.obligation {
            let Some((f, rest)) = o.split_once('#') else {
                return Err(format!(
                    "`obligation` `{o}` is not an obligation id (`<function>#<kind>/<n>`, for example `demo::max#Function-Def/0`)"
                ));
            };
            if f.is_empty() || !rest.contains('/') {
                return Err(format!(
                    "`obligation` `{o}` is not an obligation id (`<function>#<kind>/<n>`)"
                ));
            }
            if parsed
                .function
                .as_deref()
                .is_some_and(|g| g != f && !f.ends_with(&format!("::{g}")))
            {
                return Err(format!("`obligation` `{o}` does not belong to `function`"));
            }
        }
        if need_target && parsed.function.is_none() && parsed.obligation.is_none() {
            return Err("give `function` or `obligation`".to_owned());
        }
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(v: &Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap_or_default()
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn verify_arguments_become_sail_arguments() {
        let a = RunArgs::parse(&map(&json!({}))).unwrap_or_default();
        assert_eq!(
            a.sail_args(&["verify", "--output", "json"]),
            strings(&["verify", "--output", "json"])
        );
        assert!(!a.scoped());
        let a = RunArgs::parse(&map(&json!({
            "project_dir": "/p", "package": "demo", "module": "arith", "function": "mul_pos",
            "cargo_args": ["--release"], "verus_args": ["--rlimit", "20"]
        })))
        .unwrap_or_default();
        assert_eq!(
            a.sail_args(&["verify"]),
            strings(&[
                "verify",
                "--release",
                "-p",
                "demo",
                "--",
                "--verify-module",
                "arith",
                "--verify-function",
                "mul_pos",
                "--rlimit",
                "20"
            ])
        );
        assert!(a.scoped());
        // A function without a module is looked up in the crate root.
        let a = RunArgs::parse(&map(&json!({"function": "max"}))).unwrap_or_default();
        assert_eq!(
            a.sail_args(&[]),
            strings(&["--", "--verify-root", "--verify-function", "max"])
        );
        let a =
            RunArgs::parse(&map(&json!({"verus_args": ["--verify-module=x"]}))).unwrap_or_default();
        assert!(a.scoped());
    }

    #[test]
    fn invalid_verify_arguments_are_rejected() {
        for (bad, why) in [
            (json!({"bogus": 1}), "unknown argument `bogus`"),
            (json!({"module": 3}), "must be a string"),
            (json!({"module": ""}), "must not be empty"),
            (json!({"function": "--all"}), "must not start with `-`"),
            (json!({"cargo_args": "x"}), "array of strings"),
            (json!({"verus_args": [1]}), "array of strings"),
            (json!({"cargo_args": ["--", "x"]}), "verus_args"),
            (json!({"cargo_args": ["--output=json"]}), "--output"),
        ] {
            let e = RunArgs::parse(&map(&bad)).err().unwrap_or_default();
            assert!(e.contains(why), "{bad}: {e}");
        }
    }

    #[test]
    fn lookup_arguments() {
        assert!(LookupArgs::parse(&map(&json!({})), false).is_ok());
        assert!(LookupArgs::parse(&map(&json!({})), true).is_err());
        let a = LookupArgs::parse(
            &map(&json!({"obligation": "demo::max#Function-Def/0"})),
            true,
        );
        assert_eq!(
            a.map(|a| a.obligation),
            Ok(Some("demo::max#Function-Def/0".to_owned()))
        );
        for bad in [
            json!({"obligation": "demo::max"}),
            json!({"obligation": "#Function-Def/0"}),
            json!({"obligation": "demo::max#x"}),
            json!({"function": "other", "obligation": "demo::max#Function-Def/0"}),
            json!({"function": 1}),
            json!({"module": "x", "function": "f"}),
        ] {
            assert!(LookupArgs::parse(&map(&bad), true).is_err(), "{bad}");
        }
        assert!(
            LookupArgs::parse(
                &map(&json!({"function": "max", "obligation": "demo::max#Function-Def/0"})),
                true
            )
            .is_ok()
        );
    }
}
