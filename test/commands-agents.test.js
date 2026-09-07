#!/usr/bin/env node
/* Regression guard for #28: plugin COMMANDS must not instruct invoking an agent type that resolves nowhere.

   The concrete bug: commands/retro-learn.md said "Invoke the `mneme` agent", but the plugin registers that
   agent as `genesis:mneme` and bootstrap installs only sensei+method as repo-local agents — so bare `mneme`
   is unresolvable ("Agent type 'mneme' not found"). sensei/method are fine bare (bootstrap installs them);
   only mneme is plugin-only, so it must always be namespaced `genesis:mneme` in a command.

   This test scans commands/*.md and asserts no file carries the bare agent token `mneme` (backtick-quoted),
   and that retro-learn.md invokes `genesis:mneme`. Run: node test/commands-agents.test.js
*/
"use strict";
const fs = require("fs");
const path = require("path");

const REPO = path.dirname(__dirname);
const CMD_DIR = path.join(REPO, "commands");

let passed = 0;
let failed = 0;
function check(name, cond) {
  if (cond) passed += 1;
  else failed += 1;
  console.log(`  ${cond ? "PASS" : "FAIL"}  ${name}`);
}

const files = fs.existsSync(CMD_DIR)
  ? fs.readdirSync(CMD_DIR).filter((f) => f.endsWith(".md"))
  : [];
check("commands/ directory has command files", files.length > 0);

// The bare agent token `mneme` (backtick-delimited) must never appear in a command — it resolves nowhere.
// `genesis:mneme` is the backtick token `genesis:mneme`, which does NOT contain the substring "`mneme`".
const BARE = "`mneme`";
for (const f of files) {
  const text = fs.readFileSync(path.join(CMD_DIR, f), "utf8");
  check(`commands/${f}: no bare \`mneme\` agent token (use \`genesis:mneme\`)`, !text.includes(BARE));
}

// retro-learn specifically must invoke the namespaced agent.
const retro = path.join(CMD_DIR, "retro-learn.md");
if (fs.existsSync(retro)) {
  const t = fs.readFileSync(retro, "utf8");
  check("commands/retro-learn.md invokes `genesis:mneme`", t.includes("`genesis:mneme`"));
}

console.log(`\n${passed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
