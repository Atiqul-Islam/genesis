// Acceptance tests for the `genesis-engineer` agent — WRITTEN FIRST (test-driven).
//
// Node stdlib only (node:assert, node:fs, node:path, node:url) — no external deps.
// Run: node test_genesis_engineer.mjs   (exit 0 == all pass)
//
// The checks mechanically verify the CHECKABLE acceptance criteria from the task-spec:
//   AC-persona-limit : persona.md exists and <= 200 lines
//   AC-behavior-limit: behavior.md exists and <= 200 lines
//   AC-meta-tools    : meta.json parses; tools EQUALS the exact list; NO 'Grep'
//   AC-meta-expertise: required_expertise EQUALS the 6 names
//   AC-deep-read     : behavior encodes ENUMERATE-then-READ-FULLY + NEVER-grep
//   AC-confirm       : behavior encodes confirm-before deploy/publish/push/delete
//   AC-tdd-spec      : behavior encodes spec-first + failing-test-first (TDD)
//   AC-growth-gov    : skills/memory autonomous; ENFORCED rules routed to user
//   AC-declare       : APPLIED-EXPERTISE instruction names all 6 expertises
//   AC-pinned        : names pinned version + cite crate@version discipline
//   AC-no-banned     : no file contains the banned reasoning-trace phrase
//   AC-no-secret     : no authored file contains a credential VALUE
//   AC-skill         : at least one self-extension skill exists
//   AC-valid-ids     : every rule-id cited in the authored prose is real (in a manifest)
//   DISCRIMINATION   : the same presence checks all FAIL on an empty baseline

import assert from "node:assert";
import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const PERSONA = join(HERE, "persona.md");
const BEHAVIOR = join(HERE, "behavior.md");
const META = join(HERE, "meta.json");
const SKILLS = join(HERE, "skills");
const MANIFESTS = join(HERE, "..", "..", ".genesis", "expertise", "manifests");

// The banned reasoning-trace phrase is built by concatenation so this test file
// itself never literally contains it (it must be absent from every file).
const BANNED = "chain-of" + "-thought";

const EXPECTED_TOOLS = [
  "Read", "Write", "Edit", "Bash", "Glob", "Agent", "SendMessage",
  "mcp__genesis-memory__store", "mcp__genesis-memory__recall",
  "mcp__genesis-memory__consolidate", "WebFetch", "WebSearch",
];
const EXPECTED_EXPERTISE = [
  "expertise-application", "memory-management", "test-driven-determinism",
  "spec-driven-development", "system-operation-maintenance", "genesis-stack-mastery",
];

const results = [];
function check(name, passed, detail = "") {
  results.push([name, Boolean(passed), detail]);
}
function read(path) {
  try { return readFileSync(path, "utf-8"); }
  catch (err) { if (err && err.code === "ENOENT") return ""; throw err; }
}
function lineCount(s) {
  if (s === "") return 0;
  const parts = s.split(/\r\n|\r|\n/);
  if (parts.length && parts[parts.length - 1] === "") parts.pop();
  return parts.length;
}
function arrEq(a, b) {
  return Array.isArray(a) && Array.isArray(b) && a.length === b.length &&
    a.every((v, i) => v === b[i]);
}
// Walk every file under team/genesis-engineer/.
function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry);
    if (statSync(p).isDirectory()) out.push(...walk(p));
    else out.push(p);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Load authored text
// ---------------------------------------------------------------------------
const personaTxt = read(PERSONA);
const behaviorTxt = read(BEHAVIOR);
const metaTxt = read(META);

