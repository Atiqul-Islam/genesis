#!/usr/bin/env node
/* Session-Copy — Phase 2: store + summary (spec: docs/SESSION_COPY_AGENT_SPEC.md §6).

   Faithful Node (CommonJS, stdlib-only) port of store.py.

   Takes the scrubbed `records.jsonl` from Phase 1 and produces the new agent's PORTABLE bundle under
   <repo>/.genesis/agents/<name>/:
     history.sqlite   — the captured records in a clean, queryable table (travels with the repo; no plugin needed).
     summary.md       — a DETERMINISTIC running digest injected at session start (D5). An LLM can enrich it later
                        (Phase 3 live step); the deterministic baseline guarantees a useful, testable summary now.
     (embedding of the records into the Genesis memory server for semantic recall is embed.js, run separately,
      because it needs the built server binary — kept out of this pure/deterministic module so it is unit-testable.)

   Deterministic + dependency-free (Node stdlib only, incl. node:sqlite). Run:
     node store.js --records <dir>/records.jsonl --out <repo>/.genesis/agents/<name>
*/
"use strict";
const fs = require("fs");
const path = require("path");
const { DatabaseSync } = require("node:sqlite");

function load_records(records_path) {
  const recs = [];
  let raw;
  try {
    raw = fs.readFileSync(records_path, "utf8");
  } catch (e) {
    return recs;
  }
  for (const rawLine of raw.split("\n")) {
    const line = rawLine.trim();
    if (!line) {
      continue;
    }
    try {
      recs.push(JSON.parse(line));
    } catch (e) {
      continue;
    }
  }
  return recs;
}

function write_history_db(recs, db_path) {
  // Portable SQLite: one row per captured record. Overwrites cleanly; travels with the repo.
  for (const p of [db_path, db_path + "-wal", db_path + "-shm", db_path + "-journal"]) {
    try {
      if (fs.existsSync(p)) {
        fs.rmSync(p);
      }
    } catch (e) {
      /* ignore */
    }
  }
  const con = new DatabaseSync(db_path);
  try {
    con.exec(
      "CREATE TABLE records (\n            seq INTEGER PRIMARY KEY, source TEXT, kind TEXT, ts TEXT, title TEXT, text TEXT)"
    );
    con.exec("CREATE INDEX idx_source ON records(source)");
    const ins = con.prepare("INSERT INTO records (seq, source, kind, ts, title, text) VALUES (?,?,?,?,?,?)");
    con.exec("BEGIN");
    for (let i = 0; i < recs.length; i++) {
      const r = recs[i];
      ins.run(i, r.source || "", r.kind || "", r.ts || "", r.title || "", r.text || "");
    }
    con.exec("COMMIT");
  } finally {
    con.close();
  }
}

function _counts(recs) {
  const c = {};
  for (const r of recs) {
    const k = r.source !== undefined && r.source !== null ? r.source : "?";
    c[k] = (c[k] || 0) + 1;
  }
  return c;
}

function _splitWs(s) {
  // Mirror Python str.split() (no args): split on any whitespace run, drop empties.
  return (s || "").split(/\s+/).filter((x) => x !== "");
}

function build_summary(recs, agent_name, recent_turns, memory_chars) {
  // A deterministic, portable digest: what was captured + the current MEMORY.md + the most recent turns.
  // This is the baseline injected at start; a live LLM pass can replace/augment it (Phase 3).
  if (recent_turns === undefined) {
    recent_turns = 12;
  }
  if (memory_chars === undefined) {
    memory_chars = 4000;
  }
  const counts = _counts(recs);
  const lines = [`# Session-copy memory — ${agent_name}`, ""];
  lines.push(
    "You were created by copying a prior Claude Code session. Its full history + memory is in your " +
      "portable store (`history.sqlite`) and is semantically recallable via your memory tools. This " +
      "summary is the always-loaded digest; recall specifics on demand."
  );
  lines.push("");
  lines.push("## What was carried over");
  for (const src of ["transcript", "auto-memory", "context-mode", "claude-mem", "genesis-memory", "user-config"]) {
    if (Object.prototype.hasOwnProperty.call(counts, src)) {
      lines.push(`- ${src}: ${counts[src]} records`);
    }
  }
  lines.push("");

  // The prior session's index memory (MEMORY.md), if captured — the highest-signal standing context.
  const mem = recs.find((r) => r.source === "auto-memory" && r.title === "MEMORY.md") || null;
  if (mem) {
    const body = (mem.text || "").trim();
    lines.push("## Standing memory (from the prior session's MEMORY.md)");
    lines.push(body.slice(0, memory_chars) + (body.length > memory_chars ? "\n…(truncated — recall the rest)" : ""));
    lines.push("");
  }

  // The tail of the conversation — most recent turns, compact.
  const turns = recs.filter((r) => r.source === "transcript" && (r.kind === "user" || r.kind === "assistant"));
  if (turns.length) {
    lines.push(`## Most recent ${Math.min(recent_turns, turns.length)} turns (tail of the prior conversation)`);
    for (const r of turns.slice(-recent_turns)) {
      const who = r.kind === "assistant" ? "You" : "User";
      const snippet = _splitWs(r.text || "").join(" ").slice(0, 240);
      lines.push(`- **${who}:** ${snippet}`);
    }
    lines.push("");
  }
  lines.push("_Recall any detail from the full history with your memory tools — it is all stored._");
  return lines.join("\n");
}

function build_bundle(records_path, out_dir, agent_name) {
  agent_name = agent_name || path.basename(path.normalize(out_dir)) || "agent";
  fs.mkdirSync(out_dir, { recursive: true });
  const recs = load_records(records_path);
  const db_path = path.join(out_dir, "history.sqlite");
  write_history_db(recs, db_path);
  const summary = build_summary(recs, agent_name);
  const summary_path = path.join(out_dir, "summary.md");
  fs.writeFileSync(summary_path, summary + "\n", "utf8");
  const manifest = {
    agent: agent_name,
    records: recs.length,
    by_source: _counts(recs),
    history_db: db_path,
    summary: summary_path,
  };
  fs.writeFileSync(path.join(out_dir, "store_manifest.json"), JSON.stringify(manifest, null, 2) + "\n", "utf8");
  return manifest;
}

// ---- CLI ---------------------------------------------------------------------------------------------
function _parseArgs(argv) {
  const a = {};
  let i = 0;
  while (i < argv.length) {
    const t = argv[i];
    if (t === "--records") {
      a.records = argv[++i];
    } else if (t === "--out") {
      a.out = argv[++i];
    } else if (t === "--name") {
      a.name = argv[++i];
    }
    i += 1;
  }
  return a;
}

function main() {
  const a = _parseArgs(process.argv.slice(2));
  if (!a.records || !a.out) {
    process.stderr.write("usage: node store.js --records <dir>/records.jsonl --out <repo>/.genesis/agents/<name> [--name <name>]\n");
    process.exit(2);
  }
  process.stdout.write(JSON.stringify(build_bundle(a.records, a.out, a.name || null), null, 2) + "\n");
}

if (require.main === module) {
  main();
}

module.exports = { load_records, write_history_db, build_summary, build_bundle, _counts, main };
