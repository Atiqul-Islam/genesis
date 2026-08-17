#!/usr/bin/env node
// skill_structure.test.mjs — structural gate for /genesis:build-engineer: the command file
// plus its two skills (build-engineer, grill). Asserts required CONCEPTS are present (not
// tautological exact-string echoes) in each file's front-matter and body. node:assert +
// node:fs only, no external deps. Exits non-zero on any failure.
//
// Covers:
//   commands/build-engineer.md
//   skills/build-engineer/SKILL.md
//   skills/grill/SKILL.md

import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import assert from "node:assert/strict";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, "..", "..", "..");

const CMD_PATH = join(REPO_ROOT, "commands", "build-engineer.md");
const BUILD_SKILL_PATH = join(REPO_ROOT, "skills", "build-engineer", "SKILL.md");
const GRILL_SKILL_PATH = join(REPO_ROOT, "skills", "grill", "SKILL.md");

// Banned reasoning-trace phrase — built at runtime so this file never contains the literal
// (house rule: never write the banned phrase, even inside a check for it).
const BANNED_PHRASE = ["chain", "of", "thought"].join("-");

// ---------------------------------------------------------------------------
// tiny check harness
// ---------------------------------------------------------------------------
const checks = [];
function check(label, fn) {
  try {
    fn();
    checks.push({ label, pass: true });
  } catch (err) {
    checks.push({ label, pass: false, detail: err.message });
  }
}

function readFile(path) {
  if (!existsSync(path)) {
    throw new Error(`file does not exist: ${path}`);
  }
  return readFileSync(path, "utf8");
}

// Split a `---\n<front-matter>\n---\n<body>` markdown file.
function splitFrontMatter(raw) {
  const m = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
  if (!m) throw new Error("no YAML front-matter block found (--- ... ---)");
  return { front: m[1], body: m[2] };
}

function frontField(front, key) {
  const m = front.match(new RegExp(`^${key}:\\s*(.+)$`, "m"));
  return m ? m[1].trim() : undefined;
}

// Find each [label, regex] pair IN ORDER: each search starts only from where the
// previous match ended, so an earlier incidental mention of the same word (e.g. a
// summary sentence before the numbered steps) can never satisfy a later step and
// break the ordering guarantee.
function findSequential(text, steps) {
  let cursor = 0;
  const results = [];
  for (const [label, re] of steps) {
    const sub = text.slice(cursor);
    const m = sub.match(re);
    if (!m) {
      throw new Error(
        `step concept "${label}" (${re}) not found at or after position ${cursor} — ` +
          `either missing entirely or appears out of order relative to the previous step`
      );
    }
    const idx = cursor + m.index;
    results.push({ label, idx });
    cursor = idx + m[0].length;
  }
  return results;
}

function assertNoBannedPhrase(raw, fileLabel) {
  assert.ok(
    !raw.toLowerCase().includes(BANNED_PHRASE),
    `${fileLabel} contains the banned reasoning-trace phrase`
  );
}

// ===========================================================================
// commands/build-engineer.md
// ===========================================================================
let cmdRaw, cmdBody;

check("commands/build-engineer.md exists", () => {
  cmdRaw = readFile(CMD_PATH);
});

check("commands/build-engineer.md has YAML front-matter with non-empty description", () => {
  const { front, body } = splitFrontMatter(cmdRaw);
  cmdBody = body;
  const desc = frontField(front, "description");
  assert.ok(desc && desc.length > 0, "front-matter `description` missing or empty");
});

check("commands/build-engineer.md body references the sensei agent", () => {
  assert.match(cmdBody, /\bsensei\b/i);
});

check("commands/build-engineer.md body references the build-engineer skill", () => {
  assert.match(cmdBody, /build-engineer[^\n]{0,20}\bskill\b|\bskill\b[^\n]{0,20}build-engineer/i);
});

check("commands/build-engineer.md body mentions deep-read mode (existing repo)", () => {
  assert.match(cmdBody, /deep-read/i, "no mention of deep-read mode");
  assert.match(cmdBody, /existing repo/i, "no mention of an existing repo as the deep-read target");
});

check("commands/build-engineer.md body mentions grill mode (empty folder)", () => {
  assert.match(cmdBody, /grill mode/i, "no mention of grill mode");
  assert.match(cmdBody, /empty/i, "no mention of an empty target for grill mode");
});

check("commands/build-engineer.md has no banned reasoning-trace phrase", () => {
  assertNoBannedPhrase(cmdRaw, "commands/build-engineer.md");
});

// ===========================================================================
// skills/build-engineer/SKILL.md
// ===========================================================================
let beRaw, beBody;

