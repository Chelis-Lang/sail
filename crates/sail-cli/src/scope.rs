//! Restrict partial timing reports to the requested functions (VI-0009).

use sail_api::output::{FunctionReport, Report, Summary};

fn function_pattern(args: &[String]) -> Option<&str> {
    let mut it = args.iter().skip_while(|a| *a != "--").skip(1);
    let mut pattern = None;
    while let Some(a) = it.next() {
        if a == "--verify-function" {
            pattern = it.next().map(String::as_str);
        } else if let Some(value) = a.strip_prefix("--verify-function=") {
            pattern = Some(value);
        }
    }
    pattern
}

fn relative(f: &FunctionReport, krate: Option<&str>) -> String {
    let Some(krate) = krate else {
        return f.function.clone();
    };
    let prefix = if f.module.is_empty() {
        format!("{krate}::")
    } else {
        format!("{krate}::{}::", f.module)
    };
    f.function
        .strip_prefix(&prefix)
        .unwrap_or(&f.function)
        .to_owned()
}

fn strict(name: &str, pattern: &str) -> bool {
    let clean = pattern.trim_matches('*');
    match (pattern.starts_with('*'), pattern.ends_with('*')) {
        (true, true) => name.contains(clean),
        (true, false) => name.ends_with(clean),
        (false, true) => name.starts_with(clean),
        (false, false) => name == clean,
    }
}

/// Verus's timing success field does not distinguish skipped functions. Follow
/// its explicit selector, including exact-match priority and unique fallback.
pub(crate) fn restrict(report: &mut Report) {
    let Some(pattern) = function_pattern(&report.command).map(str::to_owned) else {
        return;
    };
    let mut removed = 0;
    for c in &mut report.crates {
        let names: Vec<_> = c
            .functions
            .iter()
            .map(|f| relative(f, c.name.as_deref()))
            .collect();
        let exact = names.iter().any(|n| strict(n, &pattern));
        let fallback =
            !pattern.contains('*') && names.iter().filter(|n| n.contains(&pattern)).count() == 1;
        let mut index = 0;
        c.functions.retain(|_| {
            let name = &names[index];
            index += 1;
            let keep = if exact {
                strict(name, &pattern)
            } else {
                fallback && name.contains(&pattern)
            };
            removed += usize::from(!keep);
            keep
        });
    }
    if removed > 0 {
        report.attribution.notes.push(format!(
            "{removed} timing entries outside the requested function selector omitted (VI-0009)"
        ));
    }
    report.summary = Summary::of(&report.crates);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(pattern: &str) -> Report {
        let mut r = crate::report::setup_failure(&[], "SAIL0113", "fixture");
        r.command = vec![
            "--".to_owned(),
            "--verify-function".to_owned(),
            pattern.to_owned(),
        ];
        r.crates = serde_json::from_value(serde_json::json!([{
            "name":"input", "verus":{"verified":1,"errors":0,"success":null,"encountered_vir_error":false},
            "functions": (["good", "good_helper", "bad"].map(|name| serde_json::json!({
                "function":format!("input::m::{name}"),"module":"m","fn_mode":"proof","verus_success":true,
                "verdict":"verified","establishment":"confirmed","evidence_tier":"smt_verified","unstable":false,"fragility":null,
                "timing":{"smt_micros":0,"rlimit":0}, "obligations":null,
                "provenance":{"mode":"default","source":"unrecorded","checks_solved":0,"checks_served":0,"checks_unrecorded":0}
            })))
        }])).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        r
    }

    #[test]
    fn skipped_successes_are_not_confirmed_and_exact_names_have_priority() {
        let mut r = report("good");
        restrict(&mut r);
        assert_eq!(r.summary.verified, 1);
        assert_eq!(r.crates[0].functions[0].function, "input::m::good");
        assert!(
            !r.crates[0]
                .functions
                .iter()
                .any(|f| f.function.ends_with("bad"))
        );
    }

    #[test]
    fn wildcards_unique_fallback_and_ambiguous_names_follow_verus() {
        for (pattern, expected) in [
            ("good*", 2),
            ("*helper", 1),
            ("*oo*", 2),
            ("helper", 1),
            ("oo", 0),
        ] {
            let mut r = report(pattern);
            restrict(&mut r);
            assert_eq!(r.crates[0].functions.len(), expected, "{pattern}");
        }
    }
}
