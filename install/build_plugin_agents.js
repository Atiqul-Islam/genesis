#!/usr/bin/env node
/* Generate the PLUGIN-shipped agents `agents/sensei.md` and `agents/method.md` from the team sources.

   Faithful Node (CommonJS, stdlib-only) port of build_plugin_agents.py.

   These are a DIFFERENT artifact from what assemble.js writes. assemble.js builds USER-repo agents
   (`<repo>/.claude/agents/<name>.md`) that MAY carry frontmatter `hooks` and point at a repo-local
   `genesis-memory` MCP server. A PLUGIN-shipped agent CANNOT declare `hooks`, `mcpServers`, or
   `permissionMode` in frontmatter (security-blocked — plugins-reference, verified 2026-07-22); its
   enforcement + memory come from the PLUGIN level instead (hooks/hooks.json + the plugin-root .mcp.json).
   So these files carry ONLY name/description/tools/skills, and their memory tools use the plugin-scoped
   names `mcp__plugin_<plugin>_genesis-memory__{store,recall,consolidate}`.

   Body composition is identical to assemble.js (persona + behavior + the expertise/memory notes) and reuses
   assemble.js's own constants, so the plugin agents never drift from the team sources.

   usage: build_plugin_agents.js [<repo_root>]     # default: this file's repo root
   writes: <repo_root>/agents/{sensei,method}.md
*/
"use strict";
const fs = require("fs");
const path = require("path");
const assemble = require("./assemble.js"); // reuse AGENTS metadata + the EXPERTISE/MEMORY notes + read() (single source)

const HERE = __dirname;

// Which plugin skills each built-in preloads (dirs live at <repo_root>/skills/). Method ships no skills.
const BUILTIN_SKILLS = { sensei: ["build-agent", "research-expertise"], method: [] };

function pluginName(repoRoot) {
  // The plugin name from .claude-plugin/plugin.json — used to build the scoped MCP tool prefix.
  const p = path.join(repoRoot, ".claude-plugin", "plugin.json");
  return JSON.parse(fs.readFileSync(p, { encoding: "utf-8" })).name;
}

function memoryTools(plugin) {
  // Plugin .mcp.json server tools are exposed to agents as `mcp__plugin_<plugin>_<server>__<tool>`.
  const prefix = `mcp__plugin_${plugin}_genesis-memory__`;
  return ["store", "recall", "consolidate"].map((t) => prefix + t);
}

function frontmatter(name, meta, skills, memTools) {
  const tools = Array.from(meta.tools).concat(memTools);
  const skillsLine = skills && skills.length ? "skills: " + skills.join(", ") + "\n" : "";
  return (
    "---\n" +
    "name: " + name + "\n" +
    "description: " + JSON.stringify(meta.description) + "\n" +
    "tools: " + tools.join(", ") + "\n" +
    skillsLine +
    "---\n"
  );
}

function isDir(p) {
  try {
    return fs.statSync(p).isDirectory();
  } catch (e) {
    return false;
  }
}

function buildOne(repoRoot, name, memTools) {
  const meta = assemble.AGENTS[name];
  const src = path.join(repoRoot, "team", name);
  const body = [
    assemble.read(path.join(src, "persona.md")),
    assemble.read(path.join(src, "behavior.md")),
    assemble.EXPERTISE_NOTE,
    assemble.MEMORY_NOTE,
  ].join("\n\n");
  const skills = BUILTIN_SKILLS[name].filter((s) => isDir(path.join(repoRoot, "skills", s)));
  const outDir = path.join(repoRoot, "agents");
  fs.mkdirSync(outDir, { recursive: true });
  const out = path.join(outDir, `${name}.md`);
  fs.writeFileSync(out, frontmatter(name, meta, skills, memTools) + "\n" + body + "\n", { encoding: "utf-8" });
  return { agent: name, written: out, tools: Array.from(meta.tools).concat(memTools), skills };
}

function main() {
  const argv = process.argv.slice(2);
  const repoRoot = argv.length > 0 ? path.resolve(argv[0]) : path.dirname(HERE);
  const memTools = memoryTools(pluginName(repoRoot));
  const out = ["sensei", "method"].map((name) => buildOne(repoRoot, name, memTools));
  console.log(JSON.stringify({ plugin_root: repoRoot, agents: out }, null, 2));
}

if (require.main === module) {
  main();
}

module.exports = { BUILTIN_SKILLS, pluginName, memoryTools, frontmatter, buildOne, main };
