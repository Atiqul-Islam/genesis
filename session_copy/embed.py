#!/usr/bin/env python3
"""Session-Copy — Phase 2b: embed captured records into the Genesis memory server for semantic recall.

Feeds each scrubbed record (from records.jsonl, or history.sqlite) into the memory server under the new
agent's `agent_id`, via the server's `store` MCP tool over stdio (the same protocol the agent uses at runtime).
After this, the agent's `recall` tool returns the relevant slices of its copied history on demand (spec D5).

Kept separate from store.py (which is pure/deterministic) because embedding needs the built Rust binary + the
ONNX model — so this is an INTEGRATION step, tested against the real server (no mocks).

Run:
  python3 embed.py --records <dir>/records.jsonl --agent-id <name> \
      --server-bin <genesis>/server/target/release/genesis-memory-server \
      --model-dir <genesis>/server/models --db <repo>/.genesis/agents/<name>/memory.sqlite
"""
import argparse, json, os, sqlite3, subprocess, sys


def _load_from_jsonl(path):
    out = []
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.strip()
            if line:
                try:
                    out.append(json.loads(line))
                except Exception:
                    pass
    return out


def _load_from_db(path):
    con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    rows = con.execute("SELECT source, kind, title, text FROM records ORDER BY seq").fetchall()
    con.close()
    return [{"source": s, "kind": k, "title": t, "text": x} for s, k, t, x in rows]


def _record_text(r):
    """Give each stored memory light provenance so a recalled chunk is self-describing."""
    src, title, text = r.get("source", ""), r.get("title", ""), r.get("text", "")
    head = f"[{src}] " + (f"{title}: " if title else "")
    return (head + text).strip()


class _Server:
    def __init__(self, server_bin, env):
        self.p = subprocess.Popen([server_bin], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  text=True, env=env)
        self._id = 0

    def _rpc(self, method, params):
        self._id += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self._id, "method": method,
                                       "params": params}) + "\n")
        self.p.stdin.flush()
        return json.loads(self.p.stdout.readline())

    def _notify(self, method, params=None):
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method, "params": params or {}}) + "\n")
        self.p.stdin.flush()

    def initialize(self):
        self._rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                 "clientInfo": {"name": "session-copy-embed", "version": "1"}})
        self._notify("notifications/initialized")

    def store(self, agent_id, text):
        r = self._rpc("tools/call", {"name": "store", "arguments": {"agent_id": agent_id, "text": text}})
        return not (r.get("result", {}) or {}).get("isError", False)

    def close(self):
        try:
            self.p.terminate()
        except Exception:
            pass


def embed_records(records, agent_id, server_bin, model_dir, db_path, max_chars=6000):
    env = dict(os.environ)
    env["GENESIS_MODEL_DIR"] = model_dir
    env["GENESIS_MEMORY_DB"] = db_path
    os.makedirs(os.path.dirname(os.path.abspath(db_path)), exist_ok=True)
    srv = _Server(server_bin, env)
    stored = failed = skipped = 0
    try:
        srv.initialize()
        for r in records:
            text = _record_text(r)
            if not text.strip():
                skipped += 1
                continue
            if len(text) > max_chars:      # cap a single huge blob; recall works on the head
                text = text[:max_chars]
            if srv.store(agent_id, text):
                stored += 1
            else:
                failed += 1
    finally:
        srv.close()
    return {"agent_id": agent_id, "db": db_path, "stored": stored, "failed": failed, "skipped": skipped,
            "total": len(records)}


def main():
    ap = argparse.ArgumentParser(description="Embed captured session records into the Genesis memory server.")
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--records", help="records.jsonl from capture.py")
    src.add_argument("--history-db", help="history.sqlite from store.py")
    ap.add_argument("--agent-id", required=True)
    ap.add_argument("--server-bin", required=True)
    ap.add_argument("--model-dir", required=True)
    ap.add_argument("--db", required=True, help="the agent's memory.sqlite (GENESIS_MEMORY_DB)")
    a = ap.parse_args()
    recs = _load_from_jsonl(a.records) if a.records else _load_from_db(a.history_db)
    print(json.dumps(embed_records(recs, a.agent_id, a.server_bin, a.model_dir, a.db), indent=2))


if __name__ == "__main__":
    main()