// ---------------------------------------------------------------------------
// Presence checks (behavior.md) — factored so a baseline "" fails them all.
// ---------------------------------------------------------------------------
function presenceChecks(behavior) {
  const c = behavior.toLowerCase();
  const has = (...ts) => ts.every((t) => c.includes(t));
  return {
    "deep-read: enumerate-then-read-fully": has("enumerate") &&
      has("read") && (c.includes("fully") || c.includes("in full")),
    "deep-read: never grep file content": c.includes("never grep") &&
      (c.includes("no grep tool") || c.includes("have no grep")),
    "confirm before irreversible (deploy/publish/push/delete)":
      (c.includes("explicit") && c.includes("authoriz")) &&
      has("deploy", "publish", "push") && c.includes("delet"),
    "tdd + spec-first": (c.includes("spec") && c.includes("first")) &&
      (c.includes("failing test") || (c.includes("red") && c.includes("green"))) &&
      (c.includes("tdd") || c.includes("test-driven")) &&
      c.includes("no spec, no code"),
    "growth: skills/memory autonomous": c.includes("skills") &&
      c.includes("autonomous") && c.includes("memory"),
    "growth: enforced rules routed to user for review":
      c.includes("enforced") && c.includes("review") && c.includes("user") &&
      c.includes("propose"),
    "declare APPLIED-EXPERTISE instruction": behavior.includes("APPLIED-EXPERTISE"),
    "names all 6 expertises": EXPECTED_EXPERTISE.every((n) => c.includes(n)),
    "pinned-version discipline + crate@version": c.includes("pinned") &&
      c.includes("version") && c.includes("crate@version") && c.includes("docs.rs"),
    "credential referenced not valued": c.includes("credential present at"),
    "uses 'structured reasoning' vocabulary": c.includes("structured reasoning"),
  };
}

// ---------------------------------------------------------------------------
// 1. File limits
// ---------------------------------------------------------------------------
check("persona.md exists", existsSync(PERSONA));
check("behavior.md exists", existsSync(BEHAVIOR));
const pLines = lineCount(personaTxt);
const bLines = lineCount(behaviorTxt);
check("persona.md <= 200 lines", pLines > 0 && pLines <= 200, `${pLines} lines`);
check("behavior.md <= 200 lines", bLines > 0 && bLines <= 200, `${bLines} lines`);

// ---------------------------------------------------------------------------
// 2. meta.json shape
// ---------------------------------------------------------------------------
let meta = {};
let metaParsed = true;
try { meta = JSON.parse(metaTxt || "{}"); } catch { metaParsed = false; }
check("meta.json parses", metaParsed && metaTxt.trim() !== "");
check("meta.tools EQUALS the exact tool list", arrEq(meta.tools, EXPECTED_TOOLS),
  JSON.stringify(meta.tools));
check("meta.tools does NOT include 'Grep'",
  Array.isArray(meta.tools) && !meta.tools.includes("Grep"));
check("meta.required_expertise EQUALS the 6 names",
  arrEq(meta.required_expertise, EXPECTED_EXPERTISE),
  JSON.stringify(meta.required_expertise));
check("meta.description is a non-empty string",
  typeof meta.description === "string" && meta.description.trim() !== "");

// ---------------------------------------------------------------------------
// 3. behavior.md presence checks
// ---------------------------------------------------------------------------
const authored = presenceChecks(behaviorTxt);
for (const [name, ok] of Object.entries(authored)) check(`BEHAVIOR [${name}]`, ok);

// ---------------------------------------------------------------------------
// 4. PER-FILE guardrails — prove neither file is dead weight: each of
//    persona.md and behavior.md must independently carry the no-grep and
//    confirm-before-irreversible guardrails.
// ---------------------------------------------------------------------------
function fileGuardrails(text) {
  const c = text.toLowerCase();
  return {
    "never grep": c.includes("never grep"),
    "confirm before irreversible":
      (c.includes("authoriz") || c.includes("confirm")) &&
      (c.includes("deploy") || c.includes("publish")) && c.includes("push"),
  };
}
for (const [fname, txt] of [["persona.md", personaTxt], ["behavior.md", behaviorTxt]]) {
  for (const [inv, ok] of Object.entries(fileGuardrails(txt))) {
    check(`PER-FILE [${fname}: ${inv}]`, ok);
  }
}

// ---------------------------------------------------------------------------
// 5. Skills — at least one self-extension skill (a */SKILL.md under skills/)
// ---------------------------------------------------------------------------
let skillFiles = [];
if (existsSync(SKILLS)) {
  skillFiles = walk(SKILLS).filter((p) => p.endsWith("SKILL.md"));
}
check("at least one skill (skills/*/SKILL.md) exists", skillFiles.length >= 1,
  `${skillFiles.length} found`);
