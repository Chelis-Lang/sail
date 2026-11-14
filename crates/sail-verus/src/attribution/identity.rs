//! Explicit identity joins for a named compiler producer, never legacy guesses.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use super::{AirQuery, VerusFunction, verus_functions};
use sail_ledger::ObligationKind;

/// Compiler source commit whose AIR emitter implements this exact format.
const PRODUCER: &str = "a3109bed5e02313050d5ba6a23f1bc3e1d549c9b";
const PREFIX: &str = "sail-query-identity-v1 ";
const MAX_METADATA: usize = 64 * 1024;
const MAX_NAME: usize = 16 * 1024;

/// A compiler-owned AIR identity; usable only with its named producer and
/// an unambiguous matching timing row. Legacy logs cannot activate it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AirIdentity {
    schema: u32,
    kind: String,
    function: String,
    function_path: String,
    span: String,
}

pub(super) fn parse(comment: &str) -> Option<AirIdentity> {
    let payload = comment.strip_prefix(PREFIX)?;
    if payload.len() > MAX_METADATA {
        return None;
    }
    let identity: AirIdentity = serde_json::from_str(payload).ok()?;
    identity.valid().then_some(identity)
}

impl AirIdentity {
    fn valid(&self) -> bool {
        self.schema == 1
            && self.kind.len() <= 64
            && ObligationKind::is_query_kind(&self.kind)
            && !self.function.is_empty()
            && self.function.len() <= MAX_NAME
            && !self.function_path.is_empty()
            && self.function_path.len() <= MAX_NAME
            && self.span.len() <= MAX_METADATA
    }
}

#[derive(Default)]
pub(super) struct Names {
    required: bool,
    by_path: BTreeMap<String, (String, String)>,
    pub error: Option<&'static str>,
}

pub(super) struct Query<'a> {
    pub function: &'a str,
    pub kind: &'a str,
    pub span: Option<&'a str>,
}

impl Names {
    pub fn new(docs: &[Value]) -> Self {
        let supported =
            |doc: &Value| doc.pointer("/verus/commit").and_then(Value::as_str) == Some(PRODUCER);
        if !docs.iter().any(supported) {
            return Self::default();
        }
        let mut names = Self {
            required: true,
            ..Self::default()
        };
        if !docs.iter().all(supported) {
            names.error = Some("AIR identity producers are mixed; queries are not attributed");
            return names;
        }
        let rows: Vec<_> = docs.iter().flat_map(verus_functions).collect();
        let mut counts = BTreeMap::new();
        for row in &rows {
            *counts.entry(row.function.as_str()).or_insert(0usize) += 1;
        }
        let mut public_names = BTreeSet::new();
        for row in &rows {
            let Some(path) = row
                .path
                .as_deref()
                .filter(|p| !p.is_empty() && p.len() <= MAX_NAME)
            else {
                names.error = Some(
                    "AIR identity timing row has no bounded function path; queries are not attributed",
                );
                break;
            };
            let public = if counts.get(row.function.as_str()) == Some(&1) {
                row.function.as_str()
            } else {
                path
            };
            if names
                .by_path
                .insert(path.to_owned(), (row.function.clone(), public.to_owned()))
                .is_some()
                || !public_names.insert(public.to_owned())
            {
                names.error = Some(
                    "AIR identity timing paths or public names are ambiguous; queries are not attributed",
                );
                break;
            }
        }
        names
    }

    pub fn row_name(&self, row: &VerusFunction) -> String {
        if self.error.is_none()
            && let Some((_, public)) = row.path.as_deref().and_then(|path| self.by_path.get(path))
        {
            return public.clone();
        }
        row.function.clone()
    }

