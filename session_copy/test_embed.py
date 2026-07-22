#!/usr/bin/env python3
"""Integration test for session_copy/embed.py (Phase 2b) — against the REAL Genesis memory server binary +
ONNX model (no mocks). Proves: captured records embed under the agent's id, then are SEMANTICALLY RECALLABLE,
and are per-agent isolated. Skips (does not fail) if the binary/model aren't built.
Run: python3 session_copy/test_embed.py
"""
import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
GH = os.path.dirname(HERE)
sys.path.insert(0, HERE)
import embed as E  # noqa: E402

BIN = os.path.join(GH, "server", "target", "release", "genesis-memory-server")
MODEL = os.path.join(GH, "server", "models")

RECS = [
    {"source": "transcript", "kind": "assistant", "title": "",
     "text": "The release manager never pushes to main when CI is red."},
    {"source": "auto-memory", "kind": "memory-file", "title": "MEMORY.md",
     "text": "Atiqul prefers concise bullet replies, about 20 words each."},
    {"source": "context-mode", "kind": "chunk", "title": "Rust",
     "text": "The memory server uses sqlite-vec for KNN over 384-dim embeddings."},
]


def _recall(agent_id, query, db, k=2):
    env = dict(os.environ); env["GENESIS_MODEL_DIR"] = MODEL; env["GENESIS_MEMORY_DB"] = db
    p = subprocess.Popen([BIN], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env)
    def rpc(o): p.stdin.write(json.dumps(o) + "\n"); p.stdin.flush(); return json.loads(p.stdout.readline())
    try:
        rpc({"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}})
        p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); p.stdin.flush()
        r = rpc({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                 "params": {"name": "recall", "arguments": {"agent_id": agent_id, "query": query, "k": k}}})
        return json.dumps(r.get("result", {}))
    finally:
        p.terminate()


def main():
    if not (os.path.exists(BIN) and os.path.isdir(MODEL)):
        print(f"  SKIP  memory server binary/model not built ({BIN}) — build with: cd server && cargo build --release")
        print("\n0 passed, 0 failed (skipped)")
        return

    passed = failed = 0
    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    db = os.path.join(tempfile.mkdtemp(prefix="embed-"), "memory.sqlite")
    res = E.embed_records(RECS, "copybot", BIN, MODEL, db)
    check("all records embedded (none failed)", res["stored"] == len(RECS) and res["failed"] == 0)

    # semantic recall — a paraphrase must surface the right record
    hit = _recall("copybot", "when is it not allowed to push code?", db)
    check("recall surfaces the CI-red rule (semantic paraphrase)", "CI is red" in hit)
    hit2 = _recall("copybot", "how does the vector search work?", db)
    check("recall surfaces the sqlite-vec chunk", "sqlite-vec" in hit2)

    # per-agent isolation — a different agent sees nothing
    other = _recall("someone-else", "when is it not allowed to push code?", db)
    check("per-agent isolation (other agent sees none)", "CI is red" not in other)

    import shutil
    shutil.rmtree(os.path.dirname(db), ignore_errors=True)
    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
