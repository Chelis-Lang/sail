#!/usr/bin/env bash
# Real MCP client smoke test: registers `sail mcp` with the pinned Claude Code
# CLI (ci/Dockerfile) exactly as docs/mcp.md says, then has Claude Code list
# its tools. Claude Code opens with `server/discover` on revision 2026-07-28
# and rejects a `tools/list` result it cannot validate, so this fails if Sail's
# list results stop satisfying it. No credentials or model calls are needed.
#
#   ci/test-claude-code.sh SAIL
set -euo pipefail
sail="${1:?usage: ci/test-claude-code.sh SAIL}"
command -v claude >/dev/null || { echo "claude-code: claude is not on PATH" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/home" "$work/project"
# A fresh, credential-free Claude Code configuration.
export HOME="$work/home" CLAUDE_CONFIG_DIR="$work/claude"
export DISABLE_AUTOUPDATER=1 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1

# Record the traffic, so the check does not rely on the listing's wording alone.
cat >"$work/sail-mcp" <<SH
#!/bin/sh
tee -a '$work/in.jsonl' | '$sail' mcp | tee -a '$work/out.jsonl'
SH
chmod 755 "$work/sail-mcp"

cd "$work/project"
claude mcp add --transport stdio sail -- "$work/sail-mcp" >/dev/null
claude mcp list >"$work/list.txt" 2>&1 || true
cat "$work/list.txt"
if ! grep -q '^sail: .*Connected' "$work/list.txt" || grep -qi 'failed' "$work/list.txt"; then
  echo "claude-code: Claude Code $(claude --version) could not list sail's tools" >&2
  exit 1
fi
python3 - "$work/in.jsonl" "$work/out.jsonl" <<'PY'
import json, sys
requests = [json.loads(line) for line in open(sys.argv[1]) if line.strip()]
replies = {json.dumps(m.get("id")): m for m in map(json.loads, open(sys.argv[2])) if m}
lists = [r for r in requests if r.get("method") == "tools/list"]
assert lists, "Claude Code never asked for tools/list"
tools = replies[json.dumps(lists[-1]["id"])]["result"]["tools"]
assert tools, "tools/list returned no tools"
version = lists[-1]["params"]["_meta"].get("io.modelcontextprotocol/protocolVersion", "legacy")
print(f"claude-code: listed {len(tools)} sail tools on protocol {version}")
PY
