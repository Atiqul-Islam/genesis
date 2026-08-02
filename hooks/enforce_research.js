#!/usr/bin/env node
/* ENFORCE-RESEARCH hook (PreToolUse: Bash) — Sensei-scoped.

   Faithful Node (CommonJS, stdlib-only) port of enforce_research.py. BLOCKS the assembler Bash call
   that builds a NON-builtin agent unless the session transcript shows the `research-expertise` Skill was
   invoked this session. Self-guards: no-op unless a genesis agent is active (payload agent_type).

   Decision:
     * command is not an assembler <src> <name> ... invocation          -> allow (not our concern)
     * the assembled agent name is a built-in (sensei/method)           -> allow
     * transcript shows a `research-expertise` Skill tool_use           -> allow
     * otherwise (incl. transcript missing/unreadable)                  -> DENY (fail-closed)
*/
"use strict";
const fs = require("fs");
const path = require("path");
const agent_ident = require("./agent_ident.js"); // sibling helper; travels with hooks/ in every install

const BUILTINS = new Set(["sensei", "method"]);
const SKILL_NAME = "research-expertise";
const HOOK_DIR = __dirname;
const ASSEMBLER = "assemble" + ".js"; // the assembler script basename the enforcer keys on (Node)
// WRITABLE runtime data -> the PROJECT's .genesis/ (cwd-derived), never the plugin dir.
const LOG = path.join(agent_ident.runtime_dir(), "hook-decisions.log");

function readStdin() {
  try {
    return fs.readFileSync(0, "utf8");
  } catch (e) {
    if (e && e.code === "EAGAIN") {
      const buf = Buffer.alloc(65536);
      const chunks = [];
      for (;;) {
        let n;
        try { n = fs.readSync(0, buf, 0, buf.length, null); }
        catch (e2) {
          if (e2 && e2.code === "EAGAIN") continue;
          if (e2 && e2.code === "EOF") break;
          return "";
        }
        if (!n) break;
        chunks.push(Buffer.from(buf.slice(0, n)));
      }
      return Buffer.concat(chunks).toString("utf8");
    }
    return "";
  }
}

function emit(obj) { fs.writeSync(1, JSON.stringify(obj) + "\n"); }
function nowIso() { return new Date().toISOString().slice(0, 19) + "+00:00"; }
function isFile(p) { try { return fs.statSync(p).isFile(); } catch (e) { return false; } }

function log(decision, agent, reason) {
  reason = reason || "";
  try {
    fs.mkdirSync(path.dirname(LOG), { recursive: true });
    const rec = { ts: nowIso(), hook: "enforce_research", decision: decision, agent: agent, reason: reason };
    fs.appendFileSync(LOG, JSON.stringify(rec) + "\n", { encoding: "utf8" });
  } catch (e) { /* ignore */ }
}

function deny(agent, reason) {
  log("deny", agent, reason);
  emit({ hookSpecificOutput: { hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: reason } });
  process.exit(0);
}

function allow(agent, note) {
  if (agent) log("allow", agent, note || "");
  process.exit(0); // stay silent; let normal permissions decide
}

// POSIX-mode shlex.split emulation. Throws on an unbalanced quote (Python raises ValueError).
function shlexSplit(s) {
  const tokens = [];
  let cur = "", inToken = false;
  let i = 0;
  const n = s.length;
  while (i < n) {
    const c = s[i];
    if (c === "\\") {
      inToken = true;
      i++;
      if (i < n) { cur += s[i]; i++; }
      else { cur += "\\"; }
      continue;
    }
    if (c === "'") {
      inToken = true;
      i++;
      const j = s.indexOf("'", i);
      if (j === -1) throw new Error("No closing quotation");
      cur += s.slice(i, j);
      i = j + 1;
      continue;
    }
    if (c === '"') {
      inToken = true;
      i++;
      let closed = false;
      while (i < n) {
        const d = s[i];
        if (d === "\\") {
          i++;
          if (i < n) {
            const e = s[i];
            if (e === '"' || e === "\\" || e === "$" || e === "`" || e === "\n") cur += e;
            else cur += "\\" + e;
            i++;
          } else { cur += "\\"; }
        } else if (d === '"') { closed = true; i++; break; }
        else { cur += d; i++; }
      }
      if (!closed) throw new Error("No closing quotation");
      continue;
    }
    if (/\s/.test(c)) {
      if (inToken) { tokens.push(cur); cur = ""; inToken = false; }
      i++;
      continue;
    }
    inToken = true;
    cur += c;
    i++;
  }
  if (inToken) tokens.push(cur);
  return tokens;
}

