import hashlib,json
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def pinned_libraries(deps):
    fingerprints = deps.parent / ".fingerprint"
    # Exact release profile was retained by the existing actual metadata controls;
    # Clippy's check profile has its own graph and cannot supply linker rlibs.
    roots = [p for p in fingerprints.glob("rust_verify-*/bin-rust_verify.json")
             if json.loads(p.read_text())["profile"] == 1241562680900052086]
    if len(roots) != 1:
        raise ValueError("require one actual pinned rust_verify binary fingerprint")
    root = json.loads(roots[0].read_text())
    fingerprints_used = {str(roots[0].relative_to(fingerprints)): sha(roots[0])}
    libraries = {}
    documents = {}
    for name in ("serde", "bincode_next"):
        requested = [item[3] for item in root["deps"] if item[1] == name]
        if len(requested) != 1:
            raise ValueError(f"missing or ambiguous pinned {name} dependency")
        package = name.replace("_", "-")
        matches = []
        for path in fingerprints.glob(f"{package}-*/lib-{name}"):
            raw = bytes.fromhex(path.read_text().strip())
            if len(raw) == 8 and int.from_bytes(raw, "little") == requested[0]:
                matches.append(path)
        if len(matches) != 1:
            raise ValueError(f"require unique actual fingerprint for {name}")
        identity = matches[0]
        document_path = identity.with_name(identity.name + ".json")
        document = json.loads(document_path.read_text())
        if document["rustc"] != root["rustc"] or document["rustflags"] != root["rustflags"]:
            raise ValueError("pinned compiler/flags mismatch")
        suffix = identity.parent.name.removeprefix(package + "-")
        library = deps / f"lib{name}-{suffix}.rlib"
        if not library.is_file():
            raise ValueError("actual pinned rlib unavailable")
        libraries[name] = library
        documents[name] = document
        fingerprints_used[str(identity.relative_to(fingerprints))] = sha(identity)
        fingerprints_used[str(document_path.relative_to(fingerprints))] = sha(document_path)
    serde_from_bincode = [item[3] for item in documents["bincode_next"]["deps"] if item[1] == "serde"]
    serde_from_driver = [item[3] for item in root["deps"] if item[1] == "serde"]
    if serde_from_bincode != serde_from_driver:
        raise ValueError("bincode serde crate identity differs from actual driver")
    return libraries, {"fingerprint_sha256": fingerprints_used,
                       "root_features": root["features"], "rustflags": root["rustflags"],
                       "root_profile": root["profile"], "dependency_profiles": {name: document["profile"] for name, document in documents.items()},
                       "dependency_features": {name: document["features"] for name, document in documents.items()}, "compiler_identity": root["rustc"],
                       "library_sha256": {p.name: sha(p) for p in libraries.values()}}

