//! The D.I.V.E. revision Sail reports is the one this workspace links.

#[test]
fn reported_revision_is_the_linked_revision() {
    let manifest = include_str!("../../../Cargo.toml");
    let lock = include_str!("../../../Cargo.lock");
    let revision = sail_dive::DIVE_REVISION;
    for name in ["dive-api", "dive-ir"] {
        let pin = format!(
            "{name} = {{ git = \"https://github.com/Chelis-Lang/dive\", rev = \"{revision}\" }}"
        );
        assert!(
            manifest.contains(&pin),
            "{name} is not pinned at {revision}"
        );
        let locked = format!(
            "name = \"{name}\"\nversion = \"0.0.0\"\nsource = \"git+https://github.com/Chelis-Lang/dive?rev={revision}#{revision}\""
        );
        assert!(lock.contains(&locked), "{name} is not locked at {revision}");
    }
}
