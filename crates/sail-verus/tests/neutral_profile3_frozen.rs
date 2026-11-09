//! Profile three is frozen (ADR 0107): retained version-four manifests, which
//! bind the exact shadow bytes through `lowered_sha256`, replay byte for byte.
use sail_verus::contracts::{
    LoweringManifest, ModuleSource, PROFILE_THREE_LOWERING_VERSION, lower_closure,
    validate_closure_manifest,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn module(name: &str, source: &str) -> ModuleSource {
    use sha2::Digest as _;
    ModuleSource {
        module: name.into(),
        source: source.into(),
        source_sha256: format!("{:x}", sha2::Sha256::digest(source.as_bytes())),
    }
}

/// Each retained case: label, root source, module files, retained manifest.
fn cases() -> Vec<(&'static str, &'static str, Vec<ModuleSource>, &'static str)> {
    vec![
        (
            "truncating division",
            include_str!("fixtures/neutral-contracts/truncating_division_profile3.rs"),
            Vec::new(),
            include_str!(
                "fixtures/neutral-contracts/truncating_division_profile3.v4-manifest.json"
            ),
        ),
        (
            "symbolic remainder",
            include_str!("fixtures/neutral-contracts/signed_remainder_symbolic_profile3.rs"),
            Vec::new(),
            include_str!(
                "fixtures/neutral-contracts/signed_remainder_symbolic_profile3.v4-manifest.json"
            ),
        ),
        (
            "body calls",
            include_str!("fixtures/neutral-contracts/body_calls_profile3.rs"),
            Vec::new(),
            include_str!("fixtures/neutral-contracts/body_calls_profile3.v4-manifest.json"),
        ),
        (
            "loops",
            include_str!("fixtures/neutral-contracts/loops_profile3.rs"),
            Vec::new(),
            include_str!("fixtures/neutral-contracts/loops_profile3.v4-manifest.json"),
        ),
        (
            "closure",
            include_str!("fixtures/neutral-contracts/closure/mod.rs"),
            vec![
                module(
                    "rules",
                    include_str!("fixtures/neutral-contracts/closure/rules.rs"),
                ),
                module(
                    "shapes",
                    include_str!("fixtures/neutral-contracts/closure/shapes.rs"),
                ),
                module(
                    "shapes::measure",
                    include_str!("fixtures/neutral-contracts/closure/shapes/measure.rs"),
                ),
            ],
            include_str!("fixtures/neutral-contracts/closure.v4-manifest.json"),
        ),
    ]
}

#[test]
fn retained_version_four_manifests_replay_byte_for_byte() -> TestResult {
    for (label, source, modules, retained) in cases() {
        let manifest: LoweringManifest = serde_json::from_str(retained)?;
        assert_eq!(manifest.version, PROFILE_THREE_LOWERING_VERSION, "{label}");
        assert_eq!(manifest.contract_version, 3, "{label}");
        validate_closure_manifest(source, &modules, &manifest)?;
        let lowered = lower_closure(source, &modules, "neutral", 3)?;
        assert_eq!(
            format!("{}\n", serde_json::to_string_pretty(&lowered.manifest)?),
            retained,
            "{label}: profile three is frozen (ADR 0107); a change needs a new lowering version"
        );
    }
    Ok(())
}
