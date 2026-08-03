#!/usr/bin/env node
/* Portability tests for assemble.js frontmatter generation (cross-platform BUILT agents).

   Proves a BUILT agent's generated hooks are portable across machines/OSes:
     * deterministic hooks (inject/gate/enforce-research/validate) invoke the native `genesis-hook`
       binary DIRECTLY at ${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook[.exe] — no Node in the hot path;
     * the path is ${CLAUDE_PROJECT_DIR}-relative (Claude Code substitutes the project root at runtime),
       NEVER an absolute machine path like /mnt/c/... or C:\\...;
     * the semantic `review` is a built-in `agent` hook (fast Haiku model), not a spawned process;
     * paths are double-quoted (project dir with spaces survives) inside a YAML single-quoted scalar.

   Run:  node install/test_portability.js
*/
"use strict";
const assemble = require("./assemble.js");

const HOME = "${CLAUDE_PROJECT_DIR}/.genesis"; // what main() computes for the normal (gh = <repo>/.genesis) case
const BIN_STEM = "genesis-hook" + (process.platform === "win32" ? ".exe" : "");
const BIN_PATH = "${CLAUDE_PROJECT_DIR}/.genesis/bin/" + BIN_STEM;
const EXP = ["persona-creation", "prompt-engineering"];

let passed = 0;
let failed = 0;

function check(name, cond) {
  if (cond) {
    passed += 1;
  } else {
    failed += 1;
  }
  console.log(`  ${cond ? "PASS" : "FAIL"}  ${name}`);
}

function commandLines(fm) {
  return fm
    .split(/\r?\n/)
    .filter((ln) => ln.trim().startsWith("command:"))
    .map((ln) => ln.trim().slice("command:".length).trim());
}

function unyamlSingle(scalar) {
  if (!(scalar.startsWith("'") && scalar.endsWith("'"))) {
    throw new Error("not a single-quoted scalar: " + scalar);
  }
  return scalar.slice(1, -1).split("''").join("'");
}

// Minimal POSIX-style shlex.split (whitespace split; honors '…', "…", \-escapes).
function shlexSplit(s) {
  const tokens = [];
  let cur = "";
  let inToken = false;
  let i = 0;
  const n = s.length;
  while (i < n) {
    const c = s[i];
    if (c === "\\") {
      inToken = true;
      i += 1;
      if (i < n) {
        cur += s[i];
        i += 1;
      }
      continue;
    }
    if (c === "'") {
      inToken = true;
      i += 1;
      const j = s.indexOf("'", i);
      if (j === -1) throw new Error("No closing quotation");
      cur += s.slice(i, j);
      i = j + 1;
      continue;
    }
    if (c === '"') {
      inToken = true;
      i += 1;
      let closed = false;
      while (i < n) {
        const d = s[i];
        if (d === "\\") {
          i += 1;
          if (i < n) {
            const e = s[i];
            if (e === '"' || e === "\\" || e === "$" || e === "`" || e === "\n") cur += e;
            else cur += "\\" + e;
            i += 1;
          }
        } else if (d === '"') {
          closed = true;
          i += 1;
          break;
        } else {
          cur += d;
          i += 1;
        }
      }
      if (!closed) throw new Error("No closing quotation");
      continue;
    }
    if (/\s/.test(c)) {
      if (inToken) {
        tokens.push(cur);
        cur = "";
        inToken = false;
      }
      i += 1;
      continue;
    }
    inToken = true;
    cur += c;
    i += 1;
  }
  if (inToken) tokens.push(cur);
  return tokens;
}

