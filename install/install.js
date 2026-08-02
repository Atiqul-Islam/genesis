#!/usr/bin/env node
/* Install Genesis into a target repo — one command.

   Faithful Node (CommonJS, stdlib-only) port of install.py.

   Assembles Genesis's own team (Sensei + Method) into <target_repo>/.claude/agents/, installs their skills
   into <target_repo>/.claude/skills/, and reports the memory-server binary location. After this, opening
   Claude Code in <target_repo> exposes `sensei` and `method`; the user talks to Sensei to build agents.

   usage: install.js <target_repo>            # genesis_home from $GENESIS_HOME, else this file's repo root
          install.js <target_repo> <genesis_home>
*/
"use strict";
const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const TEAM = ["sensei", "method"];

function isDir(p) {
  try {
    return fs.statSync(p).isDirectory();
  } catch (e) {
    return false;
  }
}

function fail(msg) {
  process.stderr.write(msg + "\n");
  process.exit(1);
}

function main() {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    fail("usage: install.js <target_repo> [genesis_home]  (or set $GENESIS_HOME)");
  }
  const target = path.resolve(args[0]);
  // genesis_home: explicit arg > $GENESIS_HOME > inferred from this file's location. Never hardcoded —
  // portable across machines/OSes (the env var is the parametrization the requirement calls for).
  const gh =
    args.length > 1
      ? path.resolve(args[1])
      : process.env.GENESIS_HOME
      ? path.resolve(process.env.GENESIS_HOME)
      : path.dirname(__dirname);

  const assembler = path.join(gh, "install", "assemble.js");
  if (!fs.existsSync(assembler)) {
    fail(`assembler not found at ${assembler} — is ${gh} the Genesis home?`);
  }
  if (!isDir(target)) {
    fail(`target repo not found: ${target}`);
  }

  const installed = [];
  for (const name of TEAM) {
    const src = path.join(gh, "team", name);
    const r = spawnSync(process.execPath, [assembler, src, name, target, gh], { encoding: "utf-8" });
    if (r.status !== 0) {
      fail(`assemble ${name} failed: ${((r.stderr || "").trim()) || ((r.stdout || "").trim())}`);
    }
    installed.push(JSON.parse((r.stdout || "").trim()));
  }

  const server = path.join(gh, "server");
  // binary name is .exe on Windows; report whichever exists so `memory_server_built` is correct cross-OS
  const exe = process.platform === "win32" ? ".exe" : "";
  let serverBin = path.join(server, "target", "release", "genesis-memory-server" + exe);
  if (!fs.existsSync(serverBin) && fs.existsSync(serverBin + ".exe")) {
    serverBin = serverBin + ".exe"; // WSL building a Windows target, etc.
  }
  const skillsInstalled = Array.from(
    new Set([].concat(...installed.map((i) => i.skills_installed || [])))
  ).sort();
  const report = {
    genesis_home: gh,
    target_repo: target,
    agents_installed: installed.map((i) => i.agent),
    agents_dir: path.join(target, ".claude", "agents"),
    skills_installed: skillsInstalled,
    memory_server_built: fs.existsSync(serverBin),
    memory_server_path: serverBin,
    next: 'Open Claude Code in the target repo; talk to Sensei: "build me an agent that …".',
  };
  console.log(JSON.stringify(report, null, 2));
  if (!report.memory_server_built) {
    process.stderr.write(
      "\nNote: the Rust memory server is not built yet. Build it with: " +
        `cd ${server} && cargo build --release\n`
    );
  }
}

if (require.main === module) {
  main();
}

module.exports = { main };
