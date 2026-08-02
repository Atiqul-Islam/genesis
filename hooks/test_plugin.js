#!/usr/bin/env node
/* Node tests for the PLUGIN scaffold + the payload agent-derivation (the plugin re-architecture, items A + B).

   Faithful port of test_plugin.py. Proves the shipped plugin artifacts obey the verified plugin constraints
   (no self-verification of a live install — that is a manual gate; here we prove the STATIC artifacts are
   correct), exercises agent_ident.js's payload derivation, and runs build_plugin_agents.js for the drift
   check. Mirrors the Python contract + case count (45 cases).

   Run:  node hooks/test_plugin.js
*/
"use strict";
const fs = require("fs");
const path = require("path");
const cp = require("child_process");

const HERE = __dirname;
const REPO = path.dirname(HERE); // hooks/ -> plugin root
const agent_ident = require(path.join(HERE, "agent_ident.js"));

const SCOPED = ["mcp__plugin_genesis_genesis-memory__store",
                "mcp__plugin_genesis_genesis-memory__recall",
                "mcp__plugin_genesis_genesis-memory__consolidate"];
const BANNED_KEYS = ["hooks", "mcpServers", "permissionMode"];

function readText(p) { return fs.readFileSync(p, "utf8"); }
function isFile(p) { try { return fs.statSync(p).isFile(); } catch (e) { return false; } }

function pySplit(s, sep, maxsplit) {
  // Python str.split(sep, maxsplit): at most maxsplit splits; the remainder is kept as the last element.
  const out = [];
  let idx = 0, count = 0;
  while (count < maxsplit) {
    const j = s.indexOf(sep, idx);
    if (j === -1) break;
    out.push(s.slice(idx, j));
    idx = j + sep.length;
    count++;
  }
  out.push(s.slice(idx));
  return out;
}

function frontmatter_and_body(p) {
  const txt = readText(p);
  if (txt.slice(0, 4) !== "---\n") throw new Error(p);
  const parts = pySplit(txt, "---\n", 2);
  return [parts[1], parts[2]];
}

function fm_keys(fm) {
  // Top-level YAML keys in a frontmatter block (lines like `key: ...`).
  const keys = [];
  for (const line of fm.split("\n")) {
    if (line && !/\s/.test(line[0]) && line.indexOf(":") !== -1) {
      keys.push(pySplit(line, ":", 1)[0].trim());
    }
  }
  return keys;
}

function fm_value(fm, key) {
  for (const line of fm.split("\n")) {
    if (line.slice(0, key.length + 1) === key + ":") {
      return pySplit(line, ":", 1)[1].trim();
    }
  }
  return "";
}

function tool_set(fm) {
  const out = new Set();
  for (const t of fm_value(fm, "tools").split(",")) {
    if (t.trim()) out.add(t.trim());
  }
  return out;
}