function main() {
  const fm = assemble.frontmatter("method", { description: "d", tools: ["Read", "Write"] }, HOME, [], EXP);
  const cmds = commandLines(fm);
  check("three deterministic hook commands (inject/gate/validate)", cmds.length === 3);
  check("all commands are YAML single-quoted", cmds.every((c) => c.startsWith("'") && c.endsWith("'")));

  const shells = cmds.map(unyamlSingle);
  check("every command invokes the genesis-hook binary directly", shells.every((s) => s.startsWith('"' + BIN_PATH + '" ')));
  check("NO `node` interpreter in the hot path", !shells.some((s) => s.includes('"node"')));
  check("every command references ${CLAUDE_PROJECT_DIR} (portable base)", shells.every((s) => s.includes("${CLAUDE_PROJECT_DIR}/.genesis/")));
  check(
    "NO absolute machine path leaked (no /mnt/, C:\\, leading-slash abs path)",
    !shells.some((s) => s.includes("/mnt/") || s.includes("C:\\") || s.includes(' "/') || s.includes(":\\"))
  );

  // inject: <bin> inject "<exp>" method
  const injectTokens = shlexSplit(shells[0]);
  check("inject: binary is token[0]", injectTokens[0] === BIN_PATH);
  check("inject: subcommand is 'inject'", injectTokens[1] === "inject");
  check("inject: expertise dir is one intact token", injectTokens.includes("${CLAUDE_PROJECT_DIR}/.genesis/expertise"));
  check("inject: agent name is the trailing token", injectTokens[injectTokens.length - 1] === "method");

  // gate: <bin> gate --expertise "<exp>"
  const gateTokens = shlexSplit(shells[1]);
  check("gate: `gate --expertise <exp>`", gateTokens[1] === "gate" && gateTokens.includes("--expertise"));

  // validate: <bin> validate . method --expertise "<exp>"
  const stopTokens = shlexSplit(shells[2]);
  check(
    "validate: `validate . method --expertise <exp>`",
    stopTokens[1] === "validate" && stopTokens[2] === "." && stopTokens[3] === "method" && stopTokens.includes("--expertise")
  );

  // review is a built-in `agent` hook, not a command
  check(
    "review is a built-in `agent` hook (type: agent + haiku model + prompt)",
    fm.includes("- type: agent") && fm.includes("model: 'claude-haiku") && /\n\s+prompt: '/.test(fm)
  );
  check(
    "review agent hook names the reviewed agent + injects $ARGUMENTS",
    // the agent name's single quotes are YAML-doubled inside the single-quoted scalar ('method' -> ''method'')
    fm.includes("reviewer for the Genesis agent ''method''") && fm.includes("$ARGUMENTS")
  );

  // method with NO expertise -> no review agent hook (nothing to review)
  const fmNoExp = assemble.frontmatter("method", { description: "d", tools: ["Read"] }, HOME, [], []);
  check("no required expertise -> no review agent hook", !fmNoExp.includes("- type: agent"));

  // A quoted path round-trips through the YAML single-quoted scalar unchanged.
  const win = assemble._yaml_cmd('"${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook"');
  const inner = unyamlSingle(win.trim().slice("command:".length).trim());
  check("quoted ${CLAUDE_PROJECT_DIR} path preserved literally", inner === '"${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook"');
  const qy = assemble._yaml_cmd('"/home/O\'Brien/bin/genesis-hook"');
  check("single quote in path is YAML-doubled", qy.includes("''"));

  // Structural check (Node stdlib has no YAML parser): frontmatter declares agent + SessionStart/Stop events.
  const block = fm.split("---").slice(1, 2).join("");
  const structOk =
    /(^|\n)name: method(\n|$)/.test(block) && block.includes("SessionStart:") && block.includes("Stop:");
  check("frontmatter declares name + SessionStart + Stop (structural)", structOk);

  // Sensei gets an ADDITIONAL Sensei-only Bash gate (enforce-research); still portable.
  const senseiFm = assemble.frontmatter("sensei", { description: "d", tools: ["Read", "Bash", "Agent"] }, HOME, [], EXP);
  const scmds = commandLines(senseiFm);
  check("sensei has four deterministic commands (adds enforce-research)", scmds.length === 4);
  check(
    "sensei wires enforce-research under a Bash matcher (binary)",
    senseiFm.includes('matcher: "Bash"') &&
      scmds.some((c) => unyamlSingle(c).includes(BIN_STEM) && unyamlSingle(c).includes("enforce-research"))
  );
  check(
    "method (non-sensei) has NO enforce-research / Bash matcher",
    !fm.includes("enforce-research") && !fm.includes('matcher: "Bash"')
  );
  check(
    "sensei PreToolUse has both Write|Edit and Bash matchers",
    senseiFm.includes('matcher: "Write|Edit"') && senseiFm.includes('matcher: "Bash"')
  );

  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
