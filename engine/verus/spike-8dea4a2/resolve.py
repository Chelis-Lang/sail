#!/usr/bin/env python3
"""Resolve the diff3 conflicts of the squashed sail series on upstream 8dea4a2."""
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
PAT = re.compile(
    r"<<<<<<< [^\n]*\n(.*?)\|\|\|\|\|\|\| [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", re.S
)


def additive(ours, base, theirs):
    """theirs is base plus a pure prefix or suffix: keep ours, add that part."""
    if theirs.startswith(base):
        return ours + theirs[len(base):]
    if theirs.endswith(base):
        return theirs[: len(theirs) - len(base)] + ours
    raise SystemExit("not additive")


def resolve(rel, rules, post=None):
    path = root / rel
    text = path.read_text()
    blocks = PAT.findall(text)
    assert len(blocks) == len(rules), (rel, len(blocks), len(rules))
    it = iter(rules)

    def sub(m):
        rule = next(it)
        return rule(m.group(1), m.group(2), m.group(3))

    text = PAT.sub(sub, text)
    if post:
        text = post(text)
    assert "<<<<<<<" not in text and ">>>>>>>" not in text
    path.write_text(text)


both = lambda o, b, t: o + t
resolve("source/Cargo.lock", [additive])
resolve("source/rust_verify_test/tests/common/mod.rs", [additive])
resolve("source/vir/src/ast_util.rs", [additive])
resolve("source/vir/src/ast_visitor.rs", [additive])
resolve(
    "source/vir/src/def.rs",
    [
        lambda o, b, t: "use crate::ast::{\n    ClosureId, ClosureKind, CrateId, Dt, Fun, FunX, InvAtomicity, Path, PathX, VarIdent,\n};\n",
        additive,
    ],
)
resolve(
    "source/vir/src/prune.rs",
    [lambda o, b, t: o.replace("HashSet::new()", "HashSet::default()")],
)


def erase_post(text):
    dup = "use vir::ast::{Datatype, Dt, Fun, Function, Krate, Mode, Path};\nuse vir::modes::ErasureModes;\nuse vir::sail_hash"
    assert dup in text
    return text.replace(dup, "use vir::sail_hash")


resolve(
    "source/rust_verify/src/erase.rs",
    [
        lambda o, b, t: o,
        lambda o, b, t: "    let shadow_check_ids: FxHashSet<_> = erasure_hints.shadow_check.iter().copied().collect();\n"
        + o.lstrip("\n"),
    ],
    erase_post,
)
resolve(
    "source/rust_verify/src/verifier.rs",
    [
        both,
        both,
        both,
        lambda o, b, t: t.replace("&mut spinoff_context\n", "&mut spinoff_z3_context\n", 1),
        lambda o, b, t: o.replace(
            "self.verify_crate_inner(&compiler, spans)",
            "self.verify_crate_inner(&compiler, spans, &Reporter::new(spans, compiler))",
        ),
    ],
)
print("resolved")
