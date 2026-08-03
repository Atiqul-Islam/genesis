#!/usr/bin/env node
/* Assemble a member's clean source files into the `.claude/agents/<name>.md` the runtime needs.

   Faithful Node (CommonJS, stdlib-only) port of assemble.py. Applies the deterministic layer (§16):
     * BOUNDARIES enforced at the TOOL layer (frontmatter `tools`) — e.g. Method has no `Agent` tool, so it
       physically cannot spawn/delegate. A real gate, not a prompt wish.
     * EXPERTISE + house rules delivered and enforced by the inject/gate/validate HOOK triple, wired here into
       each agent's frontmatter (verified YAML shape: event -> matcher -> hooks:[{type,command}]).

   Deterministic hooks (inject/gate/enforce-research/validate) invoke the native `genesis-hook` binary
   DIRECTLY at ${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook[.exe] (staged per-machine by bootstrap) —
   NO Node in the hot path. The semantic `review` is a Claude Code built-in `agent` hook (fast Haiku model,
   tool-capable). Paths reference the repo's own `.genesis` via the braced `${CLAUDE_PROJECT_DIR}` (Claude
   Code substitutes the project root at runtime, Windows + macOS + Linux) — never an absolute machine path.

   usage: assemble.js <source_member_dir> <name> <target_repo> <genesis_home>
   reads:  <source_member_dir>/{persona.md, behavior.md, skills/}
   writes: <target_repo>/.claude/agents/<name>.md
*/
"use strict";
const fs = require("fs");
const path = require("path");

const AGENTS = {
  sensei: {
    description:
      "Genesis coordinator - the user talks to Sensei; it verifies requirements, plans, " +
      "delegates authoring to Method, then assembles, wires, installs, and delivers.",
    tools: ["Read", "Write", "Edit", "Bash", "Glob", "Grep", "Agent", "SendMessage"],
    expertise: ["agent-building", "agentic-teams", "expertise-application"],
    // Sensei's skills now live at the genesis-home `skills/` dir (plugin-root skills/), not under team/.
    skills: ["build-agent", "research-expertise"],
  },
  method: {
    // No `Agent` tool -> cannot spawn/delegate. Boundary enforced by config, not prompt.
    description:
      "Genesis craftsman - authors and tests each agent's persona, behavior, and skills. " +
      "Writes tests first; ships nothing untested; never orchestrates.",
    tools: ["Read", "Write", "Edit", "Bash", "Glob", "Grep", "SendMessage"],
    expertise: ["persona-creation", "prompt-engineering", "expertise-application"],
    skills: [],
  },
};

const EXPERTISE_NOTE =
  "## Your expertise\n" +
  "- A SessionStart hook injects the house rules and pointers to your decoupled, versioned expertise store.\n" +
  "- Read the expertise file your behavior names, on demand, before deep work. It is authoritative.\n" +
  "- The hard, checkable rules are also enforced by gate/validate hooks — you cannot violate them.";

// Per-agent semantic memory via the Genesis MCP memory server (registered as `genesis-memory`).
const MEMORY_TOOLS = ["mcp__genesis-memory__store", "mcp__genesis-memory__recall", "mcp__genesis-memory__consolidate"];
const MEMORY_NOTE =
  "## Your memory (per-agent, durable across sessions)\n" +
  "- The `genesis-memory` MCP server gives you your own semantic memory: `store`, `recall`, `consolidate`.\n" +
  "- ALWAYS pass your own agent name as `agent_id` — the store is scoped by it, so you only see your own memories.\n" +
  "- `store` a durable fact/decision; `recall` before deep work to retrieve what you learned before; " +
  "`consolidate` to dedup. This is separate from the transient session context.";

function read(p) {
  // Match Python str.rstrip(): strip trailing whitespace (incl. newlines).
  return fs.readFileSync(p, { encoding: "utf-8" }).replace(/\s+$/, "");
}

function isDir(p) {
  try {
    return fs.statSync(p).isDirectory();
  } catch (e) {
    return false;
  }
}

function isFile(p) {
  try {
    return fs.statSync(p).isFile();
  } catch (e) {
    return false;
  }
}

