#!/usr/bin/env node
/* Shared helper: resolve the ACTIVE AGENT + the PROJECT runtime dir for a hook.

   Faithful Node (CommonJS, stdlib-only) port of agent_ident.py. See agent_ident.py for the full
   rationale (plugin re-architecture; agent identity derives from the hook PAYLOAD's `agent_type`).

   RESOLUTION PRECEDENCE (keeps the built-agent frontmatter path + all existing tests intact):
     1. an explicit argv positional agent (assemble.js's built-agent frontmatter hooks pass it);
     2. the payload's `agent_type`, scope-normalized (the plugin session-level path);
     3. a `--main-agent <name>` fallback the plugin hooks.json supplies for the main thread.
*/
"use strict";
const path = require("path");

function stripCharset(s, chars) {
  // Emulate Python str.strip(chars): remove leading & trailing chars that are in `chars`.
  let start = 0, end = s.length;
  while (start < end && chars.indexOf(s[start]) !== -1) start++;
  while (end > start && chars.indexOf(s[end - 1]) !== -1) end--;
  return s.slice(start, end);
}

function normalize(agent_type) {
  // `genesis:method` / `^genesis:method$` / `method` -> `method`. '' -> ''.
  if (!agent_type) return "";
  let a = String(agent_type).trim();
  a = stripCharset(a, "^$").trim();
  if (a.indexOf(":") !== -1) {            // plugin-scoped "<plugin>:<name>" -> bare "<name>"
    a = a.slice(a.lastIndexOf(":") + 1);
  }
  return a.trim();
}

function agent_from_payload(ev) {
  // The active agent name from a hook payload, or '' if none. Reads `agent_type`
  // (docs: the agent/subagent name/type; camelCase variant tolerated defensively).
  if (ev === null || typeof ev !== "object" || Array.isArray(ev)) return "";
  return normalize(ev.agent_type || ev.agentType || "");
}

function resolve_agent(ev, argv_agent, default_agent) {
  // Active agent, precedence: explicit argv > payload agent_type > --main-agent fallback.
  argv_agent = argv_agent || "";
  default_agent = default_agent || "";
  if (argv_agent) return argv_agent;
  const a = agent_from_payload(ev);
  if (a) return a;
  return default_agent || "";
}

function split_args(argv) {
  // Parse a Stop/SubagentStop hook's argv into [root, agent, main_agent].
  // Tolerates the historical positional form `<root> <agent>` MIXED with a `--main-agent <name>`
  // flag the plugin hooks.json adds. Returns root=null when no positional root was given.
  let root = null, agent = "", main_agent = "";
  const pos = [];
  let i = 0;
  while (i < argv.length) {
    const t = argv[i];
    if (t === "--main-agent" && i + 1 < argv.length) {
      main_agent = argv[i + 1];
      i += 2;
      continue;
    }
    pos.push(t);
    i += 1;
  }
  if (pos.length) root = pos[0];
  if (pos.length > 1) agent = pos[1];
  return [root, agent, main_agent];
}

function runtime_dir(cwd) {
  // The PROJECT's writable `.genesis/` runtime dir, derived from the CWD. NEVER the plugin dir and
  // NEVER hook-dir-relative. Guards against a nested `.genesis/.genesis` if cwd already IS a workspace.
  cwd = cwd || process.cwd();
  const norm = path.normalize(cwd).replace(/[\\/]+$/, "");
  if (path.basename(norm) === ".genesis") return cwd;
  return path.join(cwd, ".genesis");
}

module.exports = {
  normalize,
  agent_from_payload,
  resolve_agent,
  split_args,
  runtime_dir,
};
