//! Body-only mutation inventory controls; these do not claim backend executions.
use sail_api::contracts::Submission;
use sail_api::mutation::digest;
use sail_verus::contracts::mutation::plan;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn input(source: &str) -> Submission {
    Submission {
        profile: 1,
        source: source.into(),
        source_sha256: digest(source.as_bytes()),
        crate_name: "neutral".into(),
    }
}
const SOURCE: &str = "#[sail_contracts::satisfies(\"BOOL\")]\n#[sail_contracts::ensures(|ret| ret)]\npub fn enabled() -> bool { /* π */ true || false }\n";
#[test]
fn exact_utf8_body_only_inventory_is_deterministic_and_preserves_contracts() -> TestResult {
    let source = input(SOURCE);
    let first = plan(&source, 8)?;
    assert_eq!(first, plan(&source, 8)?);
    assert_eq!(first.candidates.len(), 2);
    assert_eq!(first.omitted, 0);
    for candidate in &first.candidates {
        assert!(
            candidate
                .source
                .source
                .starts_with(SOURCE.split('{').next().ok_or("prefix")?)
        );
        assert_eq!(candidate.source.source.rsplit('}').next(), Some("\n"));
        candidate.source.validate()?;
        assert_eq!(candidate.requirement_id, "BOOL");
        assert_eq!(
            candidate.guarantee_sha256,
            sail_verus::contracts::lower(SOURCE, "neutral")?
                .manifest
                .bindings[0]
                .guarantee_sha256
        );
    }
    Ok(())
}
#[test]
fn budgets_and_unsupported_return_shapes_remain_explicit() -> TestResult {
    let first = plan(&input(SOURCE), 1)?;
    assert_eq!(first.candidates.len(), 1);
    assert_eq!(first.omitted, 1);
    assert!(plan(&input(SOURCE), 0).is_err());
    assert!(plan(&input(SOURCE), 9).is_err());
    let composite = "#[derive(Clone,Copy)] struct Row { flag: bool }\n#[sail_contracts::satisfies(\"ROW\")] #[sail_contracts::ensures(|ret| ret.flag)] fn row()->Row { Row { flag:true } }";
    let unsupported = plan(&input(composite), 8)?;
    assert_eq!(unsupported.candidates.len(), 0);
    assert_eq!(unsupported.unsupported.len(), 1);
    Ok(())
}