check("skills/build-engineer/SKILL.md exists", () => {
  beRaw = readFile(BUILD_SKILL_PATH);
});

check("skills/build-engineer/SKILL.md front-matter: name=build-engineer, non-empty description", () => {
  const { front, body } = splitFrontMatter(beRaw);
  beBody = body;
  const name = frontField(front, "name");
  const desc = frontField(front, "description");
  assert.equal(name, "build-engineer", `front-matter \`name\` was "${name}", expected "build-engineer"`);
  assert.ok(desc && desc.length > 0, "front-matter `description` missing or empty");
});

check("skills/build-engineer/SKILL.md body contains the 8 procedure steps, IN ORDER", () => {
  const steps = [
    ["detect mode", /detect\s+mode/i],
    ["minimal asks", /minimal\s+asks?/i],
    ["bootstrap", /bootstrap/i],
    ["expertise (deep-read vs grill)", /expertise/i],
    ["delegate to Method", /delegate\s+to\s+method/i],
    ["assemble", /assemble/i],
    ["verify", /verify/i],
    ["offer promote", /promote/i],
  ];
  findSequential(beBody, steps);
});

check("skills/build-engineer/SKILL.md expertise step distinguishes deep-read vs grill branches", () => {
  assert.match(beBody, /deep-read/i, "no deep-read branch mentioned");
  assert.match(beBody, /grill/i, "no grill branch mentioned");
});

check("skills/build-engineer/SKILL.md states both hard rules (never speculate; never shortcut / production-ready)", () => {
  assert.match(beBody, /never speculate/i, "hard rule 1 (never speculate) not stated");
  assert.match(beBody, /never shortcut/i, "hard rule 2 (never shortcut) not stated");
  assert.match(beBody, /production-ready/i, "\"production-ready\" outcome of the never-shortcut rule not stated");
});

check("skills/build-engineer/SKILL.md has no banned reasoning-trace phrase", () => {
  assertNoBannedPhrase(beRaw, "skills/build-engineer/SKILL.md");
});

// ===========================================================================
// skills/grill/SKILL.md
// ===========================================================================
let grillRaw, grillBody;

check("skills/grill/SKILL.md exists", () => {
  grillRaw = readFile(GRILL_SKILL_PATH);
});

check("skills/grill/SKILL.md front-matter: name=grill, non-empty description", () => {
  const { front, body } = splitFrontMatter(grillRaw);
  grillBody = body;
  const name = frontField(front, "name");
  const desc = frontField(front, "description");
  assert.equal(name, "grill", `front-matter \`name\` was "${name}", expected "grill"`);
  assert.ok(desc && desc.length > 0, "front-matter `description` missing or empty");
});

check("skills/grill/SKILL.md body covers all 8 onboarding topics", () => {
  const topics = [
    ["goals & success criteria", /goals?[^\n]*success/i],
    ["stack + exact versions", /stack[^\n]*version|version[^\n]*stack/i],
    ["architecture", /architecture/i],
    ["conventions / coding standards", /convention/i],
    ["testing & CI", /testing[^\n]*\bci\b/i],
    ["deployment & operations", /deployment[^\n]*operat/i],
    ["done-criteria", /done.criteria/i],
    ["escalation triggers", /escalation/i],
  ];
  for (const [label, re] of topics) {
    assert.match(grillBody, re, `missing onboarding topic: ${label}`);
  }
});

check("skills/grill/SKILL.md states it does not finish until every topic is answered", () => {
  assert.match(
    grillBody,
    /not.*finish|until.*answered|every.*(topic|question).*answered/i,
    "no statement that onboarding does not finish until every topic is answered"
  );
});

check("skills/grill/SKILL.md mentions capturing answers to memory under agent_id", () => {
  assert.match(grillBody, /memory/i, "no mention of memory");
  assert.match(grillBody, /agent_id/i, "no mention of agent_id");
});

check("skills/grill/SKILL.md has no banned reasoning-trace phrase", () => {
  assertNoBannedPhrase(grillRaw, "skills/grill/SKILL.md");
});

// ===========================================================================
// summary
// ===========================================================================
let failCount = 0;
for (const c of checks) {
  if (c.pass) {
    console.log(`  PASS  ${c.label}`);
  } else {
    failCount += 1;
    console.log(`  FAIL  ${c.label}`);
    console.log(`        ${c.detail}`);
  }
}

const total = checks.length;
const passCount = total - failCount;
console.log(`\n${passCount}/${total} checks passed.`);

if (failCount > 0) {
  console.log(`RESULT: FAIL (${failCount} failing)`);
  process.exit(1);
} else {
  console.log("RESULT: PASS");
  process.exit(0);
}