function assembledAgentName(command) {
  // If `command` is an assembler <src> <name> <target> <gh> invocation, return <name>, else null.
  let toks;
  try { toks = shlexSplit(command); }
  catch (e) { toks = command.trim().split(/\s+/).filter(Boolean); }
  for (let i = 0; i < toks.length; i++) {
    const t = toks[i].replace(/\\/g, "/");
    if (t.endsWith(ASSEMBLER)) {
      // positional args after the script: src(i+1), name(i+2), target(i+3), gh(i+4)
      if (i + 2 < toks.length) return toks[i + 2];
      return ""; // malformed assembler call -> treat as unknown (fail-closed below)
    }
  }
  return null;
}

function researchSkillUsed(transcript_path) {
  // true if the transcript shows a Skill tool_use naming research-expertise; null if the path was given
  // but unreadable (a real error -> fail closed); false if not provided/absent/not-used.
  if (!transcript_path || !isFile(transcript_path)) return false;
  let text;
  try { text = fs.readFileSync(transcript_path, "utf8"); }
  catch (e) { return null; }
  try {
    const lines = text.split(/\r\n|\r|\n/);
    for (const line of lines) {
      let ev;
      try { ev = JSON.parse(line); } catch (e) { continue; }
      if (!ev || ev.type !== "assistant") continue;
      const content = (ev.message && ev.message.content) || "";
      const blocks = Array.isArray(content) ? content : [];
      for (const b of blocks) {
        if (b && b.type === "tool_use" && b.name === "Skill") {
          const inp = b.input || {};
          if (inp.skill === SKILL_NAME || JSON.stringify(inp).includes(SKILL_NAME)) return true;
        }
      }
    }
    return false;
  } catch (e) {
    return null; // unreadable despite existing -> fail closed
  }
}

function main() {
  let ev;
  try {
    ev = JSON.parse(readStdin());
  } catch (e) {
    process.exit(0); // no/garbled input -> allow, fail-open on parse
  }
  if (ev === null || typeof ev !== "object" || Array.isArray(ev)) ev = {};
  // DORMANCY GUARD: no-op unless a genesis agent is active (payload agent_type).
  if (!agent_ident.resolve_agent(ev)) process.exit(0);

  const command = (ev.tool_input && ev.tool_input.command) || "";
  const name = assembledAgentName(command);
  if (name === null) allow(); // not an assembler invocation
  if (BUILTINS.has(name)) allow(name, "builtin-exempt");
  if (!name) {
    deny(
      "(unknown)",
      ASSEMBLER + " invocation with no parseable agent name — cannot verify the " +
        "research-expertise skill ran; refusing to build (fail-closed)."
    );
  }

  const used = researchSkillUsed(ev.transcript_path || "");
  if (used === true) allow(name, "research-expertise skill confirmed");
  // used is false or null -> fail closed
  deny(
    name,
    "You must run the `research-expertise` skill to select and research '" + name + "'s expertise " +
      "(with the user) BEFORE assembling it. Invoke the research-expertise skill, complete the " +
      "process, then assemble again."
  );
}

if (require.main === module) {
  main();
}

module.exports = { main, assembledAgentName, researchSkillUsed, shlexSplit };