function fail(msg) {
  process.stderr.write(msg + "\n");
  process.exit(1);
}

function jsonDumpAscii(obj, indent) {
  // Match Python json.dump(indent=2) default ensure_ascii=True: escape every non-ASCII code unit (>= 0x80)
  // as \\uXXXX (surrogate halves included), so required.json stays byte-identical to the Python original.
  const s = JSON.stringify(obj, null, indent);
  let out = "";
  for (let i = 0; i < s.length; i++) {
    const code = s.charCodeAt(i);
    out += code >= 0x80 ? "\\u" + code.toString(16).padStart(4, "0") : s[i];
  }
  return out;
}

function registerRequired(gh, name, expertise) {
  // Upsert this agent's REQUIRED expertise into expertise/required.json so the validate (Stop) hook
  // enforces the APPLIED-EXPERTISE declaration for it — identical machinery to sensei/method. Preserves
  // every other agent's entry and the _doc note.
  const p = path.join(gh, "expertise", "required.json");
  let data;
  try {
    data = JSON.parse(fs.readFileSync(p, { encoding: "utf-8" }));
  } catch (e) {
    data = {
      _doc:
        "Per-agent REQUIRED expertise (auto-registered by assemble.js); the validate Stop " +
        "hook blocks finishing until each is declared via APPLIED-EXPERTISE.",
    };
  }
  data[name] = Array.from(expertise);
  fs.writeFileSync(p, jsonDumpAscii(data, 2) + "\n", { encoding: "utf-8" });
}

function copySkill(skillDir, destRoot) {
  // Copy one skill dir (must contain SKILL.md) into destRoot; return its name or null.
  if (!(isDir(skillDir) && fs.existsSync(path.join(skillDir, "SKILL.md")))) {
    return null;
  }
  const dest = path.join(destRoot, path.basename(skillDir));
  if (fs.existsSync(dest)) {
    fs.rmSync(dest, { recursive: true, force: true });
  }
  fs.cpSync(skillDir, dest, { recursive: true });
  return path.basename(skillDir);
}

function installSkills(src, target) {
  // BUILT agents: copy each skill dir under <src>/skills/ into the repo's .claude/skills/ so Claude Code
  // discovers them, and return their names for the agent's frontmatter `skills:` field (preloads them).
  const srcSkills = path.join(src, "skills");
  if (!isDir(srcSkills)) {
    return [];
  }
  const destRoot = path.join(target, ".claude", "skills");
  fs.mkdirSync(destRoot, { recursive: true });
  const names = [];
  for (const entry of fs.readdirSync(srcSkills).sort()) {
    const n = copySkill(path.join(srcSkills, entry), destRoot);
    if (n) {
      names.push(n);
    }
  }
  return names;
}

function installNamedSkills(gh, names, target) {
  // BUILT-IN agents (sensei/method): their skills live at the genesis-home `skills/` dir (the plugin-root
  // skills/), not under team/<name>/skills/. Copy the NAMED skills into the repo's .claude/skills/ and return
  // the ones actually found (so a missing skill dir never fabricates a frontmatter reference).
  if (!names || !names.length) {
    return [];
  }
  const srcRoot = path.join(gh, "skills");
  const destRoot = path.join(target, ".claude", "skills");
  fs.mkdirSync(destRoot, { recursive: true });
  const out = [];
  for (const name of names) {
    const n = copySkill(path.join(srcRoot, name), destRoot);
    if (n) {
      out.push(n);
    }
  }
  return out;
}

