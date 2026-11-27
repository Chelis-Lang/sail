//! Round trips, the two mapping tables and the no-upgrade rule.
use super::*;

const NO_CLAIM: [Outcome; 7] = [
    Outcome::Timeout,
    Outcome::OutOfMemory,
    Outcome::Stopped,
    Outcome::Crash,
    Outcome::Cancelled,
    Outcome::Unsupported,
    Outcome::BuildFailure,
];

fn parts(subject: Subject, claim: Claim) -> Parts {
    Parts {
        subject,
        outcome: claim.outcome,
        tier: claim.outcome.tier(),
        strength: claim.strength,
        bounds: Vec::new(),
        sufficiency: claim.sufficiency,
        assumptions: Vec::new(),
        checks: BTreeMap::new(),
        witness: None,
        tools: vec![Tool {
            name: "tool".into(),
            version: Some("1".into()),
            sha256: None,
        }],
        integrity: Integrity::Supervised,
        provenance: Provenance::Fresh,
        cost: Cost::default(),
    }
}

fn harness() -> Subject {
    Subject {
        backend: Backend::Kani,
        kind: SubjectKind::Harness,
        id: "pilot::handwritten".into(),
    }
}

fn model() -> Subject {
    Subject {
        backend: Backend::Dive,
        kind: SubjectKind::Model,
        id: "MCKVS.tla".into(),
    }
}

fn round_trip(parts: Parts) -> Result<String, String> {
    let evidence = Evidence::new(parts)?;
    let text = serde_json::to_string(&evidence).map_err(|e| e.to_string())?;
    let back: Evidence = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    assert_eq!(back, evidence);
    assert!(!text.contains("verified"), "{text}");
    Ok(text)
}

#[test]
fn kani_outcomes_map_and_round_trip() -> Result<(), String> {
    let table = [
        (
            KaniOutcome::BoundedSuccess,
            Outcome::Success,
            Some(Strength::Bounded),
            Sufficiency::NotEstablished,
        ),
        (
            KaniOutcome::Counterexample,
            Outcome::Counterexample,
            Some(Strength::Bounded),
            Sufficiency::NotEstablished,
        ),
        (
            KaniOutcome::BoundInsufficient,
            Outcome::BoundInsufficient,
            None,
            Sufficiency::Insufficient,
        ),
    ];
    for (native, outcome, strength, sufficiency) in table {
        let claim = kani_claim(native);
        assert_eq!(
            claim,
            Claim {
                outcome,
                strength,
                sufficiency
            }
        );
        assert_eq!(outcome.tier().is_some(), strength.is_some());
        round_trip(parts(harness(), claim))?;
    }
    Ok(())
}

#[test]
fn every_dive_symbolic_verdict_maps_and_round_trips() -> Result<(), String> {
    let bounded = Some(Strength::Bounded);
    let table = [
        (
            "success",
            Outcome::Success,
            bounded,
            Sufficiency::Sufficient,
        ),
        (
            "bounded-safe",
            Outcome::Success,
            bounded,
            Sufficiency::Prefix,
        ),
        (
            "executions-too-short",
            Outcome::Success,
            bounded,
            Sufficiency::NotReached,
        ),
        (
            "inductive-safety",
            Outcome::Success,
            Some(Strength::Inductive),
            Sufficiency::Sufficient,
        ),
        (
            "invariant-violation",
            Outcome::Counterexample,
            bounded,
            Sufficiency::NotEstablished,
        ),
        (
            "deadlock",
            Outcome::Counterexample,
            bounded,
            Sufficiency::NotEstablished,
        ),
        (
            "induction-inconclusive",
            Outcome::Inconclusive,
            None,
            Sufficiency::NotEstablished,
        ),
        (
            "inconclusive",
            Outcome::Inconclusive,
            None,
            Sufficiency::NotEstablished,
        ),
        (
            "unsupported",
            Outcome::Unsupported,
            None,
            Sufficiency::NotEstablished,
        ),
        ("error", Outcome::Crash, None, Sufficiency::NotEstablished),
        (
            "stopped",
            Outcome::Stopped,
            None,
            Sufficiency::NotEstablished,
        ),
    ];
    for (native, outcome, strength, sufficiency) in table {
        let claim = dive_symbolic_claim(native).ok_or(native)?;
        assert_eq!(
            claim,
            Claim {
                outcome,
                strength,
                sufficiency
            },
            "{native}"
        );
        round_trip(parts(model(), claim))?;
    }
    for unknown in ["", "verified", "bounded-inconclusive", "Success", "proved"] {
        assert_eq!(dive_symbolic_claim(unknown), None, "{unknown}");
    }
    Ok(())
}