// The self-extension skill must cover BOTH lanes: autonomous skills/memory AND
// routing a new enforced rule to the user.
{
  const skillText = skillFiles.map((p) => read(p)).join("\n").toLowerCase();
  check("skill covers autonomous skills/memory growth",
    skillText.includes("autonomous") &&
    (skillText.includes("skill") && skillText.includes("memory")));
  check("skill covers proposing an enforced rule to the user",
    skillText.includes("enforced") && skillText.includes("propose") &&
    skillText.includes("user"));
  check("skill front-matter has name + description",
    skillFiles.every((p) => {
      const t = read(p);
      return /name:\s*\S+/i.test(t) && /description:\s*\S+/i.test(t);
    }));
}

// ---------------------------------------------------------------------------
// 6. Hygiene — NO banned reasoning-trace phrase and NO credential VALUE in ANY
//    file under team/genesis-engineer/.
// ---------------------------------------------------------------------------
const allFiles = walk(HERE);
const secretPatterns = [
  /AKIA[0-9A-Z]{16}/,
  /-----BEGIN [A-Z ]*PRIVATE KEY-----/,
  /(password|passphrase|secret|api[_-]?key|token)\s*[:=]\s*\S+/i,
  /\bxox[baprs]-[0-9A-Za-z-]{10,}/,
  /\bgh[pousr]_[0-9A-Za-z]{20,}/,
];
{
  const bannedHits = allFiles.filter((p) => read(p).toLowerCase().includes(BANNED));
  check("no file contains the banned reasoning-trace phrase", bannedHits.length === 0,
    bannedHits.join(", "));

  // The credential scan excludes THIS test file, whose source legitimately holds
  // the detector patterns; every other file must be clean.
  const scan = allFiles.filter((p) => p !== fileURLToPath(import.meta.url));
  const secretHits = scan.filter((p) => {
    const t = read(p);
    return secretPatterns.some((re) => re.test(t));
  });
  check("no authored file contains a credential value", secretHits.length === 0,
    secretHits.join(", "));
}

// ---------------------------------------------------------------------------
// 7. Every rule-id cited in the authored prose is REAL (exists in a manifest).
//    Couples the agent to the real expertise store per the task-spec.
// ---------------------------------------------------------------------------
{
  const validIds = new Set();
  let manifestsOk = true;
  for (const name of EXPECTED_EXPERTISE) {
    const mp = join(MANIFESTS, `${name}.json`);
    if (!existsSync(mp)) { manifestsOk = false; continue; }
    try {
      const m = JSON.parse(read(mp));
      for (const r of m.rules) validIds.add(r.id);
    } catch { manifestsOk = false; }
  }
  check("6 expertise manifests are loadable", manifestsOk);

  const prose = [personaTxt, behaviorTxt, ...skillFiles.map((p) => read(p))].join("\n");
  const idRe = /\b(?:ea|mm|tdd|sdd|som|gsm)-[0-9][0-9a-z-]*/gi;
  const cited = [...new Set((prose.match(idRe) || []).map((s) => s.toLowerCase()))];
  const unknown = cited.filter((id) => !validIds.has(id));
  check("at least one real rule-id is cited", cited.length > 0, `${cited.length} cited`);
  check("every cited rule-id exists in a manifest", unknown.length === 0,
    `unknown: ${unknown.join(", ")}`);
}

// ---------------------------------------------------------------------------
// 8. DISCRIMINATION — the presence checks must all FAIL on an empty baseline,
//    proving they measure the authored content and not nothing.
// ---------------------------------------------------------------------------
const baseline = presenceChecks("");
const basePass = Object.values(baseline).filter(Boolean).length;
check("DISCRIMINATION: empty-baseline behavior fails every presence check",
  basePass === 0, `${basePass} passed on empty baseline`);

// ---------------------------------------------------------------------------
// REPORT
// ---------------------------------------------------------------------------
const passed = results.filter(([, ok]) => ok).length;
const failed = results.length - passed;
console.log("=".repeat(72));
for (const [name, ok, detail] of results) {
  let line = `[${ok ? "PASS" : "FAIL"}] ${name}`;
  if (detail && !ok) line += `  <- ${detail}`;
  console.log(line);
}
console.log("=".repeat(72));
console.log(`${passed} pass / ${failed} fail`);
assert.ok(true); // keep node:assert imported and meaningful
process.exit(failed === 0 ? 0 : 1);
