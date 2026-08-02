#!/usr/bin/env node
/* Session-Copy — Phase 1: capture/extract library (spec: docs/SESSION_COPY_AGENT_SPEC.md).

   Faithful Node (CommonJS, stdlib-only) port of capture.py.

   Extracts the readable CONTENT of every way Claude Code holds context/memory for a session, into ONE
   normalized, credential-scrubbed record stream. We EXTRACT text rather than copy native plugin DBs — so the
   result is portable (a target machine needs none of the source plugins to read it) and uniform for embedding
   into the Genesis memory server (Phase 2).

   Stores captured (verified on-disk schemas 2026-07-22):
     A. transcript      ~/.claude/projects/<enc>/<session>.jsonl  (user/assistant/system turns)
     B3 auto-memory     <same project dir>/memory/*.md            (MEMORY.md + topic files)
     B5a context-mode   ~/.claude/context-mode/content/*.db       (table `chunks(content, session_id, ...)`)
     B5b claude-mem     ~/.claude/projects/(*claude-mem-observer*)/(*.jsonl) (records with a `content` field)
     B4 genesis-memory  <GENESIS_MEMORY_DB>                        (only if the current agent already has one)
     C7 user config     ~/.claude/{CLAUDE.md, settings.json, skills-recursive-SKILL.md}  (snapshot; scrubbed hard)

   Every extracted text is CREDENTIAL-SCRUBBED before it leaves this module — a matched secret becomes
   "[REDACTED credential]"; the value is never written or returned. (Hard workspace rule.)

   Output: <out_dir>/records.jsonl  (one normalized record per line) + <out_dir>/capture_manifest.json.
   A normalized record: {"source","kind","ts","title","text"}.

   CLI:  node capture.js --session <id> --cwd <repo> --out <dir> [--genesis-db <path>] [--agent-id <name>]
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const { DatabaseSync } = require("node:sqlite");

// ---- credential scrubbing (reused patterns from hooks/gate.py + validate.py) --------------------------
const _SECRET_PATTERNS = [
  /AKIA[0-9A-Z]{16}/g,
  /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----[\s\S]*?-----END (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/g,
  // matches key=value, key: value, AND the JSON form "key": "value" (optional quote before the : / =)
  /(\b(?:password|passwd|secret|api[_-]?key|token|authorization|bearer)\b['"]?\s*[:=]\s*['"]?)([^\s'"]{6,})/gi,
  /gh[pousr]_[A-Za-z0-9]{20,}/g, // GitHub tokens
  /sk-[A-Za-z0-9]{20,}/g, //         OpenAI-style keys
  /xox[baprs]-[A-Za-z0-9-]{10,}/g, //Slack tokens
];
function _groupCount(rx) {
  // Number of capturing groups in a RegExp (mirrors Python's rx.groups).
  return new RegExp(rx.source + "|").exec("").length - 1;
}
const _SECRET_GROUPS = _SECRET_PATTERNS.map(_groupCount);
const _REDACTED = "[REDACTED credential]";
let _KNOWN_SECRETS = []; // exact values to guarantee-redact; set per-capture via capture(known_secrets=...)

function scrub_text(text, known) {
  // Return [scrubbed, n_redacted]. Never returns a matched secret value.
  // Redacts: caller-supplied exact `known` values (guaranteed) + known shapes + labelled secrets.
  if (!text) {
    return [text || "", 0];
  }
  let n = 0;
  let out = text;
  const knownList = known !== undefined && known !== null ? known : _KNOWN_SECRETS;
  for (const val of knownList) {
    if (val && out.includes(val)) {
      out = out.split(val).join(_REDACTED);
      n += 1;
    }
  }
  for (let i = 0; i < _SECRET_PATTERNS.length; i++) {
    const rx = _SECRET_PATTERNS[i];
    if (_SECRET_GROUPS[i] >= 2) {
      // keep the label, redact the value
      out = out.replace(rx, (m, g1) => {
        n += 1;
        return g1 + _REDACTED;
      });
    } else {
      const matches = out.match(rx);
      const k = matches ? matches.length : 0;
      n += k;
      out = out.replace(rx, _REDACTED);
    }
  }
  return [out, n];
}

// ---- Python-compatible compact JSON (separators ", " / ": ", non-ASCII kept literally) ---------------
function _pyCompact(v) {
  if (v === null || v === undefined) {
    return "null";
  }
  const t = typeof v;
  if (t === "string") {
    return JSON.stringify(v);
  }
  if (t === "number") {
    return String(v);
  }
  if (t === "boolean") {
    return v ? "true" : "false";
  }
  if (Array.isArray(v)) {
    return "[" + v.map(_pyCompact).join(", ") + "]";
  }
  if (t === "object") {
    return "{" + Object.keys(v).map((k) => JSON.stringify(k) + ": " + _pyCompact(v[k])).join(", ") + "}";
  }
  return "null";
}

// ---- path resolution (encoding-agnostic: find by the session id itself) ------------------------------
function claude_home() {
  return process.env.CLAUDE_CONFIG_DIR || path.join(os.homedir(), ".claude");
}

function _listDirs(dir) {
  // Immediate subdirectories of `dir` (non-hidden), sorted. Mirrors what glob `*/` would enumerate.
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch (e) {
    return [];
  }
  return entries
    .filter((e) => e.isDirectory() && !e.name.startsWith("."))
    .map((e) => e.name)
    .sort();
}

function find_transcript(session_id, cwd) {
  // Locate <session>.jsonl by globbing every project dir (robust to path-encoding differences).
  const home = claude_home();
  const projects = path.join(home, "projects");
  for (const d of _listDirs(projects)) {
    const cand = path.join(projects, d, `${session_id}.jsonl`);
    if (isFile(cand)) {
      return cand;
    }
  }
  return null;
}

function project_dir_for(transcript_path) {
  // The ~/.claude/projects/<enc>/ dir that holds a transcript (auto-memory is its sibling).
  return transcript_path ? path.dirname(transcript_path) : null;
}

function isFile(p) {
  try {
    return fs.statSync(p).isFile();
  } catch (e) {
    return false;
  }
}
function isDir(p) {
  try {
    return fs.statSync(p).isDirectory();
  } catch (e) {
    return false;
  }
}
function readText(p) {
  // errors="replace" parity: Node's utf8 decoder substitutes U+FFFD for invalid bytes.
  return fs.readFileSync(p, "utf8");
}

// ---- record helper -----------------------------------------------------------------------------------
function _rec(source, kind, ts, title, text) {
  const [scrubbed, n] = scrub_text(typeof text === "string" ? text : _pyCompact(text));
  return [{ source: source, kind: kind, ts: ts || "", title: title || "", text: scrubbed }, n];
}

function _text_from_content(content) {
  // Flatten an Anthropic message `content` (str | list of blocks) into readable text.
  if (typeof content === "string") {
    return content;
  }
  const parts = [];
  if (Array.isArray(content)) {
    for (const b of content) {
      if (b === null || typeof b !== "object" || Array.isArray(b)) {
        parts.push(String(b));
        continue;
      }
      const t = b.type;
      if (t === "text") {
        parts.push(b.text || "");
      } else if (t === "tool_use") {
        parts.push(`[tool_use ${b.name || ""}] ` + _pyCompact(b.input !== undefined ? b.input : {}).slice(0, 2000));
      } else if (t === "tool_result") {
        const c = b.content !== undefined ? b.content : "";
        parts.push("[tool_result] " + (typeof c !== "string" ? _text_from_content(c) : c).slice(0, 2000));
      } else if (t === "thinking") {
        parts.push("[thinking] " + (b.thinking || "").slice(0, 2000));
      }
    }
  }
  return parts.filter((p) => p).join("\n");
}

function _iterJsonl(p) {
  // Yield parsed JSON objects from a jsonl file (skipping blanks + unparseable lines); [] on any read error.
  let raw;
  try {
    raw = fs.readFileSync(p, "utf8");
  } catch (e) {
    return [];
  }
  const out = [];
  for (const rawLine of raw.split("\n")) {
    const line = rawLine.trim();
    if (!line) {
      continue;
    }
    try {
      out.push(JSON.parse(line));
    } catch (e) {
      continue;
    }
  }
  return out;
}

// ---- extractors: each returns [records, n_redacted] --------------------------------------------------
function extract_transcript(transcript_path) {
  // A: user/assistant/system conversation turns from the session .jsonl.
  const recs = [];
  let red = 0;
  if (!transcript_path || !isFile(transcript_path)) {
    return [recs, red];
  }
  let raw;
  try {
    raw = fs.readFileSync(transcript_path, "utf8");
  } catch (e) {
    return [recs, red];
  }
  for (const rawLine of raw.split("\n")) {
    const line = rawLine.trim();
    if (!line) {
      continue;
    }
    let ev;
    try {
      ev = JSON.parse(line);
    } catch (e) {
      continue;
    }
    const typ = ev.type;
    if (typ !== "user" && typ !== "assistant" && typ !== "system") {
      continue;
    }
    const ts = ev.timestamp || "";
    let text;
    if (typ === "system") {
      text = typeof ev.content === "string" ? ev.content : _text_from_content(ev.content !== undefined ? ev.content : "");
    } else {
      const msg = ev.message || {};
      text = _text_from_content(msg.content !== undefined ? msg.content : "");
    }
    if (!text.trim()) {
      continue;
    }
    const [r, n] = _rec("transcript", typ, ts, "", text);
    recs.push(r);
    red += n;
  }
  return [recs, red];
}

function extract_auto_memory(project_dir) {
  // B3: MEMORY.md + topic files (already markdown).
  const recs = [];
  let red = 0;
  const mem = path.join(project_dir || "", "memory");
  if (!isDir(mem)) {
    return [recs, red];
  }
  let names;
  try {
    names = fs.readdirSync(mem).sort();
  } catch (e) {
    return [recs, red];
  }
  for (const name of names) {
    const p = path.join(mem, name);
    if (!(isFile(p) && name.endsWith(".md"))) {
      continue;
    }
    let txt;
    try {
      txt = readText(p);
    } catch (e) {
      continue;
    }
    const [r, n] = _rec("auto-memory", "memory-file", "", name, txt);
    recs.push(r);
    red += n;
  }
  return [recs, red];
}

function extract_contextmode(session_id, home) {
  // B5a: readable text from context-mode content DBs — `chunks(title,content,session_id)` for THIS session.
  const recs = [];
  let red = 0;
  home = home || claude_home();
  const contentDir = path.join(home, "context-mode", "content");
  let files;
  try {
    files = fs
      .readdirSync(contentDir)
      .filter((f) => f.endsWith(".db") && !f.startsWith("."))
      .sort();
  } catch (e) {
    return [recs, red];
  }
  for (const f of files) {
    const db = path.join(contentDir, f);
    if (!isFile(db)) {
      continue;
    }
    let c;
    try {
      c = new DatabaseSync(db, { readOnly: true });
      const has = c.prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='chunks'").get();
      if (!has) {
        c.close();
        continue;
      }
      const rows = c.prepare("SELECT title, content, timestamp FROM chunks WHERE session_id=?").all(session_id);
      for (const row of rows) {
        const [r, n] = _rec("context-mode", "chunk", row.timestamp || "", row.title || "", row.content || "");
        recs.push(r);
        red += n;
      }
      c.close();
    } catch (e) {
      try {
        if (c) {
          c.close();
        }
      } catch (e2) {
        /* ignore */
      }
      continue;
    }
  }
  return [recs, red];
}

function extract_claude_mem(session_id, home) {
  // B5b: observation `content` from claude-mem observer jsonl, scoped to EXACTLY this session.
  //
  // HONEST SCOPE (verified 2026-07-22): the observer jsonl carries the OBSERVER's own sessionId, not the main
  // session's, and no project field — so from files we can only reliably capture records whose `sessionId`
  // equals the given session. We do NOT fall back to "keep everything": that dumped the entire cross-project
  // claude-mem corpus (~13k records / ~85MB in testing). COMPLETE project-scoped claude-mem capture requires
  // claude-mem's own MCP search API and is done by the LIVE in-session capture step (Phase 3).
  const recs = [];
  let red = 0;
  home = home || claude_home();
  const projects = path.join(home, "projects");
  const files = [];
  for (const d of _listDirs(projects)) {
    // fnmatch("*claude-mem*observer*"): name contains "claude-mem" then "observer" after it.
    const idx = d.indexOf("claude-mem");
    if (idx === -1 || d.indexOf("observer", idx + "claude-mem".length) === -1) {
      continue;
    }
    const dir = path.join(projects, d);
    let inner;
    try {
      inner = fs.readdirSync(dir).filter((f) => f.endsWith(".jsonl") && !f.startsWith("."));
    } catch (e) {
      continue;
    }
    for (const f of inner) {
      const fp = path.join(dir, f);
      if (isFile(fp)) {
        files.push(fp);
      }
    }
  }
  files.sort();
  for (const f of files) {
    for (const ev of _iterJsonl(f)) {
      const content = ev.content;
      if (!content || ev.sessionId !== session_id) {
        continue;
      }
      const [r, n] = _rec("claude-mem", ev.operation || "observation", ev.timestamp || "", "session-match", content);
      recs.push(r);
      red += n;
    }
  }
  return [recs, red];
}

function extract_genesis_memory(db_path, agent_id) {
  // B4: the current agent's own Genesis semantic memory rows, if it has any.
  const recs = [];
  let red = 0;
  if (!db_path || !isFile(db_path)) {
    return [recs, red];
  }
  let c;
  try {
    c = new DatabaseSync(db_path, { readOnly: true });
    const tbls = new Set(
      c.prepare("SELECT name FROM sqlite_master WHERE type='table'").all().map((r) => r.name)
    );
    const tbl = tbls.has("memories") ? "memories" : tbls.has("memory") ? "memory" : null;
    if (tbl) {
      const cols = c.prepare(`PRAGMA table_info('${tbl}')`).all().map((r) => r.name);
      const textcol = ["text", "content", "body"].find((x) => cols.includes(x)) || null;
      const idcol = ["agent_id", "agent"].find((x) => cols.includes(x)) || null;
      if (textcol) {
        let q = `SELECT ${textcol} FROM ${tbl}`;
        let args = [];
        if (idcol && agent_id) {
          q += ` WHERE ${idcol}=?`;
          args = [agent_id];
        }
        for (const row of c.prepare(q).all(...args)) {
          const [r, n] = _rec("genesis-memory", "memory", "", "", row[textcol] || "");
          recs.push(r);
          red += n;
        }
      }
    }
    c.close();
  } catch (e) {
    try {
      if (c) {
        c.close();
      }
    } catch (e2) {
      /* ignore */
    }
  }
  return [recs, red];
}

function _globSkills(root) {
  // Recursive walk collecting files named SKILL.md under `root` (non-hidden dirs/files), full paths sorted.
  const out = [];
  function walk(dir) {
    let entries;
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch (e) {
      return;
    }
    for (const e of entries) {
      if (e.name.startsWith(".")) {
        continue;
      }
      const p = path.join(dir, e.name);
      if (e.isDirectory()) {
        walk(p);
      } else if (e.isFile() && e.name === "SKILL.md") {
        out.push(p);
      }
    }
  }
  walk(root);
  out.sort();
  return out;
}

function extract_user_config(home) {
  // C7: user-level/global config snapshot (scrubbed). settings.json values are scrubbed hard.
  const recs = [];
  let red = 0;
  home = home || claude_home();
  for (const rel of ["CLAUDE.md"]) {
    const p = path.join(home, rel);
    if (isFile(p)) {
      const [r, n] = _rec("user-config", "claude-md", "", rel, readText(p));
      recs.push(r);
      red += n;
    }
  }
  const sj = path.join(home, "settings.json");
  if (isFile(sj)) {
    const raw = readText(sj);
    const [r, n] = _rec("user-config", "settings", "", "settings.json", raw); // scrub_text redacts secret values
    recs.push(r);
    red += n;
  }
  for (const skill of _globSkills(path.join(home, "skills"))) {
    let txt;
    try {
      txt = readText(skill);
    } catch (e) {
      continue;
    }
    const [r, n] = _rec("user-config", "skill", "", path.relative(home, skill), txt);
    recs.push(r);
    red += n;
  }
  return [recs, red];
}

// ---- orchestration -----------------------------------------------------------------------------------
function capture(session_id, cwd, out_dir, genesis_db, agent_id, include_user_config, known_secrets) {
  if (include_user_config === undefined) {
    include_user_config = true;
  }
  _KNOWN_SECRETS = Array.from(known_secrets || []);
  const transcript = find_transcript(session_id, cwd);
  const proj = project_dir_for(transcript);
  const extractors = [
    ["transcript", () => extract_transcript(transcript)],
    ["auto-memory", () => extract_auto_memory(proj)],
    ["context-mode", () => extract_contextmode(session_id)],
    ["claude-mem", () => extract_claude_mem(session_id)],
    ["genesis-memory", () => extract_genesis_memory(genesis_db, agent_id)],
  ];
  if (include_user_config) {
    extractors.push(["user-config", () => extract_user_config()]);
  }

  fs.mkdirSync(out_dir, { recursive: true });
  const per_source = {};
  let total = 0;
  let total_red = 0;
  const out_path = path.join(out_dir, "records.jsonl");
  const chunks = [];
  for (const [name, fn] of extractors) {
    let recs;
    let red;
    try {
      [recs, red] = fn();
    } catch (e) {
      per_source[name] = { records: 0, redactions: 0, error: `${e && e.name ? e.name : "Error"}: ${e && e.message ? e.message : e}` };
      continue;
    }
    for (const r of recs) {
      chunks.push(_pyCompact(r) + "\n");
    }
    per_source[name] = { records: recs.length, redactions: red };
    total += recs.length;
    total_red += red;
  }
  fs.writeFileSync(out_path, chunks.join(""), "utf8");

  const manifest = {
    session_id: session_id,
    cwd: cwd ? path.resolve(cwd) : null,
    transcript: transcript,
    project_dir: proj,
    records_file: out_path,
    total_records: total,
    total_redactions: total_red,
    by_source: per_source,
  };
  fs.writeFileSync(path.join(out_dir, "capture_manifest.json"), JSON.stringify(manifest, null, 2) + "\n", "utf8");
  return manifest;
}

// ---- CLI ---------------------------------------------------------------------------------------------
function _parseArgs(argv) {
  const a = { known_secret: [] };
  let i = 0;
  while (i < argv.length) {
    const t = argv[i];
    if (t === "--session") {
      a.session = argv[++i];
    } else if (t === "--cwd") {
      a.cwd = argv[++i];
    } else if (t === "--out") {
      a.out = argv[++i];
    } else if (t === "--genesis-db") {
      a.genesis_db = argv[++i];
    } else if (t === "--agent-id") {
      a.agent_id = argv[++i];
    } else if (t === "--no-user-config") {
      a.no_user_config = true;
    } else if (t === "--known-secret") {
      a.known_secret.push(argv[++i]);
    }
    i += 1;
  }
  return a;
}

function main() {
  const a = _parseArgs(process.argv.slice(2));
  if (!a.session || !a.out) {
    process.stderr.write("usage: node capture.js --session <id> --out <dir> [--cwd <repo>] [--genesis-db <path>] [--agent-id <name>] [--no-user-config] [--known-secret V ...]\n");
    process.exit(2);
  }
  const m = capture(a.session, a.cwd !== undefined ? a.cwd : process.cwd(), a.out, a.genesis_db || null, a.agent_id || null, !a.no_user_config, a.known_secret);
  // Print the manifest (by_source last, matching capture.py's CLI output).
  const { by_source, ...rest } = m;
  process.stdout.write(JSON.stringify({ ...rest, by_source }, null, 2) + "\n");
}

if (require.main === module) {
  main();
}

module.exports = {
  scrub_text,
  claude_home,
  find_transcript,
  project_dir_for,
  _rec,
  _text_from_content,
  extract_transcript,
  extract_auto_memory,
  extract_contextmode,
  extract_claude_mem,
  extract_genesis_memory,
  extract_user_config,
  capture,
  main,
  _pyCompact,
};
