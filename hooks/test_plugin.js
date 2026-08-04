#!/usr/bin/env node
/* Node tests for the PLUGIN scaffold + the payload agent-derivation (the plugin re-architecture, items A + B).

   Proves the shipped plugin artifacts obey the verified plugin constraints (no self-verification of a live
   install — that is a manual gate; here we prove the STATIC artifacts are correct: agents/*.md frontmatter,
   hooks.json wiring, .mcp.json, commands). Agent-identity derivation is the Rust genesis-hook binary
   (hook/src/agent.rs); the committed-agents drift check is a Rust integration test (cli/tests/).

   Run:  node hooks/test_plugin.js
*/
"use strict";
const fs = require("fs");
const path = require("path");

const HERE = __dirname;
const REPO = path.dirname(HERE); // hooks/ -> plugin root
// (agent identity derivation is now the Rust genesis-hook binary — unit-tested in hook/src/agent.rs.)

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
  // collect every command string (agent-type hooks carry a `prompt`, not a `command`)
  const cmds = [];
  const agentHooks = [];
  for (const k of Object.keys(H)) {
    for (const blk of H[k]) {
      for (const h of (blk.hooks || [])) {
        if (h.command) cmds.push(h.command);
        if (h.type === "agent") agentHooks.push(h);
      }
    }
  }
  check("every command hook uses ${CLAUDE_PLUGIN_ROOT}", cmds.every((c) => c.indexOf("${CLAUDE_PLUGIN_ROOT}") !== -1));
  check("NO --main-agent anywhere (no forced main-thread agent)", !cmds.some((c) => c.indexOf("--main-agent") !== -1));
  check("deterministic hooks invoke genesis-hook via the launcher's --run-hook shim",
        cmds.length > 0 && cmds.every((c) => c.indexOf("--run-hook") !== -1 && c.indexOf("bin/genesis-memory.js") !== -1));
  check("no legacy run.js resolver referenced (absorbed into the launcher)",
        !cmds.some((c) => c.indexOf("run.js") !== -1));
  for (const sub of ["inject", "gate", "enforce-research", "validate"]) {
    check("hooks.json wires the '" + sub + "' subcommand", cmds.some((c) => c.indexOf(" " + sub) !== -1));
  }
  check("NO legacy Node hook .js referenced (all-Rust deterministic hooks)",
        !cmds.some((c) => /\/(inject|gate|enforce_research|validate|review|agent_ident|session_pointer|adherence)\.js/.test(c)));
  const pre_matchers = new Set((H.PreToolUse || []).map((blk) => blk.matcher));
  check("PreToolUse matches Write|Edit and Bash", ["Write|Edit", "Bash"].every((m) => pre_matchers.has(m)));
  const start_matchers = (H.SubagentStart || []).map((blk) => blk.matcher || "").join(" ");
  check("SubagentStart injects for BOTH sensei and method",
        start_matchers.indexOf("sensei") !== -1 && start_matchers.indexOf("method") !== -1);
  const sub_matchers = (H.SubagentStop || []).map((blk) => blk.matcher || "").join(" ");
  check("SubagentStop enforces on BOTH sensei and method",
        sub_matchers.indexOf("sensei") !== -1 && sub_matchers.indexOf("method") !== -1);
  const subHooks = [];
  for (const blk of H.SubagentStop) for (const h of blk.hooks) subHooks.push(h);
  check("SubagentStop runs validate (command) THEN review (built-in agent hook)",
        subHooks.some((h) => h.command && h.command.indexOf("validate") !== -1) &&
        subHooks.some((h) => h.type === "agent"));
  check("review agent hook uses a fast Haiku model + injects $ARGUMENTS",
        agentHooks.some((h) => (h.model || "").indexOf("haiku") !== -1 && (h.prompt || "").indexOf("$ARGUMENTS") !== -1));

  // ---- .mcp.json ----
  const mj = JSON.parse(readText(path.join(REPO, ".mcp.json")));
  const gm = mj.mcpServers["genesis-memory"];
  check(".mcp.json launcher: node bin/genesis-memory.js (GitHub-Releases launcher, no npx)",
        gm.command === "node" && gm.args.some((arg) => arg.indexOf("bin/genesis-memory.js") !== -1));

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

  // (agent identity derivation — normalize / resolve_agent / split_args — is now the Rust genesis-hook
  //  binary, covered by hook/src/agent.rs unit tests; the Node port was removed with the other Node hooks.)
  // (drift: committed agents/*.md == genesis-cli build-plugin-agents output — now a Rust integration test
  //  in cli/tests/, since the generator is the native genesis-cli binary.)

  console.log("\n" + passed + " passed, " + failed + " failed");
  process.exit(failed ? 1 : 0);
}

if (require.main === module) {
  main();
}