function arrEq(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

function main() {
  let passed = 0, failed = 0;
  function check(name, cond) {
    if (cond) { passed += 1; } else { failed += 1; }
    console.log("  " + (cond ? "PASS" : "FAIL") + "  " + name);
  }

  // ---- plugin.json ----
  const pj = JSON.parse(readText(path.join(REPO, ".claude-plugin", "plugin.json")));
  check("plugin.json parses and name == genesis", pj.name === "genesis");

  // ---- agents/*.md ----
  for (const [name, wants_agent] of [["sensei", true], ["method", false]]) {
    const [fm, body] = frontmatter_and_body(path.join(REPO, "agents", name + ".md"));
    const keys = new Set(fm_keys(fm));
    check(name + ".md frontmatter has NO hooks/mcpServers/permissionMode",
          !BANNED_KEYS.some((k) => keys.has(k)));
    const tools = tool_set(fm);
    check(name + ".md uses the 3 plugin-scoped memory tools", SCOPED.every((s) => tools.has(s)));
    check(name + ".md declares name/description/tools", ["name", "description", "tools"].every((k) => keys.has(k)));
    check(name + ".md " + (wants_agent ? "has" : "has NO") + " Agent tool",
          tools.has("Agent") === wants_agent);
    const cap = name.charAt(0).toUpperCase() + name.slice(1);
    check(name + ".md body carries the persona (mentions " + cap + ")", body.indexOf(cap) !== -1);
  }
  // method must NOT be able to spawn/delegate; sensei must.
  const [sfm] = frontmatter_and_body(path.join(REPO, "agents", "sensei.md"));
  check("sensei preloads build-agent + research-expertise skills",
        fm_value(sfm, "skills").indexOf("build-agent") !== -1 && fm_value(sfm, "skills").indexOf("research-expertise") !== -1);

  // ---- hooks/hooks.json ----
  const hj = JSON.parse(readText(path.join(REPO, "hooks", "hooks.json")));
  const H = hj.hooks || {};
  check("hooks.json parses with a 'hooks' object", H && typeof H === "object" && !Array.isArray(H) && Object.keys(H).length > 0);
  for (const ev of ["SubagentStart", "PreToolUse", "SubagentStop"]) {
    check("hooks.json wires " + ev, ev in H);
  }
  check("hooks.json does NOT wire main-thread SessionStart (dormant)", !("SessionStart" in H));
  check("hooks.json does NOT wire main-thread Stop (dormant)", !("Stop" in H));
  // collect every command string
  const cmds = [];
  for (const k of Object.keys(H)) {
    for (const blk of H[k]) {
      for (const h of (blk.hooks || [])) cmds.push(h.command);
    }
  }
  check("every hook command uses ${CLAUDE_PLUGIN_ROOT}", cmds.every((c) => c.indexOf("${CLAUDE_PLUGIN_ROOT}") !== -1));
  check("NO --main-agent anywhere (no forced main-thread agent)", !cmds.some((c) => c.indexOf("--main-agent") !== -1));
  for (const script of ["inject.js", "gate.js", "enforce_research.js", "validate.js", "review.js"]) {
    check("hooks.json references " + script, cmds.some((c) => c.indexOf(script) !== -1));
  }
  const pre_matchers = new Set((H.PreToolUse || []).map((blk) => blk.matcher));
  check("PreToolUse matches Write|Edit and Bash", ["Write|Edit", "Bash"].every((m) => pre_matchers.has(m)));
  const start_matchers = (H.SubagentStart || []).map((blk) => blk.matcher || "").join(" ");
  check("SubagentStart injects for BOTH sensei and method",
        start_matchers.indexOf("sensei") !== -1 && start_matchers.indexOf("method") !== -1);
  const sub_matchers = (H.SubagentStop || []).map((blk) => blk.matcher || "").join(" ");
  check("SubagentStop enforces on BOTH sensei and method",
        sub_matchers.indexOf("sensei") !== -1 && sub_matchers.indexOf("method") !== -1);
  const subCmds = [];
  for (const blk of H.SubagentStop) for (const h of blk.hooks) subCmds.push(h.command);
  check("SubagentStop runs validate THEN review",
        subCmds.some((c) => c.indexOf("validate.js") !== -1) && subCmds.some((c) => c.indexOf("review.js") !== -1));

  // ---- .mcp.json ----
  const mj = JSON.parse(readText(path.join(REPO, ".mcp.json")));
  const gm = mj.mcpServers["genesis-memory"];
  check(".mcp.json launcher: npx @xcidos/genesis-memory-server",
        gm.command === "npx" && gm.args.some((arg) => arg.indexOf("@xcidos/genesis-memory-server") !== -1));

  // ---- dormancy: NO settings.json agent auto-activation ----
  check("settings.json is absent (no global agent:sensei auto-activation)",
        !fs.existsSync(path.join(REPO, "settings.json")));

  // ---- /genesis:new entry command ----
  const cmd = path.join(REPO, "commands", "new.md");
  check("commands/new.md exists (the on-demand /genesis:new entry point)", isFile(cmd));
  check("old commands/genesis.md is gone (renamed to new.md)", !isFile(path.join(REPO, "commands", "genesis.md")));
  if (isFile(cmd)) {
    const ctext = readText(cmd);
    check("commands/new.md invokes the sensei agent", ctext.toLowerCase().indexOf("sensei") !== -1);
    check("commands/new.md passes $ARGUMENTS to the build", ctext.indexOf("$ARGUMENTS") !== -1);
  }

  // ---- agent_ident: payload derivation ----
  check("normalize strips the plugin scope (genesis:method -> method)",
        agent_ident.normalize("genesis:method") === "method" && agent_ident.normalize("^genesis:method$") === "method");
  check("resolve from SubagentStop payload agent_type",
        agent_ident.resolve_agent({ agent_type: "genesis:method" }) === "method");
  check("resolve bare agent_type",
        agent_ident.resolve_agent({ agent_type: "sensei" }) === "sensei");
  check("argv positional wins over payload",
        agent_ident.resolve_agent({ agent_type: "method" }, "acme-bot") === "acme-bot");
  check("main-thread fallback when payload omits agent_type",
        agent_ident.resolve_agent({}, "", "sensei") === "sensei");
  check("no identity anywhere -> ''", agent_ident.resolve_agent({}) === "");
  check("split_args parses . --main-agent sensei",
        arrEq(agent_ident.split_args([".", "--main-agent", "sensei"]), [".", "", "sensei"]));
  check("split_args keeps the historical <root> <agent> form",
        arrEq(agent_ident.split_args([".", "method"]), [".", "method", ""]));

  // ---- drift: committed agents/*.md == what build_plugin_agents.js regenerates from team/ sources ----
  const before = {}; for (const n of ["sensei", "method"]) before[n] = readText(path.join(REPO, "agents", n + ".md"));
  const r = cp.spawnSync(process.execPath, [path.join(REPO, "install", "build_plugin_agents.js"), REPO],
                         { encoding: "utf8" });
  const after = {}; for (const n of ["sensei", "method"]) after[n] = readText(path.join(REPO, "agents", n + ".md"));
  check("build_plugin_agents.js regenerates cleanly", r.status === 0);
  check("committed plugin agents match the team sources (no drift)",
        before.sensei === after.sensei && before.method === after.method);

  console.log("\n" + passed + " passed, " + failed + " failed");
  process.exit(failed ? 1 : 0);
}

if (require.main === module) {
  main();
}
