#!/usr/bin/env node
/* Portability tests for assemble.js frontmatter generation (§21 — cross-platform BUILT agents).

   Faithful Node (CommonJS, stdlib-only) port of test_portability.py, updated for the Node interpreter scheme.

   Proves a BUILT agent's generated hook `command:` lines are portable across machines/OSes:
     * interpreter is `node` (resolved via PATH at runtime), NOT this machine's absolute interpreter path;
     * hook scripts are referenced via `${CLAUDE_PROJECT_DIR}/.genesis/...` (Claude Code substitutes the
       project root at runtime, Windows + macOS + Linux) — NEVER an absolute machine path like /mnt/c/... or
       C:\\...;
     * paths are double-quoted (so a project dir with spaces survives) inside a YAML single-quoted scalar.

   Run:  node install/test_portability.js
*/
"use strict";
const assemble = require("./assemble.js");

const HOME = "${CLAUDE_PROJECT_DIR}/.genesis"; // what main() computes for the normal (gh = <repo>/.genesis) case

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
  // Undo a YAML single-quoted scalar: strip the wrapping quotes, undouble ''.
  if (!(scalar.startsWith("'") && scalar.endsWith("'"))) {
    throw new Error("not a single-quoted scalar: " + scalar);
  }
  return scalar.slice(1, -1).split("''").join("'");
}

// Minimal POSIX-style shlex.split (whitespace split; honors '…' , "…" and \-escapes). Enough for the
// double-quoted paths + bare tokens the assembler emits.
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
  const fm = assemble.frontmatter("method", { description: "d", tools: ["Read", "Write"] }, HOME, []);
  const cmds = commandLines(fm);
  check("four hook commands generated (inject/gate/validate/review)", cmds.length === 4);
  check(
    "all commands are YAML single-quoted",
    cmds.every((c) => c.startsWith("'") && c.endsWith("'"))
  );

  const shells = cmds.map(unyamlSingle);
  // Interpreter is portable `node`, never an absolute interpreter path.
  check("uses portable `node` interpreter", shells.every((s) => s.startsWith('"node" ')));
  // THE FIX: every hook path is ${CLAUDE_PROJECT_DIR}-relative — no absolute machine path anywhere.
  check(
    "every command references ${CLAUDE_PROJECT_DIR}",
    shells.every((s) => s.includes("${CLAUDE_PROJECT_DIR}/.genesis/hooks/"))
  );
  check(
    "NO absolute machine path leaked (no /mnt/c, no C:\\, no leading-slash abs path)",
    !shells.some((s) => s.includes("/mnt/") || s.includes("C:\\") || s.includes(' "/') || s.includes(":\\"))
  );

  // The ${CLAUDE_PROJECT_DIR} path is one intact shell token (double-quoted → survives spaces after runtime expansion).
  const injectTokens = shlexSplit(shells[0]);
  check("interpreter is token[0] = node", injectTokens[0] === "node");
  check(
    "inject.js path is one intact token",
    injectTokens.includes("${CLAUDE_PROJECT_DIR}/.genesis/hooks/inject.js")
  );
  check(
    "expertise dir is one intact token",
    injectTokens.includes("${CLAUDE_PROJECT_DIR}/.genesis/expertise")
  );
  check("agent name is the trailing token", injectTokens[injectTokens.length - 1] === "method");

  const stopTokens = shlexSplit(shells[2]);
  check(
    "stop cmd = node validate.js . method",
    stopTokens[0] === "node" &&
      stopTokens[1].endsWith("validate.js") &&
      stopTokens[stopTokens.length - 2] === "." &&
      stopTokens[stopTokens.length - 1] === "method"
  );
  const reviewTokens = shlexSplit(shells[3]);
  check(
    "review cmd = node review.js . method",
    reviewTokens[0] === "node" &&
      reviewTokens[1].endsWith("review.js") &&
      reviewTokens[reviewTokens.length - 2] === "." &&
      reviewTokens[reviewTokens.length - 1] === "method"
  );

  // A quoted path round-trips through the YAML single-quoted scalar unchanged.
  const win = assemble._yaml_cmd('"${CLAUDE_PROJECT_DIR}/.genesis/hooks/gate.js"');
  const inner = unyamlSingle(win.trim().slice("command:".length).trim());
  check(
    "quoted ${CLAUDE_PROJECT_DIR} path preserved literally",
    inner === '"${CLAUDE_PROJECT_DIR}/.genesis/hooks/gate.js"'
  );
  const qy = assemble._yaml_cmd('"/home/O\'Brien/hooks/gate.js"');
  check("single quote in path is YAML-doubled", qy.includes("''"));

  // Structural check in place of a YAML-library parse (Node stdlib has no YAML parser): the frontmatter
  // block declares the agent + the SessionStart/Stop hook events.
  const block = fm.split("---").slice(1, 2).join("");
  const structOk =
    /(^|\n)name: method(\n|$)/.test(block) && block.includes("SessionStart:") && block.includes("Stop:");
  check("frontmatter declares name + SessionStart + Stop (structural)", structOk);

  // Sensei gets an ADDITIONAL Sensei-only Bash gate (enforce_research); still portable.
  const senseiFm = assemble.frontmatter("sensei", { description: "d", tools: ["Read", "Bash", "Agent"] }, HOME, []);
  const scmds = commandLines(senseiFm);
  check("sensei has five hook commands (adds enforce_research)", scmds.length === 5);
  check(
    "sensei wires enforce_research under a Bash matcher (portable path)",
    senseiFm.includes('matcher: "Bash"') &&
      scmds.some((c) => unyamlSingle(c).includes("${CLAUDE_PROJECT_DIR}/.genesis/hooks/enforce_research.js"))
  );
  check(
    "method (non-sensei) has NO enforce_research / Bash matcher",
    !fm.includes("enforce_research.js") && !fm.includes('matcher: "Bash"')
  );
  // Sensei PreToolUse carries both the Write|Edit and Bash matchers (structural check).
  check(
    "sensei PreToolUse has both Write|Edit and Bash matchers",
    senseiFm.includes('matcher: "Write|Edit"') && senseiFm.includes('matcher: "Bash"')
  );

  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
