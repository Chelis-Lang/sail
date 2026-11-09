#!/usr/bin/env python3
"""Scripted LSP client for the `sail lsp` spike.

Drives a real `sail lsp` against a real Verus project the way an editor
does (initialize with a workspace folder, didOpen, edits followed by
didSave, shutdown, exit) and checks what it publishes:

  1. a failed postcondition appears on the right file, line and columns;
  2. the diagnostic is cleared when the edit is queued, and stays cleared
     after the fixed source verifies;
  3. after the job daemon is killed, the server reconnects and publishes
     the next failure instead of exiting;
  4. with `--stale mark`, the old diagnostic is demoted while the next
     run is pending and then replaced at its new line.

Usage: lsp_client.py SAIL PROJECT_DIR
  SAIL         the `sail` binary (with `sail-z3` beside it)
  PROJECT_DIR  a library created by `sail new --lib`; src/lib.rs is rewritten

The environment selects the cache, daemon socket and Verus build
(SAIL_CACHE_DIR, SAIL_DAEMON_SOCKET, SAIL_VERUS_BUILD). Nothing is timed;
the waits only bound a hung run.
"""

import json
import os
import queue
import subprocess
import sys
import threading
import urllib.parse

WAIT = 900

BODY = """use vstd::prelude::*;

verus! {

pub fn double(x: u8) -> (r: u8)
    requires x < 100,
    ensures r == 2 * x,
{
    %s
}

} // verus!
"""
FAILING = BODY % "x + x + 1"
PASSING = BODY % "x + x"


class Client:
    def __init__(self, sail, cwd, args):
        self.process = subprocess.Popen(
            [sail, "lsp", *args],
            cwd=cwd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=open(os.devnull, "wb"),
        )
        self.messages = queue.Queue()
        self.log = []
        self.next_id = 0
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        out = self.process.stdout
        while True:
            length = None
            while True:
                line = out.readline()
                if not line:
                    self.messages.put(None)
                    return
                line = line.strip()
                if not line:
                    break
                name, _, value = line.partition(b":")
                if name.lower() == b"content-length":
                    length = int(value)
            self.messages.put(json.loads(out.read(length)))

    def send(self, message):
        body = json.dumps({"jsonrpc": "2.0", **message}).encode()
        self.process.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
        self.process.stdin.flush()

    def request(self, method, params=None):
        self.next_id += 1
        self.send({"id": self.next_id, "method": method, "params": params})
        return self.until(lambda m: m.get("id") == self.next_id, f"reply to {method}")

    def notify(self, method, params=None):
        self.send({"method": method, "params": params})

    def until(self, predicate, what):
        """Returns the first message satisfying `predicate`, and every
        message read on the way (inclusive)."""
        seen = []
        while True:
            try:
                message = self.messages.get(timeout=WAIT)
            except queue.Empty:
                raise SystemExit(f"FAIL: timed out waiting for {what}")
            if message is None:
                raise SystemExit(f"FAIL: server closed stdout waiting for {what}")
            self.log.append(message)
            seen.append(message)
            show(message)
            if predicate(message):
                return message, seen


def show(message):
    method = message.get("method")
    params = message.get("params") or {}
    if method == "textDocument/publishDiagnostics":
        items = [
            (d["range"]["start"]["line"], d["range"]["start"]["character"],
             d["range"]["end"]["character"], d["severity"], d["message"])
            for d in params["diagnostics"]
        ]
        print(f"  <- publishDiagnostics {os.path.basename(params['uri'])} {items}")
    elif method == "$/sail/status":
        keep = {k: v for k, v in params.items() if k not in ("job", "summary") and v is not None}
        print(f"  <- $/sail/status {keep}")
    elif method == "window/showMessage":
        print(f"  <- showMessage {params}")
    elif method is None:
        print(f"  <- response id={message.get('id')} error={message.get('error')}")


def is_status(event):
    return lambda m: m.get("method") == "$/sail/status" and m["params"].get("event") == event


def publishes(seen, uri):
    return [
        m["params"]["diagnostics"]
        for m in seen
        if m.get("method") == "textDocument/publishDiagnostics" and m["params"]["uri"] == uri
    ]


def check(condition, what):
    print(("ok: " if condition else "FAIL: ") + what)
    if not condition:
        raise SystemExit(1)


