//! Native differential fixtures for the actual `request_targets` operations.

use crate::request_targets::{
    OriginalRequestHeader, OriginalTargets, construct_original_request, insert_original_target,
    project_original_targets,
};
use std::collections::BTreeSet;

#[test]
fn actual_dictionary_projection_retains_old_order_scope_and_duplicates()
-> Result<(), Box<dyn std::error::Error>> {
    let original: Vec<(String, String, String)> = vec![
        ("package-b".into(), "sibling".into(), "bin".into()),
        ("package-a".into(), "original".into(), "lib".into()),
        ("package-a".into(), "original".into(), "bin".into()),
        ("package-a".into(), "original".into(), "lib".into()),
        ("λ".into(), "target\0".into(), "lib".into()),
    ];
    let mut source = OriginalTargets::new();
    for original in &original {
        insert_original_target(&mut source, original.clone());
    }
    let projected = project_original_targets(&source).ok_or("original set projection refused")?;
    let actual: Vec<_> = projected
        .into_iter()
        .map(|target| (target.package_id, target.target, target.kind))
        .collect();
    let expected: BTreeSet<_> = original.into_iter().collect();
    assert_eq!(actual, expected.into_iter().collect::<Vec<_>>());
    assert_eq!(
        project_original_targets(&OriginalTargets::new())
            .ok_or("empty original set projection refused")?,
        Vec::<crate::project::RequestTarget>::new()
    );
    Ok(())
}

/// Actual construction data only: the borrowed header is not a proof-state
/// token, and compile-only remains compile-only after original-set projection.
#[test]
fn actual_original_request_preserves_nonempty_header_and_domain()
-> Result<(), Box<dyn std::error::Error>> {
    let arguments = vec![
        "--package".into(),
        "original".into(),
        "--".into(),
        "--no-verify".into(),
    ];
    let mut original = OriginalTargets::new();
    insert_original_target(
        &mut original,
        ("package-b".into(), "sibling".into(), "lib".into()),
    );
    insert_original_target(
        &mut original,
        ("package-a".into(), "original".into(), "bin".into()),
    );
    let request = construct_original_request(
        OriginalRequestHeader {
            run_id: "original-run",
            invocation: "original-invocation",
            workspace: "original-workspace",
            context: "original-context",
            revision: "original-revision",
            arguments: &arguments,
            engine: Some(crate::project::EngineIdentity {
                name: "original-engine".into(),
                manifest_sha256: "original-engine-digest".into(),
            }),
            function: Some("original-function"),
            repair_package: "original-package",
            repair_target: Some("original-binary"),
            repair_kind: "bin",
        },
        &original,
    )
    .ok_or("original request construction refused")?;
    assert_eq!(request.run_id, "original-run");
    assert_eq!(request.invocation, "original-invocation");
    assert_eq!(request.workspace, "original-workspace");
    assert_eq!(request.context_sha256, "original-context");
    assert_eq!(request.input_revision, "original-revision");
    assert_eq!(request.args, arguments);
    let engine = request.engine.as_ref().ok_or("original engine missing")?;
    assert_eq!(engine.name, "original-engine");
    assert_eq!(engine.manifest_sha256, "original-engine-digest");
    assert_eq!(request.function.as_deref(), Some("original-function"));
    assert_eq!(request.repair_package, "original-package");
    assert_eq!(request.repair_target.as_deref(), Some("original-binary"));
    assert_eq!(request.repair_kind, "bin");
    assert!(!crate::acceptance::verification_enabled_command(
        &request.args
    ));
    let actual: Vec<_> = request
        .targets
        .into_iter()
        .map(|target| (target.package_id, target.target, target.kind))
        .collect();
    assert_eq!(
        actual,
        vec![
            ("package-a".into(), "original".into(), "bin".into()),
            ("package-b".into(), "sibling".into(), "lib".into()),
        ]
    );
    Ok(())
}
