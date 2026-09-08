#!/usr/bin/env node
/* Tests for the system-operation-maintenance expertise manifest — the rule index for genesis-engineer.

   Self-contained: Node stdlib only (node:assert + node:fs), NO external deps, NO network. Exits non-zero on
   any failure. Same shape of assertions the other Genesis expertise manifests are held to:

     * the manifest JSON parses and carries the expected top-level fields;
     * every rule has {id, section, text, type}, ids are unique, and type ∈ {checkable, judgment, principle};
     * a checkable rule carries a predicate {kind, spec}; a judgment rule carries a reviewer_criterion;
       a principle rule carries NEITHER;
     * no rule text targets the private reasoning trace (govern outputs, not the trace);
     * sections_accounted covers every "## "/"§" header in the source .md;
     * faithfulness (best-effort): every manifest rule id appears in the source .md;
     * the load-bearing rules exist and are typed as required (autonomy boundary as a wiring predicate,
       semver, changelog, keep-CI-green, memory-store consistency).

   Run:  node .genesis/expertise/manifests/tests/system-operation-maintenance.test.js
*/
"use strict";
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

// tests/ -> manifests/ -> expertise/
const MANIFEST_DIR = path.dirname(__dirname);
const EXPERTISE_DIR = path.dirname(MANIFEST_DIR);
const MANIFEST_PATH = path.join(MANIFEST_DIR, "system-operation-maintenance.json");
const GUIDE_PATH = path.join(EXPERTISE_DIR, "system-operation-maintenance.md");

const VALID_TYPES = new Set(["checkable", "judgment", "principle"]);
const TRACE_PHRASES = ["in your thinking", "in your reasoning", "in your extended thinking"];

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
function nonEmptyStr(v) {
  return typeof v === "string" && v.trim().length > 0;
}

// ── parse ──────────────────────────────────────────────────────────────────────────────
check("manifest file exists", fs.existsSync(MANIFEST_PATH));
check("guide file exists", fs.existsSync(GUIDE_PATH));

const raw = fs.readFileSync(MANIFEST_PATH, "utf8");
let manifest;
assert.doesNotThrow(() => {
  manifest = JSON.parse(raw);
}, "manifest JSON must parse");
check("manifest JSON parses", !!manifest);

const guide = fs.readFileSync(GUIDE_PATH, "utf8");

// ── top-level fields ─────────────────────────────────────────────────────────────────
check("expertise == 'system-operation-maintenance'", manifest.expertise === "system-operation-maintenance");
check(
  "source points at the guide .md",
  manifest.source === "expertise/system-operation-maintenance.md"
);
check("rules is a non-empty array", Array.isArray(manifest.rules) && manifest.rules.length > 0);
check(
  "sections_accounted is a non-empty object",
  manifest.sections_accounted && typeof manifest.sections_accounted === "object" &&
    Object.keys(manifest.sections_accounted).length > 0
);

// ── per-rule structure ───────────────────────────────────────────────────────────────
const rules = Array.isArray(manifest.rules) ? manifest.rules : [];
const ids = new Set();
let dupIds = 0;
let missingFields = 0;
let badType = 0;
let checkableBadPredicate = 0;
let judgmentBadCriterion = 0;
let principleHasExtra = 0;
let traceTargeting = 0;
let idNotInGuide = 0;

for (const r of rules) {
  if (!r || !nonEmptyStr(r.id) || !nonEmptyStr(r.section) || !nonEmptyStr(r.text) || !nonEmptyStr(r.type)) {
    missingFields += 1;
    continue;
  }
  if (ids.has(r.id)) dupIds += 1;
  ids.add(r.id);

  if (!VALID_TYPES.has(r.type)) badType += 1;

  if (r.type === "checkable") {
    const p = r.predicate;
    if (!p || typeof p !== "object" || !nonEmptyStr(p.kind) || !nonEmptyStr(p.spec)) {
      checkableBadPredicate += 1;
    }
  } else if (r.type === "judgment") {
    if (!nonEmptyStr(r.reviewer_criterion)) judgmentBadCriterion += 1;
  } else if (r.type === "principle") {
    if (r.predicate !== undefined || r.reviewer_criterion !== undefined) principleHasExtra += 1;
  }

  const lower = r.text.toLowerCase();
  if (TRACE_PHRASES.some((ph) => lower.includes(ph))) traceTargeting += 1;

  // faithfulness (best-effort): the rule id appears in the source guide.
  if (!guide.includes(r.id)) idNotInGuide += 1;
}