function yamlCmd(...parts) {
  // Build a cross-platform hook `command:` YAML line from already-native path parts.
  //
  // Portability (Windows + POSIX): each path is double-quoted so paths with spaces survive, and NATIVE
  // separators are kept. The whole value is a YAML SINGLE-quoted scalar, which keeps backslashes and inner
  // double-quotes literal (only `'` doubles). So a Windows path like
  // C:\\Users\\Jane Doe\\genesis\\hooks\\gate.js round-trips intact and the shell still sees it quoted.
  const shell = parts.join(" ");
  return "          command: '" + shell.replace(/'/g, "''") + "'\n";
}

function q(p) {
  // Double-quote a path arg for the generated shell command (handles spaces; native separators kept).
  return '"' + p + '"';
}

function reviewPromptText(name, expertise) {
  // The independent semantic reviewer's instructions, baked into the built-in `agent` hook. Mirrors the
  // old review.js job: judge whether the produced artifacts EMBODY each expertise's judgment /
  // non-mechanical checkable rules; the mechanical predicates are enforced deterministically by validate.
  return (
    "You are an INDEPENDENT semantic reviewer for the Genesis agent '" + name + "', which just finished. " +
    "You did NOT write its work; judge it skeptically. Read the artifacts it produced under the current " +
    "project (files matching *persona.md, *behavior.md, CLAUDE.md, .claude/agents/*.md, *persona.spec.json, " +
    "*.tests.json); if there are none, respond {\"ok\":true}. Its REQUIRED expertise: " + expertise.join(", ") +
    ". The expertise store is at .genesis/expertise under the project root. For each required expertise " +
    "<name>, read .genesis/expertise/manifests/<name>.json and check every rule of type \"judgment\" (and " +
    "non-mechanical \"checkable\" rules) against the artifacts — SKIP mechanical predicate kinds " +
    "(regex/line_count/declaration), which are enforced deterministically elsewhere. Be skeptical: if an " +
    "artifact does not clearly satisfy a rule, treat it as FAIL. Respond with ONLY a JSON object: " +
    "{\"ok\":true} if every checked rule is satisfied, or {\"ok\":false,\"reason\":\"<name>#<rule-id>: what " +
    "is missing (one per line)\"}. $ARGUMENTS"
  );
}

function frontmatter(name, meta, home, skills, expertise) {
  // Deterministic hooks run the native `genesis-hook` binary DIRECTLY (no Node in the hot path) at
  // ${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook[.exe] — staged per-machine by bootstrap. The .exe suffix
  // is chosen for THIS machine; a clone to another OS re-runs bootstrap (which re-stages the binary + re-
  // assembles). `home` is e.g. "${CLAUDE_PROJECT_DIR}/.genesis" — a portable base, never an absolute path.
  const expDir = home + "/expertise";
  const bin = q(home + "/bin/genesis-hook" + (process.platform === "win32" ? ".exe" : ""));
  const inject = bin + " inject " + q(expDir) + " " + name;
  const gate = bin + " gate --expertise " + q(expDir);
  const stop = bin + " validate . " + name + " --expertise " + q(expDir);
  const skillsLine = skills && skills.length ? "skills: " + skills.join(", ") + "\n" : "";
  // PreToolUse: every agent gets the Write|Edit gate (house rules + rule surfacing). SENSEI additionally
  // gets a Bash gate that blocks assembling a built agent unless the research-expertise skill ran this
  // session — only Sensei orchestrates/assembles, so it is wired Sensei-scoped and no other agent changes.
  let pretooluse =
    "  PreToolUse:\n" +
    '    - matcher: "Write|Edit"\n' +
    "      hooks:\n" +
    "        - type: command\n" +
    yamlCmd(gate);
  if (name === "sensei") {
    const enforce = bin + " enforce-research";
    pretooluse +=
      '    - matcher: "Bash"\n' +
      "      hooks:\n" +
      "        - type: command\n" +
      yamlCmd(enforce);
  }
  // Stop: deterministic validate (command -> binary) + semantic review (built-in `agent` hook, fast Haiku
  // model). The review hook is emitted only when the agent has required expertise (nothing to review else).
  let stopHooks =
    "  Stop:\n" +
    "    - hooks:\n" +
    "        - type: command\n" +
    yamlCmd(stop);
  if (expertise && expertise.length) {
    stopHooks +=
      "        - type: agent\n" +
      "          model: 'claude-haiku-4-5-20251001'\n" +
      "          timeout: 120\n" +
      "          prompt: '" + reviewPromptText(name, expertise).replace(/'/g, "''") + "'\n";
  }
  // Verified frontmatter-hooks YAML shape (docs db-reader example): event -> [{matcher, hooks:[{type,command}]}].
  return (
    "---\n" +
    "name: " + name + "\n" +
    "description: " + JSON.stringify(meta.description) + "\n" +
    "tools: " + meta.tools.join(", ") + "\n" +
    skillsLine +
    "hooks:\n" +
    "  SessionStart:\n" +
    '    - matcher: "startup|resume|compact"\n' +
    "      hooks:\n" +
    "        - type: command\n" +
    yamlCmd(inject) +
    pretooluse +
    stopHooks +
    "---\n"
  );
}

function main() {
  const argv = process.argv.slice(2);
  const src = argv[0];
  const name = argv[1];
  const target = argv[2];
  const gh = argv[3];
  // Genesis's own team (sensei/method) has built-in metadata; a BUILT agent supplies its own via meta.json
  // ({"description":..., "tools":[...]}) written alongside its persona.md/behavior.md.
  let meta = AGENTS[name];
  if (!meta) {
    const mp = path.join(src, "meta.json");
    if (!fs.existsSync(mp)) {
      fail(`no metadata for agent '${name}' — provide ${mp} with 'description' and 'tools'.`);
    }
    meta = JSON.parse(fs.readFileSync(mp, { encoding: "utf-8" }));
    if (!("description" in meta) || !("tools" in meta)) {
      fail(`${mp} must contain 'description' and 'tools'.`);
    }
  }
  // Every assembled agent gets per-agent memory tools (design: each agent has its own vector memory).
  meta = Object.assign({}, meta, { tools: Array.from(meta.tools).concat(MEMORY_TOOLS) });
  // Parity with sensei/method: register this agent's required expertise so the Stop hook enforces the
  // declaration. Built agents supply it via meta.json ("required_expertise" or "expertise"); built-ins
  // via the AGENTS table. No expertise assigned -> nothing to enforce (declaration check is a no-op).
  const expertise = meta.expertise || meta.required_expertise || [];
  registerRequired(gh, name, expertise);
  // Copy this agent's skills into <target>/.claude/skills/ and collect their names for the frontmatter.
  // Built-in team members (sensei/method) source their skills from the genesis-home `skills/` dir (the
  // plugin-root skills/); a BUILT agent supplies its own under its source dir's skills/.
  let skills;
  if (name in AGENTS) {
    skills = installNamedSkills(gh, AGENTS[name].skills || [], target);
  } else {
    skills = installSkills(src, target);
  }
  const body = [
    read(path.join(src, "persona.md")),
    read(path.join(src, "behavior.md")),
    EXPERTISE_NOTE,
    MEMORY_NOTE,
  ].join("\n\n");
  const outDir = path.join(target, ".claude", "agents");
  fs.mkdirSync(outDir, { recursive: true });
  const out = path.join(outDir, `${name}.md`);
  // Portable hook base: express genesis_home RELATIVE to the target repo via ${CLAUDE_PROJECT_DIR} so the
  // written agent has NO absolute machine path (works on any machine/OS after the repo's .genesis exists).
  // Normal case: gh = <target>/.genesis -> "${CLAUDE_PROJECT_DIR}/.genesis". If gh is somehow outside the
  // repo (or on a different Windows drive, where relative paths are impossible), fall back to the
  // (non-portable) absolute path rather than emit a broken reference.
  let home;
  try {
    const rel = path.relative(target, gh).split(path.sep).join("/");
    home = !rel.startsWith("..") && !path.isAbsolute(rel) ? "${CLAUDE_PROJECT_DIR}/" + rel : gh.split(path.sep).join("/");
  } catch (e) {
    home = gh.split(path.sep).join("/");
  }
  fs.writeFileSync(out, frontmatter(name, meta, home, skills, expertise) + "\n" + body + "\n", { encoding: "utf-8" });
  console.log(
    JSON.stringify({
      agent: name,
      written: out,
      tools: meta.tools,
      skills_installed: skills,
      body_lines: body.split("\n").length,
    })
  );
}

if (require.main === module) {
  main();
}

module.exports = {
  AGENTS,
  EXPERTISE_NOTE,
  MEMORY_NOTE,
  MEMORY_TOOLS,
  read,
  frontmatter,
  _yaml_cmd: yamlCmd,
  main,
};
