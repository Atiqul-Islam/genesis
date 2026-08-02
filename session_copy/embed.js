#!/usr/bin/env node
/* Session-Copy — Phase 2b: embed captured records into the Genesis memory server for semantic recall.

   Faithful Node (CommonJS, stdlib-only) port of embed.py.

   Feeds each scrubbed record (from records.jsonl, or history.sqlite) into the memory server under the new
   agent's `agent_id`, via the server's `store` MCP tool over stdio (the same protocol the agent uses at runtime).
   After this, the agent's `recall` tool returns the relevant slices of its copied history on demand (spec D5).

   Kept separate from store.js (which is pure/deterministic) because embedding needs the built Rust binary + the
   ONNX model — so this is an INTEGRATION step, tested against the real server (no mocks).

   Run:
     node embed.js --records <dir>/records.jsonl --agent-id <name> \
         --server-bin <genesis>/server/target/release/genesis-memory-server \
         --model-dir <genesis>/server/models --db <repo>/.genesis/agents/<name>/memory.sqlite
*/
"use strict";
const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");
const { DatabaseSync } = require("node:sqlite");

function _load_from_jsonl(p) {
  const out = [];
  let raw;
  try {
    raw = fs.readFileSync(p, "utf8");
  } catch (e) {
    return out;
  }
  for (const rawLine of raw.split("\n")) {
    const line = rawLine.trim();
    if (line) {
      try {
        out.push(JSON.parse(line));
      } catch (e) {
        /* ignore */
      }
    }
  }
  return out;
}

function _load_from_db(p) {
  const con = new DatabaseSync(p, { readOnly: true });
  try {
    const rows = con.prepare("SELECT source, kind, title, text FROM records ORDER BY seq").all();
    return rows.map((r) => ({ source: r.source, kind: r.kind, title: r.title, text: r.text }));
  } finally {
    con.close();
  }
}

function _record_text(r) {
  // Give each stored memory light provenance so a recalled chunk is self-describing.
  const src = r.source || "";
  const title = r.title || "";
  const text = r.text || "";
  const head = `[${src}] ` + (title ? `${title}: ` : "");
  return (head + text).trim();
}

class _Server {
  constructor(server_bin, env) {
    this.p = spawn(server_bin, [], { stdio: ["pipe", "pipe", "inherit"], env: env });
    this._id = 0;
    this._buf = "";
    this._lines = []; // complete lines received but not yet consumed
    this._waiters = []; // resolvers awaiting the next line
    this.p.stdout.setEncoding("utf8");
    this.p.stdout.on("data", (d) => {
      this._buf += d;
      let nl;
      while ((nl = this._buf.indexOf("\n")) !== -1) {
        const line = this._buf.slice(0, nl);
        this._buf = this._buf.slice(nl + 1);
        if (this._waiters.length) {
          this._waiters.shift()(line);
        } else {
          this._lines.push(line);
        }
      }
    });
  }

  _readLine() {
    if (this._lines.length) {
      return Promise.resolve(this._lines.shift());
    }
    return new Promise((resolve) => this._waiters.push(resolve));
  }

  async _rpc(method, params) {
    this._id += 1;
    this.p.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: this._id, method: method, params: params }) + "\n");
    return JSON.parse(await this._readLine());
  }

  _notify(method, params) {
    this.p.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: method, params: params || {} }) + "\n");
  }

  async initialize() {
    await this._rpc("initialize", {
      protocolVersion: "2024-11-05",
      capabilities: {},
      clientInfo: { name: "session-copy-embed", version: "1" },
    });
    this._notify("notifications/initialized");
  }

  async store(agent_id, text) {
    const r = await this._rpc("tools/call", { name: "store", arguments: { agent_id: agent_id, text: text } });
    return !((r.result || {}) || {}).isError;
  }

  close() {
    try {
      this.p.kill();
    } catch (e) {
      /* ignore */
    }
  }
}

async function embed_records(records, agent_id, server_bin, model_dir, db_path, max_chars) {
  if (max_chars === undefined) {
    max_chars = 6000;
  }
  const env = Object.assign({}, process.env);
  env.GENESIS_MODEL_DIR = model_dir;
  env.GENESIS_MEMORY_DB = db_path;
  fs.mkdirSync(path.dirname(path.resolve(db_path)), { recursive: true });
  const srv = new _Server(server_bin, env);
  let stored = 0;
  let failed = 0;
  let skipped = 0;
  try {
    await srv.initialize();
    for (const r of records) {
      let text = _record_text(r);
      if (!text.trim()) {
        skipped += 1;
        continue;
      }
      if (text.length > max_chars) {
        // cap a single huge blob; recall works on the head
        text = text.slice(0, max_chars);
      }
      if (await srv.store(agent_id, text)) {
        stored += 1;
      } else {
        failed += 1;
      }
    }
  } finally {
    srv.close();
  }
  return { agent_id: agent_id, db: db_path, stored: stored, failed: failed, skipped: skipped, total: records.length };
}

// ---- CLI ---------------------------------------------------------------------------------------------
function _parseArgs(argv) {
  const a = {};
  let i = 0;
  while (i < argv.length) {
    const t = argv[i];
    if (t === "--records") {
      a.records = argv[++i];
    } else if (t === "--history-db") {
      a.history_db = argv[++i];
    } else if (t === "--agent-id") {
      a.agent_id = argv[++i];
    } else if (t === "--server-bin") {
      a.server_bin = argv[++i];
    } else if (t === "--model-dir") {
      a.model_dir = argv[++i];
    } else if (t === "--db") {
      a.db = argv[++i];
    }
    i += 1;
  }
  return a;
}

async function main() {
  const a = _parseArgs(process.argv.slice(2));
  if ((!a.records && !a.history_db) || (a.records && a.history_db)) {
    process.stderr.write("error: exactly one of --records / --history-db is required\n");
    process.exit(2);
  }
  if (!a.agent_id || !a.server_bin || !a.model_dir || !a.db) {
    process.stderr.write("usage: node embed.js (--records <f> | --history-db <f>) --agent-id <name> --server-bin <bin> --model-dir <dir> --db <path>\n");
    process.exit(2);
  }
  const recs = a.records ? _load_from_jsonl(a.records) : _load_from_db(a.history_db);
  const res = await embed_records(recs, a.agent_id, a.server_bin, a.model_dir, a.db);
  process.stdout.write(JSON.stringify(res, null, 2) + "\n");
}

if (require.main === module) {
  main();
}

module.exports = { _load_from_jsonl, _load_from_db, _record_text, embed_records, _Server, main };
