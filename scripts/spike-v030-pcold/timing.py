#!/usr/bin/env python3
"""Print the units of a cargo --timings report that matter for the root's start."""
import json
import re
import sys

for path in sys.argv[1:]:
    text = open(path).read()
    match = re.search(r"const UNIT_DATA = (\[.*?\]);\n", text, re.S)
    units = json.loads(match.group(1))
    print("==", path, len(units), "units")
    for u in units:
        u["end"] = u["start"] + u["duration"]
    names = ("vstd", "verus_", "verifiable", "verus-")
    for u in sorted(units, key=lambda u: u["start"]):
        if u["name"].startswith(names) or u["name"] in ("syn", "proc-macro2", "quote", "unicode-ident"):
            print("  %-28s %-28s start %7.2f dur %7.2f end %7.2f rmeta %s" % (
                u["name"], u.get("target", "") + " " + u.get("mode", ""), u["start"], u["duration"], u["end"],
                u.get("rmeta_time")))
    late = sorted(units, key=lambda u: -u["end"])[:8]
    print("  latest-finishing units:")
    for u in late:
        print("    %-28s %-24s start %7.2f dur %7.2f end %7.2f rmeta %s" % (
            u["name"], u.get("target", "") + " " + u.get("mode", ""), u["start"], u["duration"], u["end"], u.get("rmeta_time")))
    non = [u for u in units if not u["name"].startswith(("vstd", "verus_temporal", "verifiable"))]
    print("  last non-vstd-chain unit end: %.2f" % max(u["end"] for u in non))
