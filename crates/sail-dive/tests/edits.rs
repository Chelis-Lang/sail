//! Compare the checked editing API with fresh closed native jobs.
use sail_dive::{
    Outcome, Request,
    edits::{Checked, Evidence, Session, Structure},
};
use std::error::Error;
type Test = Result<(), Box<dyn Error>>;

fn check(source: &str, cfg: &str, max_states: u64) -> Request {
    Request {
        version: sail_dive::VERSION,
        model: sail_dive::Model {
            root: "A.tla".into(),
            config: "A.cfg".into(),
            files: [
                ("A.tla".into(), source.into()),
                ("A.cfg".into(), cfg.into()),
            ]
            .into(),
        },
        operation: sail_dive::Operation::Check {
            max_states,
            max_depth: 30,
        },
    }
}

fn observed(request: &Request, session: &mut Session, expected: Option<(Structure, bool)>) -> Test {
    let fresh = sail_dive::execute(request);
    sail_dive::validate_response(request, &fresh)?;
    let entry = session.check(request)?;
    match &entry.evidence {
        Evidence::Fresh { response } => {
            assert_eq!(expected, None);
            assert_eq!(response.outcome, fresh.outcome);
            assert_eq!(
                response.native_report["verdict"],
                fresh.native_report["verdict"]
            );
        }
        Evidence::Checked {
            structure,
            reused,
            decision,
            decision_counts,
            ..
        } => {
            assert_eq!(expected, Some((*structure, *reused)));
            assert_eq!(
                decision_counts.generated,
                fresh.native_report["statistics"]["generated"]
            );
            assert_eq!(
                decision_counts.distinct,
                fresh.native_report["statistics"]["distinct"]
            );
            assert_eq!(
                decision_counts.depth,
                fresh.native_report["statistics"]["depth"]
            );
            match decision {
                Checked::Holds => assert_eq!(fresh.outcome, Outcome::Holds),
                Checked::Refuted { validation, trace } => {
                    assert_eq!(fresh.outcome, Outcome::Refuted);
                    assert_eq!(trace.as_ref(), &fresh.itf_traces[0]);
                    assert_eq!(validation.outcome, Outcome::Valid);
                    assert_eq!(validation.validations[0]["result"], "valid");
                }
            }
        }
    }
    Ok(())
}

const SAFETY: &str = "---- MODULE A ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == IF x = 0 THEN x' \\in {1, 2} ELSE IF x = 2 THEN x' = 3 ELSE x' = x\nInv == x <= 3\n====\n";
const CFG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\n";

#[test]
fn real_ordered_discovery_and_refutations() -> Test {
    let mut s = Session::default();
    let original = check(SAFETY, CFG, 20);
    observed(&original, &mut s, Some((Structure::SafetyRows, false)))?;
    let bad = SAFETY.replace("x <= 3", "x /= 3");
    observed(
        &check(&bad, CFG, 20),
        &mut s,
        Some((Structure::SafetyRows, true)),
    )?;
    observed(
        &check(&SAFETY.replace("x <= 3", "x < 9"), CFG, 20),
        &mut s,
        Some((Structure::SafetyRows, true)),
    )?;
    let mut prefix = Session::default();
    observed(
        &check(&bad, CFG, 20),
        &mut prefix,
        Some((Structure::SafetyRows, false)),
    )?;
    observed(
        &check(&bad.replace("x /= 3", "x < 3"), CFG, 20),
        &mut prefix,
        Some((Structure::SafetyRows, true)),
    )?;
    let sibling_error = bad.replace("x /= 3", "IF x = 1 THEN 1 ELSE x /= 3");
    observed(&check(&sibling_error, CFG, 20), &mut prefix, None)?;
    observed(
        &check(&bad.replace("{1, 2}", "{1,2,4,5,6}"), CFG, 3),
        &mut prefix,
        None,
    )?;
    let mut expanded = original;
    expanded
        .model
        .files
        .insert("B.tla".into(), "---- MODULE B ----\n====\n".into());
    observed(&expanded, &mut s, Some((Structure::SafetyRows, false)))
}

#[test]
fn real_graph_property_edits_and_deadline_bounds() -> Test {
    let mut s = Session::default();
    let tla = "---- MODULE A ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = 1\nSpec == Init /\\ [][Next]_x\nP == <>(x = 0)\nQ == <>(x < 2)\n====\n";
    let cfg = "SPECIFICATION Spec\nPROPERTY P Q\n";
    observed(
        &check(tla, cfg, 10),
        &mut s,
        Some((Structure::TemporalGraph, false)),
    )?;
    let edited = tla.replace("Q == <>(x < 2)", "Q == <>(x = 2)");
    observed(
        &check(&edited, cfg, 10),
        &mut s,
        Some((Structure::TemporalGraph, true)),
    )?;
    observed(&check(&edited, cfg, 1), &mut s, None)
}

#[test]
fn direct_embedding_preserves_ordinary_checker_overrides() -> Test {
    const CHILD: &str = "SAIL_TEST_EDIT_CORE_OVERRIDE";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "direct_embedding_preserves_ordinary_checker_overrides",
            ])
            .env(CHILD, "1")
            .env("DIVE_CORE_OPT", "-symbolic-member")
            .status()?;
        assert!(status.success());
        return Ok(());
    }
    let source = "---- MODULE A ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nInv == x \\in 0..100000000\n====\n";
    let current = check(source, CFG, 20);
    assert_eq!(sail_dive::execute(&current).outcome, Outcome::Unsupported);
    observed(&current, &mut Session::default(), None)
}
