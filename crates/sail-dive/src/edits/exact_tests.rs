//! Transport checks for a reference to an earlier complete native result.
use super::*;
use std::collections::BTreeMap;

fn model() -> Request {
    Request {
        version: crate::VERSION,
        model: crate::Model {
            root: "A.tla".into(),
            config: "A.cfg".into(),
            files: BTreeMap::from([
                ("A.tla".into(), "---- MODULE A ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = 1\nInv == x < 2\n====\n".into()),
                ("A.cfg".into(), "INIT Init\nNEXT Next\nINVARIANT Inv".into()),
            ]),
        },
        operation: Operation::Check { max_states: 5, max_depth: 5 },
    }
}

fn fresh(request: &Request) -> Result<Entry, String> {
    Ok(Entry {
        elapsed_us: 0,
        request_sha256: crate::request_sha256(request).map_err(|e| e.to_string())?,
        evidence: Evidence::Fresh {
            response: Box::new(crate::execute(request)),
        },
    })
}

fn pair(first: &Request, next: Request, basis: usize) -> Result<(Batch, BatchResult), String> {
    let replay = Entry {
        elapsed_us: 0,
        request_sha256: crate::request_sha256(&next).map_err(|e| e.to_string())?,
        evidence: Evidence::ExactRequest { basis_entry: basis },
    };
    Ok((
        Batch {
            version: 1,
            wall_time_ms: 1000,
            checks: vec![first.clone(), next],
        },
        BatchResult {
            version: 1,
            entries: vec![fresh(first)?, replay],
        },
    ))
}

#[test]
fn replay_requires_earlier_complete_native_identity() -> Result<(), String> {
    let base = model();
    let (batch, result) = pair(&base, base.clone(), 0)?;
    result.validate(&batch)?;
    for basis in [1, usize::MAX] {
        let (batch, result) = pair(&base, base.clone(), basis)?;
        assert!(result.validate(&batch).is_err());
    }
    let mut changed = base.clone();
    changed
        .model
        .files
        .insert("Unused.tla".into(), "---- MODULE Unused ----\n====".into());
    let (batch, result) = pair(&base, changed, 0)?;
    assert!(result.validate(&batch).is_err());
    let mut limited = base.clone();
    limited.operation = Operation::Check {
        max_states: 1,
        max_depth: 5,
    };
    let (batch, result) = pair(&base, limited.clone(), 0)?;
    assert!(result.validate(&batch).is_err());
    let (batch, result) = pair(&limited, limited.clone(), 0)?;
    assert!(result.validate(&batch).is_err());
    let (mut batch, mut result) = pair(&base, base.clone(), 0)?;
    let (_, chain) = pair(&base, base.clone(), 1)?;
    batch.checks.push(base);
    result.entries.extend(chain.entries.into_iter().skip(1));
    assert!(result.validate(&batch).is_err());
    Ok(())
}

#[test]
fn excluded_or_damaged_basis_cannot_certify_a_repeat() -> Result<(), String> {
    let mut bad = model();
    let text = bad.model.files.get_mut("A.tla").ok_or("missing fixture")?;
    *text = text.replace("x < 2", "x = 0");
    let (batch, mut result) = pair(&bad, bad.clone(), 0)?;
    result.validate(&batch)?;
    let Evidence::Fresh { response } = &mut result.entries[0].evidence else {
        return Err("expected native basis".into());
    };
    assert_eq!(response.outcome, Outcome::Refuted);
    response.validations.clear();
    assert!(result.validate(&batch).is_err());

    let mut excluded = model();
    excluded
        .model
        .files
        .insert("A.cfg".into(), "this is unparseable".into());
    let (batch, result) = pair(&excluded, excluded.clone(), 0)?;
    assert!(result.validate(&batch).is_err());
    Ok(())
}