#[test]
fn full_record_serializes_every_field() -> Result<(), String> {
    let mut record = parts(
        Subject {
            backend: Backend::Dive,
            kind: SubjectKind::Property,
            id: "Mutex".into(),
        },
        dive_symbolic_claim("invariant-violation").ok_or("unmapped")?,
    );
    record.bounds = vec![
        Bound {
            kind: BoundKind::PathLength,
            scope: "subject".into(),
            value: 15,
            source: BoundSource::Argument,
        },
        Bound {
            kind: BoundKind::InductionDepth,
            scope: "subject".into(),
            value: 2,
            source: BoundSource::Argument,
        },
    ];
    record.assumptions = vec![Assumption {
        kind: AssumptionKind::FiniteLayout,
        name: "TypeOK".into(),
    }];
    record.checks = BTreeMap::from([("invariant-violation".to_owned(), 1)]);
    record.witness = Some(Witness {
        format: "dive-trace".into(),
        validated_by: Some("dive validate-trace".into()),
    });
    record.provenance = Provenance::ReceiptShared;
    record.cost = Cost {
        wall_ms: Some(354),
        peak_memory_bytes: Some(114_294_784),
    };
    let text = round_trip(record)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    assert_eq!(
        value,
        serde_json::json!({
            "subject": {"backend": "dive", "kind": "property", "id": "Mutex"},
            "outcome": "counterexample",
            "tier": "bounded",
            "strength": "bounded",
            "bounds": [
                {"kind": "path_length", "scope": "subject", "value": 15, "source": "argument"},
                {"kind": "induction_depth", "scope": "subject", "value": 2, "source": "argument"}
            ],
            "sufficiency": "not_established",
            "assumptions": [{"kind": "finite_layout", "name": "TypeOK"}],
            "checks": {"invariant-violation": 1},
            "witness": {"format": "dive-trace", "validated_by": "dive validate-trace"},
            "tools": [{"name": "tool", "version": "1", "sha256": null}],
            "integrity": "supervised",
            "provenance": "receipt_shared",
            "cost": {"wall_ms": 354, "peak_memory_bytes": 114_294_784}
        })
    );
    Ok(())
}

#[test]
fn no_claim_outcomes_have_no_tier_and_are_never_replayed() -> Result<(), String> {
    for outcome in NO_CLAIM {
        assert_eq!(outcome.tier(), None);
        assert!(!outcome.completed());
        let fresh = parts(harness(), Claim::untiered(outcome));
        let text = round_trip(fresh.clone())?;
        assert!(text.contains(r#""tier":null"#), "{text}");
        for provenance in [Provenance::ReceiptLocal, Provenance::ReceiptShared] {
            let mut replayed = fresh.clone();
            replayed.provenance = provenance;
            assert!(Evidence::new(replayed).is_err(), "{outcome:?}");
        }
        let mut promoted = fresh;
        promoted.tier = Some(Tier::Bounded);
        promoted.strength = Some(Strength::Bounded);
        assert!(Evidence::new(promoted).is_err(), "{outcome:?}");
    }
    Ok(())
}

#[test]
fn nothing_upgrades_across_tiers() -> Result<(), String> {
    let good = parts(harness(), kani_claim(KaniOutcome::BoundedSuccess));
    let text = round_trip(good.clone())?;
    for stronger in [
        "smt_verified",
        "smt-verified",
        "certificate_checked",
        "kernel_checked",
        "verified",
    ] {
        let forged = text.replace(r#""tier":"bounded""#, &format!(r#""tier":"{stronger}""#));
        assert_ne!(forged, text);
        assert!(
            serde_json::from_str::<Evidence>(&forged).is_err(),
            "{stronger}"
        );
    }
    assert_eq!(EvidenceTier::from(Tier::Bounded), EvidenceTier::Bounded);

    // Inductive strength exists only on a covering success, still bounded.
    let inductive = parts(
        model(),
        dive_symbolic_claim("inductive-safety").ok_or("unmapped")?,
    );
    assert_eq!(Evidence::new(inductive.clone())?.tier, Some(Tier::Bounded));
    let mut on_refutation = inductive.clone();
    on_refutation.outcome = Outcome::Counterexample;
    assert!(Evidence::new(on_refutation).is_err());
    let mut on_prefix = inductive;
    on_prefix.sufficiency = Sufficiency::Prefix;
    assert!(Evidence::new(on_prefix).is_err());

    let insufficient = parts(harness(), kani_claim(KaniOutcome::BoundInsufficient));
    let mut tiered = insufficient.clone();
    tiered.tier = Some(Tier::Bounded);
    tiered.strength = Some(Strength::Bounded);
    assert!(Evidence::new(tiered).is_err());
    let mut covered = insufficient;
    covered.sufficiency = Sufficiency::Sufficient;
    assert!(Evidence::new(covered).is_err());

    let mut untiered_success = good.clone();
    untiered_success.tier = None;
    assert!(Evidence::new(untiered_success).is_err());
    let mut unobserved = good.clone();
    unobserved.integrity = Integrity::NotEstablished;
    assert!(Evidence::new(unobserved).is_err());
    let mut anonymous = good.clone();
    anonymous.tools.clear();
    assert!(Evidence::new(anonymous).is_err());
    let mut unsupervised_replay = good.clone();
    unsupervised_replay.integrity = Integrity::Reconciled;
    unsupervised_replay.provenance = Provenance::ReceiptLocal;
    assert!(Evidence::new(unsupervised_replay).is_err());
    let mut witnessed_success = good.clone();
    witnessed_success.witness = Some(Witness {
        format: "itf".into(),
        validated_by: None,
    });
    assert!(Evidence::new(witnessed_success).is_err());
    let mut unnamed = good;
    unnamed.subject.id.clear();
    assert!(Evidence::new(unnamed).is_err());
    Ok(())
}

#[test]
fn unknown_fields_and_values_do_not_decode() -> Result<(), String> {
    let text = round_trip(parts(harness(), kani_claim(KaniOutcome::Counterexample)))?;
    let extra = text.replacen('{', r#"{"verdict":"verified","#, 1);
    assert!(serde_json::from_str::<Evidence>(&extra).is_err());
    let outcome = text.replace(r#""outcome":"counterexample""#, r#""outcome":"verified""#);
    assert_ne!(outcome, text);
    assert!(serde_json::from_str::<Evidence>(&outcome).is_err());
    Ok(())
}
