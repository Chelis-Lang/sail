"""Isolated manifest for the embedded helper's trusted OS bootstrap closure."""
import fcntl
import json
import os
import signal
import resource
import stat
import sys

def support_paths():
    paths = {os.path.abspath(sys.executable)}
    for module in tuple(sys.modules.values()):
        for name in ("__file__", "__cached__"):
            path = getattr(module, name, None)
            if isinstance(path, str):
                paths.add(os.path.abspath(path))
    with open("/proc/self/maps", encoding="utf-8") as mappings:
        for line in mappings:
            fields = line.rstrip("\n").split(None, 5)
            if len(fields) == 6 and fields[5].startswith("/"):
                if fields[5].endswith(" (deleted)") or "\\" in fields[5]:
                    raise ValueError("unsupported loaded-library binding")
                paths.add(fields[5])
    return sorted(paths)
