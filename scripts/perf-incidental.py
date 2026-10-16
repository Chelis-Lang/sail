#!/usr/bin/env python3
"""Frozen incidental edits for perf-real.sh; never rewrites fetched sources.

`check CORPUS` validates all source pins and edit round trips without Verus.
`list PROJECT` emits perf-real edit records. `apply PROJECT ID PRISTINE COPY`
edits only COPY after checking the exact original bytes. `restore PROJECT COPY`
checks that every catalogue file was restored byte-for-byte.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
CATALOGUE = ROOT / "corpus/incidental-edits.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def catalogue():
    data = json.loads(CATALOGUE.read_bytes())
    if data["schema_version"] != 1:
        raise ValueError("unsupported incidental catalogue schema")
    pins = (ROOT / "corpus/corpus.toml").read_text()
    for project in data["projects"].values():
        blocks = [block for block in pins.split("[[project]]")
                  if re.search(r'^name = "' + re.escape(project["corpus_entry"]) + r'"$', block, re.M)]
        if len(blocks) != 1 or not re.search(
                r'^commit = "' + re.escape(project["commit"]) + r'"$', blocks[0], re.M):
            raise ValueError("incidental catalogue no longer matches the corpus pin")
    return data["projects"]


def original(project, root):
    files = {}
    for name, expected in project["files"].items():
        data = (root / name).read_bytes()
        if sha(data) != expected:
            raise ValueError(f"incidental source does not match pin: {name}")
        files[name] = data
    return files


def unique_replace(source, before, after):
    before, after = before.encode(), after.encode()
    if not before or source.count(before) != 1:
        raise ValueError("incidental anchor is absent or ambiguous")
    return source.replace(before, after, 1)


def transform(source, edit):
    if edit["kind"] == "helper-move":
        moved = edit["remove"]
        result = unique_replace(source, moved, "")
        result = unique_replace(result, edit["insert_before"], moved + edit["insert_before"])
        # Moving a helper must preserve its bytes and exactly one occurrence.
        if result.count(moved.encode()) != 1:
            raise ValueError("helper move changed or duplicated the helper")
    else:
        result = unique_replace(source, edit["before"], edit["replacement"])
    if result == source:
        raise ValueError("incidental edit made no change")
    return result


def check(corpus):
    records = []
    for name, project in catalogue().items():
        root = corpus / project["source_dir"]
        files = original(project, root)
        for edit in project["edits"]:
            source = files[edit["file"]]
            changed = transform(source, edit)
            if edit["kind"] == "helper-move":
                moved = edit["remove"].encode()
                # Both source and edited file contain exactly the same helper;
                # removing it from each must leave identical surrounding code.
                if source.replace(moved, b"", 1) != changed.replace(moved, b"", 1):
                    raise ValueError("helper move changed surrounding source")
            records.append({"project": name, "commit": project["commit"],
                            "edit": edit["id"], "file": edit["file"],
                            "original_sha256": sha(source), "edited_sha256": sha(changed)})
    print(json.dumps({"catalogue_sha256": sha(CATALOGUE.read_bytes()), "edits": records}, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "list", "apply", "restore"))
    parser.add_argument("args", nargs="+")
    args = parser.parse_args()
    if args.command == "check":
        [corpus] = args.args
        check(Path(corpus))
        return
    project = catalogue()[args.args[0]]
    if args.command == "list":
        for edit in project["edits"]:
            print(edit["id"], "incidental", edit["file"], edit["id"], 0)
    elif args.command == "restore":
        [_, root] = args.args
        original(project, Path(root))
    else:
        [_, identity, pristine, destination] = args.args
        edit = next(e for e in project["edits"] if e["id"] == identity)
        source = original(project, Path(pristine))[edit["file"]]
        changed = transform(source, edit)
        target = Path(destination) / edit["file"]
        temporary = target.with_name(target.name + ".sail-incidental-tmp")
        temporary.write_bytes(changed)
        temporary.replace(target)
        print(json.dumps({"edit": identity, "file": edit["file"],
                          "original_sha256": sha(source), "edited_sha256": sha(changed)}))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, StopIteration) as error:
        print(f"incidental catalogue: {error}", file=sys.stderr)
        sys.exit(1)