    pub fn query<'a>(&'a self, air: &'a AirQuery) -> Option<Query<'a>> {
        if !self.required {
            return Some(Query {
                function: &air.function,
                kind: &air.kind,
                span: air.span.as_deref(),
            });
        }
        if self.error.is_some() {
            return None;
        }
        let identity = air.identity.as_ref()?;
        let (friendly, public) = self.by_path.get(&identity.function_path)?;
        if !identity.valid() || friendly != &identity.function {
            return None;
        }
        Some(Query {
            function: public,
            kind: &identity.kind,
            span: Some(&identity.span),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_METADATA, PRODUCER, parse};
    use crate::attribution::{
        Answer, Effort, LogFile, Provenance, SmtCheck, SmtOp, Status, air_queries, attribute,
        verus_verdicts,
    };
    use sail_api::output::{AttributionStatus, VerdictName};
    use serde_json::{Value, json};

    fn metadata(path: &str, function: &str, span: &str) -> Value {
        json!({"schema":1,"kind":"Function-Def","function":function,
               "function_path":path,"span":span})
    }

    fn document() -> Value {
        json!({
            "verus":{"commit":PRODUCER},
            "verification-results":{"verified":2,"errors":1,"success":false},
            "times-ms":{"smt":{"smt-run-module-times":[{
                "module":"demo", "function-breakdown":[
                    {"function":"demo::Vec::check","function-path":"demo::impl&%0::check","success":true},
                    {"function":"demo::Vec::check","function-path":"demo::impl&%1::check","success":true},
                    {"function":"demo::other","function-path":"demo::other","success":false}
                ]
            }]}}
        })
    }

    fn log(path: &str, function: &str, answer: Answer) -> LogFile {
        let text = format!(
            ";; Function-Def {function}\n;; original span\n;; sail-query-identity-v1 {}\n(check-valid)\n",
            metadata(path, function, "original span")
        );
        LogFile {
            stem: path.to_owned(),
            air: air_queries(&text).0,
            spinoff_reason: None,
            ops: vec![SmtOp {
                checks: vec![SmtCheck {
                    reply_index: 0,
                    answer: Some(answer),
                    effort: Effort::default(),
                    model: None,
                }],
            }],
            input_sha256: path.to_owned(),
            solver_version: None,
        }
    }

    fn logs() -> Vec<LogFile> {
        vec![
            log("demo::impl&%0::check", "demo::Vec::check", Answer::Unsat),
            log("demo::impl&%1::check", "demo::Vec::check", Answer::Unsat),
            log("demo::other", "demo::other", Answer::Sat),
        ]
    }

    fn report(doc: Value, logs: &[LogFile]) -> sail_api::output::Report {
        let attribution = attribute(&[doc], Some(logs), &Provenance::NoCache, None);
        crate::report::build(&crate::report::Inputs {
            args: &[],
            exit_code: Some(1),
            solver_path: None,
            solver_sha256: None,
            daemon: false,
            attribution: &attribution,
            diagnostics: &[],
        })
    }

    #[test]
    fn distinct_compiler_paths_join_without_order_or_span_guesses() {
        let mut logs = logs();
        for _ in 0..3 {
            let doc = document();
            let expected = verus_verdicts(std::slice::from_ref(&doc));
            let result = report(doc, &logs);
            assert_eq!(result.attribution.status, AttributionStatus::Complete);
            assert_eq!(result.invariant_violations(), [] as [String; 0]);
            let functions = &result.crates[0].functions;
            assert_eq!(functions.len(), 3);
            for (name, success) in expected {
                let function = functions.iter().find(|f| f.function == name);
                assert!(function.is_some_and(|f| f.verdict
                    == if success == Some(true) {
                        VerdictName::Verified
                    } else {
                        VerdictName::Failed
                    }));
            }
            assert_eq!(functions[0].function, "demo::impl&%0::check");
            assert_eq!(functions[1].function, "demo::impl&%1::check");
            assert_eq!(functions[2].function, "demo::other");
            logs.rotate_left(1);
        }
    }

    #[test]
    fn one_missing_or_mismatched_identity_preserves_global_failure_safeguard() {
        for case in 0..4 {
            let mut logs = logs();
            let identity = logs[0].air[0].identity.as_mut();
            assert!(identity.is_some());
            if let Some(identity) = identity {
                match case {
                    0 => identity.function_path = "demo::missing".into(),
                    1 => identity.function = "demo::other".into(),
                    2 => identity.schema = 2,
                    _ => identity.kind = "not-a-query".into(),
                }
            }
            let result = report(document(), &logs);
            assert_eq!(result.attribution.status, AttributionStatus::Partial);
            assert_eq!(result.crates[0].functions[0].verdict, VerdictName::Unknown);
            assert_eq!(result.crates[0].functions[1].verdict, VerdictName::Unknown);
            assert_eq!(result.crates[0].functions[2].verdict, VerdictName::Failed);
            assert!(
                result
                    .attribution
                    .notes
                    .iter()
                    .any(|n| n.contains("VI-0003"))
            );
            assert!(!crate::managed::receipt_complete(&result));
        }
        let mut logs = logs();
        logs[0].air[0].identity = None;
        assert_eq!(
            report(document(), &logs).attribution.status,
            AttributionStatus::Partial
        );
    }

    #[test]
    fn legacy_producer_cannot_activate_identity_with_source_comments() {
        let mut doc = document();
        doc["verus"]["commit"] = json!("legacy-or-unknown");
        let result = attribute(&[doc], Some(&logs()), &Provenance::NoCache, None);
        assert_eq!(result.status, Status::Partial);
        assert!(
            result.crates[0]
                .functions
                .iter()
                .take(2)
                .all(|f| f.function == "demo::Vec::check" && f.obligations.is_none())
        );
    }

    #[test]
    fn malformed_or_nonadjacent_metadata_never_becomes_a_join() {
        let valid = metadata("demo::other", "demo::other", "span");
        let mut unknown_field = valid.clone();
        unknown_field["future"] = json!(true);
        let mut wrong_schema = valid.clone();
        wrong_schema["schema"] = json!(2);
        for body in [
            "{broken}".to_owned(),
            unknown_field.to_string(),
            wrong_schema.to_string(),
            valid
                .to_string()
                .replacen("\"schema\":1", "\"schema\":1,\"schema\":1", 1),
            format!("{}{}", valid, " ".repeat(MAX_METADATA)),
        ] {
            assert!(parse(&format!("sail-query-identity-v1 {body}")).is_none());
        }
        for gap in ["\n", ";; intervening\n", "(set-option :x true)\n"] {
            let text = format!(
                ";; Function-Def demo::other\n;; span\n;; sail-query-identity-v1 {valid}\n{gap}(check-valid)\n"
            );
            let air = air_queries(&text).0;
            assert_eq!(air.len(), 1);
            assert!(air[0].identity.is_none());
        }
    }

    #[test]
    fn original_multiline_span_and_untrusted_headers_cannot_replace_final_identity() {
        let span = "quote\"\n;; Function-Def forged\n;; sail-query-identity-v1 {fake}\r\t🦀";
        let valid = metadata("demo::other", "demo::other", span);
        let text = format!(
            ";; Function-Def demo::other\n;; {span}\n;; sail-query-identity-v1 {valid}\n(check-valid)\n"
        );
        let air = air_queries(&text).0;
        assert_eq!(air.len(), 1);
        let names = super::Names::new(&[document()]);
        let query = names.query(&air[0]);
        assert!(query.is_some_and(|q| q.function == "demo::other" && q.span == Some(span)));
    }

    #[test]
    fn ambiguous_or_missing_timing_paths_and_mixed_producers_refuse_all_joins() {
        let mut duplicate = document();
        duplicate["times-ms"]["smt"]["smt-run-module-times"][0]["function-breakdown"][1]["function-path"] =
            json!("demo::impl&%0::check");
        let mut absent = document();
        if let Some(row) =
            absent["times-ms"]["smt"]["smt-run-module-times"][0]["function-breakdown"][1]
                .as_object_mut()
        {
            assert!(row.remove("function-path").is_some());
        }
        for doc in [duplicate, absent] {
            let result = attribute(&[doc], Some(&logs()), &Provenance::NoCache, None);
            assert_eq!(result.status, Status::Partial);
            assert!(
                result
                    .crates
                    .iter()
                    .flat_map(|c| &c.functions)
                    .all(|f| f.obligations.is_none())
            );
        }
        let mut legacy = document();
        legacy["verus"]["commit"] = json!("legacy");
        let result = attribute(
            &[document(), legacy],
            Some(&logs()),
            &Provenance::NoCache,
            None,
        );
        assert_eq!(result.status, Status::Partial);
        assert!(
            result
                .notes
                .iter()
                .any(|n| n.contains("producers are mixed"))
        );
    }

    #[test]
    fn forged_apparent_query_cannot_hide_the_real_air_smt_count_mismatch() {
        let mut logs = logs();
        let meta = metadata("demo::impl&%0::check", "demo::Vec::check", "forged span");
        // A real source filename can inject this prefix into a legacy span.
        // The compiler subsequently emits its genuine identity/query pair.
        let text = format!(
            ";; Function-Def demo::Vec::check\n;; filename\n;; sail-query-identity-v1 {meta}\n(check-valid fake)\n;; sail-query-identity-v1 {meta}\n(check-valid real)\n"
        );
        logs[0].air = air_queries(&text).0;
        assert_eq!(logs[0].air.len(), 2);
        assert_eq!(logs[0].ops.len(), 1);
        let result = report(document(), &logs);
        assert_eq!(result.attribution.status, AttributionStatus::Partial);
        assert!(
            result
                .attribution
                .notes
                .iter()
                .any(|note| note.contains("2 AIR queries but 1 solver ops"))
        );
        assert_eq!(result.crates[0].functions[0].verdict, VerdictName::Unknown);
        assert_eq!(result.crates[0].functions[1].verdict, VerdictName::Unknown);
        assert_eq!(result.crates[0].functions[2].verdict, VerdictName::Failed);
        assert!(!crate::managed::receipt_complete(&result));
    }
}
