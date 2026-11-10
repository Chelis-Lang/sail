use super::*;

#[test]
fn windows_are_unique() {
    let src = "a\nb\nx\nb\nc\n";
    let s = src.find('x').unwrap_or(0);
    // "x\n" alone is unique.
    assert_eq!(unique_window(src, s, s + 1), Some((4, 6)));
    // The second "b\n" needs context.
    let second = src.rfind('b').unwrap_or(0);
    let w = unique_window(src, second, second + 1).unwrap_or((0, 0));
    let text = src.get(w.0..w.1).unwrap_or_default();
    assert_eq!(src.matches(text).count(), 1);
    assert!(text.contains("b\n"));
}

/// A two-file crate in a fresh temporary directory.
fn fixture(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sail-mutate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("src"));
    let lib = "verus! {\nmod m;\nfn f(a: u64) -> (r: u64)\n    requires\n        a < 10,\n    ensures\n        r == a + 1,\n{\n    let x = a + 1;\n    assert(x > a);\n    x\n}\n}\n";
    let _ = std::fs::write(dir.join("src/lib.rs"), lib);
    let _ = std::fs::write(
        dir.join("src/m.rs"),
        "verus! {\nproof fn p(x: int) ensures x + 0 == x {}\n}\n",
    );
    dir
}

#[test]
fn generation_is_deterministic() {
    let dir = fixture("det");
    let files = crate_files(&dir, &dir.join("src/lib.rs"));
    assert_eq!(files, ["src/lib.rs", "src/m.rs"]);
    let run = |seed| {
        generate(&dir, &files, "p", "p", seed, 3)
            .map(|(m, _)| catalogue(&m, "p/u", ""))
            .unwrap_or_default()
    };
    assert_eq!(run(7), run(7));
    assert!(run(7).contains("[[edit]]"));
    let parsed: std::result::Result<toml::Table, _> = run(7).parse();
    assert!(parsed.is_ok(), "{parsed:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn request(dir: &Path) -> Request {
    Request {
        source: dir.to_path_buf(),
        crate_root: dir.join("src/lib.rs"),
        project: "fixture".to_owned(),
        unit: "fixture/src/lib.rs".to_owned(),
        commit: "0000000000000000000000000000000000000000".to_owned(),
        invocation: "--project fixture".to_owned(),
        name: "fixture".to_owned(),
        seed: 20_261_020,
        per_class: 2,
        local: false,
    }
}

#[test]
fn settling_replays_the_oracle_rewrite() {
    let dir = fixture("settle");
    let g = render(&request(&dir));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(g.is_ok(), "{g:?}");
    let Ok(g) = g else {
        return;
    };
    assert!(g.mutants.len() >= 2, "{:?}", g.mutants);
    let first = g.mutants.first().map(|m| m.id.clone()).unwrap_or_default();
    let mut results = serde_json::Map::new();
    for m in &g.mutants {
        results.insert(m.id.clone(), serde_json::json!({"outcomes": ["errors"]}));
    }
    let mut meta = g.meta.clone();
    if let Some(o) = meta.as_object_mut() {
        o.insert(
            "oracle".to_owned(),
            serde_json::json!({
                "runs": 3,
                "platform": "x86_64-unknown-linux-gnu",
                "stock_verus": "0.0.0",
                "dropped": {first.clone(): "does not compile"},
                "results": results,
            }),
        );
    }
    let settled = settle(&g, "fixture/src/lib.rs", &meta).unwrap_or_default();
    assert!(!settled.contains(&format!("id = \"{first}\"")));
    assert!(!settled.contains("\"pending\""));
    assert!(settled.contains("expected = \"errors\""));
    assert!(settled.contains(&format!(
        "# Oracle: 3 stock runs per mutant on x86_64-unknown-linux-gnu (Verus 0.0.0); {} kept, 1 dropped\n# (reasons in the sidecar).\nschema_version = 1\n",
        g.mutants.len() - 1
    )));
    assert!(settled.contains("(gen-mutants 3)"));
    // An oracle that misses a generated mutant is refused.
    if let Some(r) = meta
        .pointer_mut("/oracle/results")
        .and_then(serde_json::Value::as_object_mut)
    {
        let last = g.mutants.last().map(|m| m.id.clone()).unwrap_or_default();
        r.remove(&last);
    }
    assert!(settle(&g, "fixture/src/lib.rs", &meta).is_err());
}

/// The checked-in fixture crate's catalogue regenerates byte for byte
/// (it is the output of `cargo xtask gen-mutants --file
/// crates/sail-verus/tests/fixtures/mutate/fixture/lib.rs --name fixture
/// --seed 20261020 --per-class 3 --out crates/sail-verus/tests/fixtures/mutate`):
/// any change to the generator's output shows here, in CI, before it
/// reaches the corpus catalogues (which `cargo xtask gen-mutants --check`
/// verifies against the fetched corpus).
#[test]
fn the_fixture_catalogue_is_unchanged() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mutate");
    let req = Request {
        // As `cargo xtask gen-mutants --file` reads it: the crate root's
        // directory is the source.
        source: root.join("fixture"),
        crate_root: root.join("fixture/lib.rs"),
        project: "fixture".to_owned(),
        unit: "fixture".to_owned(),
        commit: "local source".to_owned(),
        invocation: "--file crates/sail-verus/tests/fixtures/mutate/fixture/lib.rs".to_owned(),
        name: "fixture".to_owned(),
        seed: 20_261_020,
        per_class: 3,
        local: true,
    };
    let g = render(&req);
    assert!(g.is_ok(), "{g:?}");
    let Ok(g) = g else {
        return;
    };
    let want_toml = std::fs::read_to_string(root.join("fixture.toml")).unwrap_or_default();
    let want_meta = std::fs::read_to_string(root.join("fixture.meta.json")).unwrap_or_default();
    if std::env::var_os("SAIL_BLESS").is_some() {
        let _ = std::fs::write(root.join("fixture.toml"), &g.text);
        let _ = std::fs::write(
            root.join("fixture.meta.json"),
            meta_text(&g.meta).unwrap_or_default(),
        );
        return;
    }
    assert_eq!(g.text, want_toml);
    assert_eq!(meta_text(&g.meta).unwrap_or_default(), want_meta);
}
