//! The private layer escapes caller-reserved tokens on every refusal path.
#![allow(clippy::unwrap_used, reason = "integration fixture assertions")]
use sail_acceptance::observation_arguments::{
    ObservationSettings, plan_observation_arguments, process_arguments, strip_observation_arguments,
};

fn requested() -> ObservationSettings {
    ObservationSettings::On {
        request: "/fresh/request".into(),
        directory: "/fresh/journal".into(),
    }
}

fn two_layers(
    original: &[String],
    settings: ObservationSettings,
) -> (Vec<String>, ObservationSettings) {
    let outer = plan_observation_arguments(original, settings);
    let (restored, held) = strip_observation_arguments(outer.arguments())
        .unwrap()
        .into_parts();
    assert_eq!(restored, original);
    let forwarded = plan_observation_arguments(&restored, held);
    strip_observation_arguments(forwarded.arguments())
        .unwrap()
        .into_parts()
}

#[test]
fn both_actual_shared_layers_restore_special_and_malformed_originals_exactly() {
    let cases: &[&[&str]] = &[
        &[],
        &["--help"],
        &["--verify-function"],
        &["--", "--help"],
        &["/toolchain/rustc", "--crate-name", "a", "源.rs"],
        &["--internal-test-mode", "x.rs"],
        &["--internal-trait-check", "x.rs"],
        &["--sail-observation-envelope-v1"],
        &["--sail-observation-envelope-v1", "off", "--help"],
        &[
            "--sail-observation-envelope-v1",
            "on",
            "/caller/request",
            "/caller/logs",
            "--help",
        ],
        &[
            "--sail-observation-envelope-v1",
            "off",
            "--sail-observation-envelope-v1",
            "on",
            "/r",
            "/d",
        ],
        &["--sail-observation-request=/caller/request", "--help"],
        &["--", "--sail-observation-dir", "/caller/logs"],
    ];
    for case in cases {
        let original: Vec<String> = case.iter().map(|s| (*s).to_owned()).collect();
        for settings in [requested(), ObservationSettings::Off] {
            let (restored, _) = two_layers(&original, settings);
            assert_eq!(restored, original);
            let process = process_arguments("original-program", &restored);
            assert_eq!(
                process.first().map(String::as_str),
                Some("original-program")
            );
            assert_eq!(&process[1..], original);
        }
    }
}

#[test]
fn refusal_still_has_one_off_layer_and_leaves_original_prefix_unconsumed() {
    let original = vec![
        "--sail-observation-envelope-v1".into(),
        "on".into(),
        "/r".into(),
        "/d".into(),
        "--help".into(),
    ];
    let plan = plan_observation_arguments(&original, requested());
    assert!(matches!(plan.settings(), ObservationSettings::Off));
    assert_eq!(
        &plan.arguments()[..2],
        ["--sail-observation-envelope-v1", "off"]
    );
    let (restored, held) = two_layers(&original, requested());
    assert_eq!(restored, original);
    assert!(matches!(held, ObservationSettings::Off));
    // Exactly one layer has been consumed. Passing the result straight to
    // ordinary parsing preserves the original unknown option, not help.
    assert_eq!(restored[0], "--sail-observation-envelope-v1");
}

#[test]
fn on_paths_are_exact_and_io_failure_off_authorizes_no_capture() {
    let original = vec!["x.rs".into(), "--verify-function".into()];
    let (_, held) = two_layers(&original, requested());
    assert!(matches!(held, ObservationSettings::On { .. }));
    if let ObservationSettings::On { request, directory } = held {
        assert_eq!(request, "/fresh/request");
        assert_eq!(directory, "/fresh/journal");
    }
    for (request, directory) in [("", "/d"), ("-option", "/d"), ("/r", "bad\0path")] {
        let (restored, held) = two_layers(
            &original,
            ObservationSettings::On {
                request: request.into(),
                directory: directory.into(),
            },
        );
        assert_eq!(restored, original);
        assert!(matches!(held, ObservationSettings::Off));
    }
    let (restored, held) = two_layers(&original, ObservationSettings::Off);
    assert_eq!(restored, original);
    assert!(matches!(held, ObservationSettings::Off));
}
