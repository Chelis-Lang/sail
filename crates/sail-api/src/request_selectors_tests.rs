//! Ordinary differential fixtures retain the native scan's original domain.
use super::request_selectors::{
    ArgumentSpan, OriginalSelectors, original_package_matches, original_span_matches_str,
    scan_original_selectors,
};

fn original_values<'a>(arguments: &'a [String], spans: &[ArgumentSpan]) -> Option<Vec<&'a str>> {
    spans
        .iter()
        .map(|span| arguments.get(span.argument)?.get(span.offset..))
        .collect()
}

// This is the original pre-execution scanner, independently retained as the
// native differential oracle; no result-selected expectation list is involved.
fn previous_scan(arguments: &[String]) -> (Vec<&str>, Vec<&str>, Vec<&str>, bool, bool) {
    let mut packages = Vec::new();
    let mut excluded = Vec::new();
    let mut bins = Vec::new();
    let mut library = false;
    let mut all_bins = false;
    let mut it = arguments.iter().take_while(|a| a.as_str() != "--");
    while let Some(a) = it.next() {
        match a.as_str() {
            "-p" | "--package" => {
                if let Some(v) = it.next() {
                    packages.push(v.as_str());
                }
            }
            "--exclude" => {
                if let Some(v) = it.next() {
                    excluded.push(v.as_str());
                }
            }
            "--bin" => {
                if let Some(v) = it.next() {
                    bins.push(v.as_str());
                }
            }
            "--lib" => library = true,
            "--bins" => all_bins = true,
            _ => {
                if let Some(v) = a.strip_prefix("--package=") {
                    packages.push(v);
                } else if let Some(v) = a.strip_prefix("-p") {
                    packages.push(v.strip_prefix('=').unwrap_or(v));
                }
                if let Some(v) = a.strip_prefix("--exclude=") {
                    excluded.push(v);
                }
                if let Some(v) = a.strip_prefix("--bin=") {
                    bins.push(v);
                }
            }
        }
    }
    (packages, excluded, bins, library, all_bins)
}

fn check_original_scan(arguments: &[String], selection: &OriginalSelectors) {
    let previous = previous_scan(arguments);
    assert_eq!(
        original_values(arguments, &selection.packages),
        Some(previous.0)
    );
    assert_eq!(
        original_values(arguments, &selection.excluded),
        Some(previous.1)
    );
    assert_eq!(
        original_values(arguments, &selection.bins),
        Some(previous.2)
    );
    assert_eq!(selection.library, previous.3);
    assert_eq!(selection.all_bins, previous.4);
    assert_eq!(
        selection.prefix_end,
        arguments
            .iter()
            .position(|a| a == "--")
            .unwrap_or(arguments.len())
    );
}

#[test]
fn original_selector_scan_preserves_native_values_and_consumption() {
    let cases: &[&[&str]] = &[
        &[],
        &["-p"],
        &["--package"],
        &["--exclude"],
        &["--bin"],
        &["-p", "--lib", "--bins"],
        &["--bin", "--exclude", "--lib"],
        &["--exclude", "--bin", "--bins"],
        &["-p", "--", "--lib"],
        &["--", "-p", "member", "--lib"],
        &["--lib", "--bin=π\0tool", "--bins"],
        &[
            "--package=π\0member",
            "-p=π\0member",
            "-pπ\0member",
            "--exclude=π\0member",
        ],
        &["--package=", "-p=", "-p", "", "--exclude=", "--bin="],
        &[
            "--unknown",
            "-pfirst",
            "--package",
            "second",
            "-pfirst",
            "--bin",
            "tool",
        ],
    ];
    for case in cases {
        let arguments: Vec<String> = case.iter().map(|arg| (*arg).to_owned()).collect();
        let original = arguments.clone();
        let selection = scan_original_selectors(&arguments);
        check_original_scan(&arguments, &selection);
        assert_eq!(arguments, original);
    }
    let arguments: Vec<String> = ["-pπ", "--package", "member", "-p=last"]
        .map(str::to_owned)
        .into();
    let selection = scan_original_selectors(&arguments);
    let positions: Vec<_> = selection
        .packages
        .iter()
        .map(|span| (span.argument, span.offset))
        .collect();
    assert_eq!(positions, [(0, 2), (2, 0), (3, 3)]);
}

#[test]
fn original_package_match_preserves_first_at_and_exact_native_bytes() {
    let specs = [
        "member",
        "path+file:///π#member@1",
        "member@1",
        "member@1@2",
        "π\0member",
        "",
        "@1",
    ];
    let candidates = [
        ("member", "path+file:///π#member@1", Some("1")),
        ("member", "other", Some("1@2")),
        ("π\0member", "other", None),
        ("", "other", Some("1")),
        ("member", "other", None),
    ];
    for spec in specs {
        let arguments = vec![format!("-p={spec}")];
        let selection = scan_original_selectors(&arguments);
        assert_eq!(selection.packages.len(), 1);
        for span in &selection.packages {
            for (name, id, version) in candidates {
                let expected = spec == name
                    || spec == id
                    || spec
                        .split_once('@')
                        .is_some_and(|(n, v)| n == name && version == Some(v));
                assert_eq!(
                    original_package_matches(&arguments, span, name, id, version),
                    expected
                );
                assert_eq!(
                    original_span_matches_str(&arguments, span, name),
                    spec == name
                );
            }
        }
    }
    let arguments = vec!["member".to_owned()];
    assert!(!original_package_matches(
        &arguments,
        &ArgumentSpan {
            argument: 1,
            offset: 0
        },
        "member",
        "id",
        Some("1")
    ));
    assert!(!original_span_matches_str(
        &arguments,
        &ArgumentSpan {
            argument: 0,
            offset: 7
        },
        "member"
    ));
}
