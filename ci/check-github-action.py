#!/usr/bin/env python3
"""Checks the GitHub Action's hosted runs (.github/workflows/ci.yml, `action-*`).

  cold      exit 0; the fixture's functions verified in this run
  restored  exit 0 from the exact cache key, with every requested target
            reused from proof receipts and no verifier run
  failing   the action step failed with `sail verify`'s nonzero exit code,
            the broken lemma failed, and SARIF locates its error
"""
import json
import os
from pathlib import Path
import sys

LEMMA = "sail_action_fixture::action_fixture_lemma"


def functions(document):
    return {function["function"]: function["verdict"]
            for target in document["targets"] if target["requested"]
            for crate in target["report"]["crates"] for function in crate["functions"]}


def main():
    phase = sys.argv[1]
    code = os.environ.get("EXIT_CODE", "")
    document = json.loads(Path(os.environ["RESULT"]).read_bytes())
    verdicts = functions(document)
    execution = document["execution"]
    print(json.dumps({"phase": phase, "exit_code": code, "verdicts": verdicts,
                      "currentness": document["currentness"], "execution": execution}, indent=2))
    assert code == str(document["exit_code"]), (code, document["exit_code"])
    if phase in ("cold", "restored"):
        assert code == "0", code
        assert verdicts.get(LEMMA) == "verified", verdicts
        assert document["currentness"] == "current", document["currentness"]
    if phase == "cold":
        assert execution["verifier_invocations"] > 0, execution
        sarif = json.loads(Path(os.environ["SARIF"]).read_bytes())
        assert sarif["version"] == "2.1.0" and sarif["runs"][0]["results"] == [], sarif
    elif phase == "restored":
        assert os.environ.get("CACHE_HIT") == "true", os.environ.get("CACHE_HIT")
        assert execution["verifier_invocations"] == 0, execution
    elif phase == "failing":
        assert os.environ.get("OUTCOME") == "failure", os.environ.get("OUTCOME")
        assert code not in ("", "0"), code
        assert verdicts.get(LEMMA) == "failed", verdicts
        results = json.loads(Path(os.environ["SARIF"]).read_bytes())["runs"][0]["results"]
        located = [result["locations"][0]["physicalLocation"] for result in results]
        assert any(location["artifactLocation"]["uri"] == "ci/github-action-fixture/src/lib.rs"
                   for location in located), located
    else:
        sys.exit(f"unknown phase {phase}")


if __name__ == "__main__":
    main()