def start(sail, project, args):
    # Deliberately not started in the project: the workspace folder must win.
    client = Client(sail, "/", args)
    uri = "file://" + urllib.parse.quote(project)
    (reply, _) = client.request(
        "initialize",
        {"processId": os.getpid(), "rootUri": uri,
         "workspaceFolders": [{"uri": uri, "name": "proj"}], "capabilities": {}},
    )
    check(reply["result"]["capabilities"]["textDocumentSync"]["save"] == {"includeText": False},
          "initialize advertises save notifications")
    client.notify("initialized", {})
    return client


def save(client, path, uri, text):
    with open(path, "w") as handle:
        handle.write(text)
    client.notify("textDocument/didSave", {"textDocument": {"uri": uri}})


def finish(client):
    (reply, _) = client.request("shutdown")
    check(reply.get("result", 0) is None and "error" not in reply, "shutdown is answered with null")
    client.notify("exit")
    code = client.process.wait(timeout=WAIT)
    check(code == 0, f"server exits 0 after shutdown and exit (got {code})")


def expect_failure(client, uri, line, what):
    (_, seen) = client.until(is_status("result"), what)
    last = publishes(seen, uri)[-1] if publishes(seen, uri) else None
    check(last is not None and len(last) == 1, f"{what}: one diagnostic on src/lib.rs")
    item = last[0]
    check(item["range"] == {"start": {"line": line, "character": 12},
                            "end": {"line": line, "character": 22}},
          f"{what}: range is line {line} (0-based), characters 12..22: {item['range']}")
    check(item["severity"] == 1 and item["source"] == "sail"
          and item["message"] == "postcondition not satisfied\nfailed this postcondition",
          f"{what}: error severity with Verus's message and label")
    return seen


def main():
    sail, project = sys.argv[1], os.path.realpath(sys.argv[2])
    path = os.path.join(project, "src", "lib.rs")
    uri = "file://" + urllib.parse.quote(path)
    socket = os.environ["SAIL_DAEMON_SOCKET"]

    print("== 1. failing source: diagnostic on the right line")
    with open(path, "w") as handle:
        handle.write(FAILING)
    client = start(sail, project, [])
    client.notify("textDocument/didOpen", {"textDocument": {
        "uri": uri, "languageId": "rust", "version": 1, "text": FAILING}})
    # `ensures r == 2 * x,` is line 7, columns 13..22 one-based.
    expect_failure(client, uri, 6, "first run")

    print("== 2. fix and save: cleared when queued, still clear after verifying")
    save(client, path, uri, PASSING)
    (_, queued) = client.until(is_status("queued"), "the run after the fix to be queued")
    check(publishes(queued, uri) == [[]], "queued: the diagnostic is cleared exactly once")
    (result, seen) = client.until(is_status("result"), "the result after the fix")
    check(publishes(seen, uri) == [], "nothing is published for the file while or after it verifies")
    params = result["params"]
    check(params["exit_code"] == 0 and params["currentness"] == "current"
          and params["published"] is False and params["summary"]["failed"] == 0
          and params["summary"]["verified"] >= 1,
          f"fixed source verified, current, nothing shown: {params}")

    print("== 3. kill the job daemon: the server reconnects and keeps publishing")
    killed = subprocess.run(["pkill", "-KILL", "-f", "--", f"serve --socket {socket}$"]).returncode
    check(killed == 0, "the daemon serving this session was killed")
    save(client, path, uri, FAILING)
    (_, seen) = client.until(is_status("reconnecting"), "the server to notice the dead service")
    check(client.process.poll() is None, "the server is still running after the failed call")
    expect_failure(client, uri, 6, "run after reconnecting")
    finish(client)

    print("== 4. --stale mark: demoted while pending, then replaced at the new line")
    client = start(sail, project, ["--stale", "mark"])
    expect_failure(client, uri, 6, "first run (mark)")
    save(client, path, uri, "// moved down one line\n" + FAILING)
    (_, queued) = client.until(is_status("queued"), "the next run to be queued")
    stale = publishes(queued, uri)
    check(len(stale) == 1 and len(stale[0]) == 1 and stale[0][0]["severity"] == 3
          and stale[0][0]["message"].startswith("[stale: verification pending] ")
          and stale[0][0]["range"]["start"]["line"] == 6,
          "queued: the old diagnostic is republished as stale information")
    expect_failure(client, uri, 7, "run after moving the function")
    save(client, path, uri, PASSING)
    (_, seen) = client.until(is_status("result"), "the result after the fix (mark)")
    check(publishes(seen, uri)[-1] == [], "fixed: the document ends with no diagnostics")
    finish(client)
    print("PASS")


if __name__ == "__main__":
    main()
