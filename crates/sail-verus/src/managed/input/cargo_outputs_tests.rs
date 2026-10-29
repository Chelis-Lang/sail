use super::*;

struct Fixture {
    root: PathBuf,
    data: Value,
}

impl Fixture {
    fn new(label: &str) -> std::io::Result<Self> {
        let root =
            std::env::temp_dir().join(format!("sail-cargo-output-{label}-{}", std::process::id()));
        std::fs::create_dir_all(root.join("src"))?;
        std::fs::create_dir_all(root.join("target"))?;
        let root = root.canonicalize()?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='demo'\nversion='0.1.0'\n",
        )?;
        std::fs::write(root.join("src/lib.rs"), "proof before")?;
        std::fs::write(root.join("target/.rustc_info.json"), "output before")?;
        std::fs::write(
            root.join("target/CACHEDIR.TAG"),
            [CARGO_TAG, b"# fixture\n"].concat(),
        )?;
        let data = serde_json::json!({
            "workspace_root":root, "sail_invocation_directory":root,
            "target_directory":root.join("managed-output"),
            "sail_source_inputs":[root.join("src/lib.rs")],
            "packages":[{"manifest_path":root.join("Cargo.toml"),
                "targets":[{"src_path":root.join("src/lib.rs"), "kind":["lib"], "name":"demo"}]}]
        });
        Ok(Self { root, data })
    }

    fn snapshot(&self, extras: &[PathBuf]) -> InputSnapshot {
        snapshot_metadata_home(&self.data, extras, None)
    }

    fn excludes_target(&self, snapshot: &InputSnapshot) -> bool {
        snapshot.excluded.contains(&self.root.join("target"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn cargo_output_churn_preserves_the_exact_revision_and_source_vector() -> std::io::Result<()> {
    let fixture = Fixture::new("churn")?;
    let before = fixture.snapshot(&[]);
    assert!(before.complete && fixture.excludes_target(&before));
    assert!(
        before
            .digests
            .contains_key(&fixture.root.join("target/CACHEDIR.TAG"))
    );
    assert!(
        !before
            .observation_paths
            .contains(&fixture.root.join("target"))
    );
    let stamp =
        mutation_stamp(&before).ok_or_else(|| std::io::Error::other("source stamp missing"))?;
    let vector = mutation_vector(&before, &stamp)
        .ok_or_else(|| std::io::Error::other("source vector missing"))?;
    std::fs::write(
        fixture.root.join("target/.rustc_info.json"),
        "different compiler output",
    )?;
    std::fs::create_dir_all(fixture.root.join("target/debug/deps"))?;
    std::fs::write(
        fixture.root.join("target/debug/deps/new.rlib"),
        "new artifact",
    )?;
    let after = fixture.snapshot(&[]);
    assert!(after.complete);
    assert_eq!(before.revision, after.revision);
    assert_eq!(before.digests, after.digests);
    assert_eq!(Some(stamp.clone()), mutation_stamp(&after));
    assert_eq!(Some(vector), mutation_vector(&after, &stamp));
    Ok(())
}

#[test]
fn declared_sources_directories_and_target_roots_keep_their_complete_inputs() -> std::io::Result<()>
{
    for kind in ["source", "extra", "directory", "ancestor", "crate"] {
        let mut fixture = Fixture::new(kind)?;
        let input = fixture.root.join("target/source.rs");
        std::fs::write(&input, "one")?;
        let mut extras = Vec::new();
        match kind {
            "source" => fixture.data["sail_source_inputs"] = serde_json::json!([input]),
            "extra" => extras.push(input.clone()),
            "directory" => extras.push(fixture.root.join("target")),
            "ancestor" => extras.push(fixture.root.clone()),
            "crate" => {
                fixture.data["packages"][0]["targets"][0]["src_path"] = serde_json::json!(input)
            }
            _ => return Err(std::io::Error::other("unknown fixture")),
        }
        let before = fixture.snapshot(&extras);
        assert!(
            before.complete && !fixture.excludes_target(&before),
            "{kind}"
        );
        assert!(before.digests.contains_key(&input), "{kind}");
        std::fs::write(&input, "two")?;
        assert_ne!(
            before.revision,
            fixture.snapshot(&extras).revision,
            "{kind}"
        );
    }
    Ok(())
}

#[test]
fn a_package_inside_target_prevents_output_exclusion() -> std::io::Result<()> {
    let mut fixture = Fixture::new("member")?;
    let member = fixture.root.join("target/member");
    std::fs::create_dir_all(member.join("src"))?;
    std::fs::write(member.join("Cargo.toml"), "[package]\n")?;
    std::fs::write(member.join("src/lib.rs"), "member source")?;
    fixture.data["packages"]
        .as_array_mut()
        .ok_or_else(|| std::io::Error::other("fixture packages"))?
        .push(
            serde_json::json!({"manifest_path":member.join("Cargo.toml"),
            "targets":[{"src_path":member.join("src/lib.rs") }]}),
        );
    let snapshot = fixture.snapshot(&[]);
    assert!(snapshot.complete && !fixture.excludes_target(&snapshot));
    assert!(snapshot.digests.contains_key(&member.join("src/lib.rs")));
    Ok(())
}

#[test]
fn names_alone_unknown_metadata_and_aliases_do_not_select_output_exclusion() -> std::io::Result<()>
{
    for kind in [
        "missing",
        "invalid",
        "oversized",
        "tag-alias",
        "target-alias",
        "src-tag",
        "unknown",
    ] {
        let mut fixture = Fixture::new(kind)?;
        let tag = fixture.root.join("target/CACHEDIR.TAG");
        match kind {
            "missing" => std::fs::remove_file(&tag)?,
            "invalid" => std::fs::write(&tag, "not Cargo's marker")?,
            "oversized" => std::fs::write(&tag, [CARGO_TAG, &vec![b'x'; 4097]].concat())?,
            "tag-alias" => {
                std::fs::rename(&tag, fixture.root.join("actual-tag"))?;
                std::os::unix::fs::symlink(fixture.root.join("actual-tag"), &tag)?;
            }
            "target-alias" => {
                std::fs::rename(
                    fixture.root.join("target"),
                    fixture.root.join("actual-target"),
                )?;
                std::os::unix::fs::symlink(
                    fixture.root.join("actual-target"),
                    fixture.root.join("target"),
                )?;
            }
            "src-tag" => {
                std::fs::rename(&tag, fixture.root.join("src/CACHEDIR.TAG"))?;
            }
            "unknown" => {
                fixture
                    .data
                    .as_object_mut()
                    .ok_or_else(|| std::io::Error::other("fixture data"))?
                    .remove("sail_source_inputs");
            }
            _ => return Err(std::io::Error::other("unknown fixture")),
        }
        let snapshot = fixture.snapshot(&[]);
        let output = fixture
            .root
            .join("target/.rustc_info.json")
            .canonicalize()?;
        assert!(
            snapshot.complete && !fixture.excludes_target(&snapshot),
            "{kind}"
        );
        assert!(snapshot.digests.contains_key(&output), "{kind}");
    }
    Ok(())
}

#[test]
fn output_namespace_and_marker_changes_refuse_old_projections() -> std::io::Result<()> {
    let mut fixture = Fixture::new("projection")?;
    let before = fixture.snapshot(&[]);
    let projected = projected_snapshot(&fixture.data, &[], None, &before)
        .ok_or_else(|| std::io::Error::other("projection refused"))?;
    assert_eq!(before.revision, projected.revision);
    assert_eq!(before.digests, projected.digests);
    assert_eq!(before.observation_paths, projected.observation_paths);
    assert_eq!(before.excluded, projected.excluded);
    let mut minimal = fixture.data.clone();
    minimal["packages"] = serde_json::json!([package_projection(&fixture.data["packages"][0])]);
    assert_eq!(
        before.revision,
        snapshot_metadata_home(&minimal, &[], None).revision
    );
    // A newly discovered compiler include must widen the walk even when the
    // old complete source vector itself still matches.
    let include = fixture.root.join("target/.rustc_info.json");
    fixture.data["sail_source_inputs"] = serde_json::json!([include]);
    assert!(projected_snapshot(&fixture.data, &[], None, &before).is_none());
    assert!(fixture.snapshot(&[]).digests.contains_key(&include));
    // Turning an already captured ordinary tree into output must also miss;
    // old bytes cannot be projected under a different namespace decision.
    let full = fixture.snapshot(&[]);
    fixture.data["sail_source_inputs"] = serde_json::json!([]);
    assert!(projected_snapshot(&fixture.data, &[], None, &full).is_none());
    std::fs::remove_file(fixture.root.join("target/CACHEDIR.TAG"))?;
    assert!(projected_snapshot(&fixture.data, &[], None, &before).is_none());
    Ok(())
}
