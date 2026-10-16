#!/usr/bin/env python3
"""Exercise independent layer reconstruction and unsafe-input refusals."""

import importlib.util
import json
import os
from pathlib import Path
import tarfile
import tempfile

spec = importlib.util.spec_from_file_location("partition_verus", Path(__file__).with_name("partition-verus.py"))
partitioner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(partitioner)

with tempfile.TemporaryDirectory(prefix="sail-verus-layers-") as temporary:
    root = Path(temporary)
    source = root / "original"
    (source / "runtime/empty").mkdir(parents=True)
    (source / "runtime/subdir").mkdir()
    (source / "runtime/compiler").write_bytes(b"compiler fixture\x00" * 100)
    (source / "runtime/compiler").chmod(0o751)
    os.link(source / "runtime/compiler", source / "runtime/subdir/compiler-link")
    (source / "runtime/same-bytes-independent").write_bytes((source / "runtime/compiler").read_bytes())
    (source / "runtime/same-bytes-independent").chmod(0o751)
    (source / "runtime/proofs.vir").write_bytes(b"proofs" * 20)
    (source / "runtime/proofs.vir").chmod(0o440)
    (source / "runtime/current").symlink_to("subdir")
    (source / "runtime/dangling").symlink_to("does-not-exist")
    (root / "external").write_bytes(b"must never follow or import")
    (source / "runtime/external").symlink_to("../../external")
    (source / "runtime/empty").chmod(0o750)
    original = partitioner.inventory(source)
    plan = partitioner.partition(source, root / "parts", 4)
    replay = partitioner.partition(source, root / "parts-repeat", 4)
    assert plan == replay, "partition must not depend on inode identity or traversal order"

    # Tar archives independently model merging whole COPY layer directories.
    # The actual Docker implementation still needs native image qualification.
    restored = root / "restored"
    restored.mkdir()
    for index in range(4):
        archive = root / (str(index) + ".tar")
        with tarfile.open(archive, "w", dereference=False) as stream:
            stream.add(root / "parts" / str(index), arcname=".")
        with tarfile.open(archive) as stream:
            # All members were produced above from this private fixture,
            # including intentional symlinks whose targets are not traversed.
            stream.extractall(restored, filter="fully_trusted")
    assert partitioner.inventory(restored) == original, "layer replay changed path identity or hardlink topology"
    linked = restored / "runtime/subdir/compiler-link"
    assert linked.samefile(restored / "runtime/compiler")
    assert not linked.samefile(restored / "runtime/same-bytes-independent")
    assert not (restored / "external").exists(), "external symlink bytes were imported"
    assert partitioner.inventory(source) == original, "source installation changed"

    refused = []
    for label, original_source, destination, count in (
        ("existing-output", source, root / "parts", 4),
        ("nested-output", source, source / "parts", 4),
        ("zero-parts", source, root / "zero", 0),
        ("unbounded-parts", source, root / "many", 33),
    ):
        try:
            partitioner.partition(original_source, destination, count)
        except ValueError:
            refused.append(label)
        else:
            raise AssertionError("unsafe input accepted: " + label)
    alias = root / "source-alias"
    alias.symlink_to(source, target_is_directory=True)
    try:
        partitioner.partition(alias, root / "alias-output", 4)
    except ValueError:
        refused.append("aliased-root")
    else:
        raise AssertionError("aliased root accepted")
    fifo_source = root / "special"
    fifo_source.mkdir()
    os.mkfifo(fifo_source / "pipe")
    try:
        partitioner.partition(fifo_source, root / "special-output", 4)
    except ValueError:
        refused.append("special-file")
    else:
        raise AssertionError("special installation file accepted")
    assert not (root / "special-output").exists(), "special-file refusal came after copying"
    print(json.dumps(dict(scope="filesystem and tar replay controls; native Docker and CI timings pending",
                          reconstructed=True, hardlinks_preserved=True, independent_files_preserved=True,
                          source_unchanged=True, refusals=refused)))
