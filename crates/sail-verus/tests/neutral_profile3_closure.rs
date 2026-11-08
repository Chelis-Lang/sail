//! Explicit profile three: a crate root and its out-of-line module files.
use sail_verus::contracts::{
    ModuleSource, lower_closure, lower_profile, validate_closure_manifest,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

const ROOT: &str = include_str!("fixtures/neutral-contracts/closure/mod.rs");
const FILES: [(&str, &str); 3] = [
    (
        "rules",
        include_str!("fixtures/neutral-contracts/closure/rules.rs"),
    ),
    (
        "shapes",
        include_str!("fixtures/neutral-contracts/closure/shapes.rs"),
    ),
    (
        "shapes::measure",
        include_str!("fixtures/neutral-contracts/closure/shapes/measure.rs"),
    ),
];

#[allow(dead_code, reason = "The runtime closure keeps every fixture item.")]
#[path = "fixtures/neutral-contracts/closure/mod.rs"]
mod runtime_closure;

fn digest(text: &str) -> String {
    use sha2::Digest as _;
    format!("{:x}", sha2::Sha256::digest(text.as_bytes()))
}

fn module(name: &str, source: &str) -> ModuleSource {
    ModuleSource {
        module: name.into(),
        source: source.into(),
        source_sha256: digest(source),
    }
}

fn files() -> Vec<ModuleSource> {
    FILES
        .iter()
        .map(|(name, source)| module(name, source))
        .collect()
}

#[test]
fn profile_three_lowers_a_closure_to_one_bound_shadow() -> TestResult {
    let lowered = lower_closure(ROOT, &files(), "neutral", 3)?;
    let names = lowered
        .manifest
        .modules
        .iter()
        .map(|m| m.module.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["rules", "shapes", "shapes::measure"]);
    let root = digest(ROOT);
    assert_eq!(
        lowered.manifest.original_sha256,
        sail_api::contracts::closure_sha256(&root, &lowered.manifest.modules)
    );
    assert_ne!(lowered.manifest.original_sha256, root);
    let files_by_function = lowered
        .manifest
        .bindings
        .iter()
        .map(|b| (b.function.as_str(), b.module_file.as_deref()))
        .collect::<Vec<_>>();
    assert_eq!(
        files_by_function,
        [
            (
                "neutral::shapes::measure::half_perimeter",
                Some("shapes::measure")
            ),
            ("neutral::span", None),
            ("neutral::wide", None),
        ]
    );
    let source = lowered.source.replace(' ', "");
    assert!(source.contains("pubmodmeasure{usevstd::prelude::*;usesuper::Grid;"));
    assert!(source.contains("useself::shapes::Grid;"));
    assert!(source.contains("crate::rules::fits("));
    // A call across module files resolves through the module import.
    assert!(
        source.contains("usesuper::Grid;") && source.contains("{measure::half_perimeter(grid)}")
    );
    assert!(source.contains("crate::rules::__sail_defined_fits("));
    validate_closure_manifest(ROOT, &files(), &lowered.manifest)?;
    // Every module byte is bound by the manifest.
    let mut changed = files();
    changed[2].source.push('\n');
    changed[2].source_sha256 = digest(&changed[2].source);
    assert!(validate_closure_manifest(ROOT, &changed, &lowered.manifest).is_err());
    assert!(validate_closure_manifest(ROOT, &[], &lowered.manifest).is_err());
    Ok(())
}

type Case = (&'static str, String, Vec<ModuleSource>, u32);

fn refusal_cases() -> Vec<Case> {
    let all = files();
    let mut unsorted = files();
    unsorted.swap(0, 1);
    let mut wrong_digest = files();
    wrong_digest[0].source_sha256 = "0".repeat(64);
    vec![
        ("missing module file", ROOT.to_owned(), all[..2].to_vec(), 3),
        ("profile two", ROOT.to_owned(), all.clone(), 2),
        ("unsorted", ROOT.to_owned(), unsorted, 3),
        ("wrong digest", ROOT.to_owned(), wrong_digest, 3),
        (
            "undeclared extra file",
            ROOT.to_owned(),
            [all.clone(), vec![module("extra", "")]].concat(),
            3,
        ),
        (
            "path redirection",
            ROOT.replace("pub mod rules;", "#[path = \"rules.rs\"] pub mod rules;"),
            all.clone(),
            3,
        ),
        (
            "glob import",
            ROOT.replace("use self::shapes::Grid;", "use self::shapes::*;"),
            all.clone(),
            3,
        ),
        (
            "external import",
            ROOT.replace(
                "use self::shapes::Grid;",
                "use self::shapes::Grid; use std::cmp::max;",
            ),
            all.clone(),
            3,
        ),
        (
            "unresolved import",
            ROOT.replace(
                "use self::shapes::Grid;",
                "use self::shapes::Grid; use self::shapes::Absent;",
            ),
            all.clone(),
            3,
        ),
        (
            "predicate imported by name",
            ROOT.replace(
                "use self::shapes::Grid;",
                "use self::shapes::Grid; use self::rules::fits;",
            ),
            all.clone(),
            3,
        ),
        (
            "scalar-shadowing alias",
            ROOT.replace(
                "use self::shapes::Grid;",
                "use self::shapes::Grid; use self::shapes::Grid as u8;",
            ),
            all.clone(),
            3,
        ),
        (
            "enum inside a module file",
            ROOT.to_owned(),
            [
                all[..1].to_vec(),
                vec![module(
                    "shapes",
                    &format!(
                        "{}\n#[derive(Debug, PartialEq, Eq)] pub enum Side {{ Left, Right }}\n",
                        FILES[1].1
                    ),
                )],
                all[2..].to_vec(),
            ]
            .concat(),
            3,
        ),
        (
            "alias shadowing a local module",
            ROOT.replace(
                "use self::shapes::Grid;",
                "use self::shapes::Grid; use self::shapes as rules;",
            ),
            all.clone(),
            3,
        ),
        (
            "super at the crate root",
            ROOT.replace(
                "use self::shapes::Grid;",
                "use self::shapes::Grid; use super::Other;",
            ),
            all.clone(),
            3,
        ),
    ]
}

#[test]
fn closures_refuse_files_and_imports_outside_the_declared_closure() {
    for (label, root, modules, profile) in refusal_cases() {
        assert!(
            lower_closure(&root, &modules, "neutral", profile).is_err(),
            "{label}"
        );
    }
}

#[test]
fn renamed_imports_resolve_and_single_files_refuse_out_of_line_modules() {
    let all = files();
    // Renamed and module imports resolve through the alias table.
    let renamed = ROOT
        .replace(
            "pub mod shapes;",
            "pub mod shapes;\nuse crate::rules as checks;",
        )
        .replace("rules::fits(ret)", "checks::fits(ret)")
        .replace(
            "use self::shapes::Grid;",
            "use self::shapes::Grid as Board;",
        )
        .replace("grid: &Grid", "grid: &Board");
    assert!(lower_closure(&renamed, &all, "neutral", 3).is_ok());
    // A single file still refuses out-of-line modules in every profile.
    for profile in 1..=3 {
        assert!(lower_profile(ROOT, "neutral", profile).is_err());
    }
}

#[test]
fn single_file_submissions_and_manifests_keep_their_exact_bytes() -> TestResult {
    let source = include_str!("fixtures/neutral-contracts/order_service_profile2.rs");
    let lowered = lower_profile(source, "neutral", 2)?;
    let bytes = serde_json::to_string(&lowered.manifest)?;
    assert!(!bytes.contains("\"modules\"") && !bytes.contains("module_file"));
    assert_eq!(lowered, lower_closure(source, &[], "neutral", 2)?);
    let submission = sail_api::contracts::Submission {
        profile: 2,
        source: source.into(),
        source_sha256: lowered.manifest.original_sha256.clone(),
        crate_name: "neutral".into(),
        modules: Vec::new(),
    };
    assert!(!serde_json::to_string(&submission)?.contains("modules"));
    assert_eq!(submission.closure_sha256(), submission.source_sha256);
    let mut closure = submission;
    closure.source = ROOT.into();
    closure.source_sha256 = digest(ROOT);
    closure.modules = files();
    assert!(
        closure.validate().is_err(),
        "module files need profile three"
    );
    closure.profile = 3;
    closure.validate()?;
    closure.modules.reverse();
    assert!(closure.validate().is_err(), "module files must be sorted");
    Ok(())
}

#[test]
fn the_runtime_closure_compiles_as_ordinary_rust_modules() {
    let grid = runtime_closure::shapes::Grid {
        width: 256,
        height: 256,
    };
    assert_eq!(runtime_closure::span(&grid), 512);
    assert_eq!(runtime_closure::shapes::measure::half_perimeter(&grid), 512);
    assert!(runtime_closure::wide(&runtime_closure::Mode::Wide));
    assert!(!runtime_closure::wide(&runtime_closure::Mode::Tall));
    let too_wide = runtime_closure::shapes::Grid {
        width: 257,
        height: 0,
    };
    assert!(std::panic::catch_unwind(|| runtime_closure::span(&too_wide)).is_err());
}