check("every rule has id/section/text/type", missingFields === 0);
check("all rule ids are unique", dupIds === 0);
check("every rule type is checkable|judgment|principle", badType === 0);
check("every checkable rule has predicate {kind, spec}", checkableBadPredicate === 0);
check("every judgment rule has a reviewer_criterion", judgmentBadCriterion === 0);
check("every principle rule has NEITHER predicate nor reviewer_criterion", principleHasExtra === 0);
check("no rule text targets the private reasoning trace", traceTargeting === 0);
check("faithfulness: every rule id appears in the guide .md", idNotInGuide === 0);

// ── sections_accounted covers every "## " / "§" header in the guide ─────────────────────
const headers = [];
for (const line of guide.split(/\r?\n/)) {
  const m = line.match(/^##\s+(.+?)\s*$/); // "## " section headers (## title, not ### )
  if (m && !line.startsWith("###")) headers.push(m[1].trim());
  const s = line.match(/^§\s*(.+?)\s*$/); // any literal "§" line-start header (none expected, handled anyway)
  if (s) headers.push(s[1].trim());
}
check("guide has section headers", headers.length > 0);

const saKeys = Object.keys(manifest.sections_accounted);
function headerAccounted(header) {
  const num = header.match(/^(\d+)\.\s+/); // "0. Operating posture" -> "0"
  if (num) {
    // require a key like "§0 ..." (and NOT "§10" for "0") — next char after the number is a non-digit.
    const re = new RegExp("^§\\s*" + num[1] + "(\\D|$)");
    return saKeys.some((k) => re.test(k.trim()));
  }
  // non-numbered header (e.g. "Source ledger") -> a key contains its title, case-insensitively.
  const h = header.toLowerCase();
  return saKeys.some((k) => k.toLowerCase().includes(h));
}
let uncovered = 0;
for (const h of headers) {
  if (!headerAccounted(h)) {
    uncovered += 1;
    console.log(`    (uncovered header) ${h}`);
  }
}
check("sections_accounted covers every guide header", uncovered === 0);

// ── load-bearing rules present + typed as required ─────────────────────────────────────
function ruleById(id) {
  return rules.find((r) => r && r.id === id);
}

const autonomy = ruleById("som-3");
check("som-3 (autonomy boundary) exists and is checkable", !!autonomy && autonomy.type === "checkable");
check(
  "som-3 is encoded as a wiring predicate about confirmation before deploy/publish/push/delete",
  !!autonomy &&
    autonomy.predicate &&
    autonomy.predicate.kind === "wiring" &&
    /confirm/i.test(autonomy.predicate.spec) &&
    /unattended|unauthor|explicit/i.test(autonomy.predicate.spec)
);

const semver = ruleById("som-8");
check(
  "som-8 (semver correctness) exists, checkable, with a semver predicate",
  !!semver && semver.type === "checkable" && semver.predicate && semver.predicate.kind === "semver"
);

const changelog = ruleById("som-22");
check(
  "som-22 (changelog) exists, checkable, references Keep a Changelog types",
  !!changelog &&
    changelog.type === "checkable" &&
    /added|changed|fixed|security|deprecated|removed|unreleased/i.test(changelog.text)
);

const greenCI = ruleById("som-15");
check(
  "som-15 (keep CI green) exists, checkable, blocks merge/release while red",
  !!greenCI && greenCI.type === "checkable" && /green|red/i.test(greenCI.text)
);

const memConsistency = ruleById("som-24");
check(
  "som-24 (memory-store consistency) exists, checkable, requires both .db + .jsonl committed",
  !!memConsistency &&
    memConsistency.type === "checkable" &&
    /memory\.db/i.test(memConsistency.text) &&
    /memory\.jsonl/i.test(memConsistency.text)
);

// A healthy manifest is predominantly checkable (determinism-first), with at least one of each other type.
const counts = { checkable: 0, judgment: 0, principle: 0 };
for (const r of rules) if (counts[r.type] !== undefined) counts[r.type] += 1;
check("has at least one checkable rule", counts.checkable > 0);
check("has at least one judgment rule", counts.judgment > 0);
check("has at least one principle rule", counts.principle > 0);
check("checkable rules are the majority (determinism-first)", counts.checkable > counts.judgment + counts.principle);

console.log(
  `\n  rules: ${rules.length}  (checkable ${counts.checkable}, judgment ${counts.judgment}, principle ${counts.principle})`
);
console.log(`\n${passed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
