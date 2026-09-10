#!/usr/bin/env node
/* Self-contained validator for the test-driven-determinism expertise module.

   Built-ins only (node:assert + node:fs) — NO external deps. Exits non-zero on ANY failure.

   Verifies (per the authoring contract):
     (a) the manifest JSON parses;
     (b) every rule has non-empty id/section/text/type; ids are unique; type ∈ {checkable,judgment,principle};
     (c) every checkable rule carries a {kind,spec} predicate (kind ∈ the allowed set) and every judgment rule
         carries a non-empty reviewer_criterion; principle rules carry NEITHER;
     (d) sections_accounted is non-empty AND every "## " section header in the guide maps to a key in it;
     (e) faithfulness (best-effort, WARN-only): each rule's distinctive vocabulary traces to the guide text.

   Run:  node .genesis/expertise/manifests/tests/test-driven-determinism.test.js
*/
"use strict";
const assert = require("node:assert");
const fs = require("node:fs");

// ── paths (string-built to avoid any non-builtin import) ───────────────────────────────────────
const HERE = __dirname; // .../manifests/tests
const MANIFEST = HERE + "/../test-driven-determinism.json";
const GUIDE = HERE + "/../../test-driven-determinism.md";

// Defensive per the contract: create manifests/tests/ if it is somehow absent.
if (!fs.existsSync(HERE)) fs.mkdirSync(HERE, { recursive: true });

let passed = 0;
let failed = 0;
const warnings = [];

function check(name, cond) {
  if (cond) {
    passed += 1;
  } else {
    failed += 1;
    console.log(`  FAIL  ${name}`);
    return false;
  }
  console.log(`  PASS  ${name}`);
  return true;
}

// ── (a) parse ──────────────────────────────────────────────────────────────────────────────────
check("manifest file exists", fs.existsSync(MANIFEST));
check("guide file exists", fs.existsSync(GUIDE));

let manifest;
try {
  manifest = JSON.parse(fs.readFileSync(MANIFEST, "utf-8"));
  check("(a) manifest parses as JSON", true);
} catch (e) {
  check("(a) manifest parses as JSON", false);
  console.log(`\n${passed} passed, ${failed} failed — cannot continue without a parseable manifest.`);
  process.exit(1);
}

const guideText = fs.readFileSync(GUIDE, "utf-8");
const guideLower = guideText.toLowerCase();

// top-level shape
check("(a) top-level has expertise === 'test-driven-determinism'", manifest.expertise === "test-driven-determinism");
check("(a) top-level has source, note, schema", !!manifest.source && !!manifest.note && !!manifest.schema);
check("(a) rules is a non-empty array", Array.isArray(manifest.rules) && manifest.rules.length > 0);

const rules = Array.isArray(manifest.rules) ? manifest.rules : [];
const TYPES = new Set(["checkable", "judgment", "principle"]);
const KINDS = new Set(["structure", "declaration", "wiring", "regex", "artifact", "command"]);

// ── (b) per-rule required fields, unique ids, valid type ─────────────────────────────────────────
const seenIds = new Set();
let dupIds = 0;
let badField = 0;
let badType = 0;
for (const r of rules) {
  const idOk = typeof r.id === "string" && r.id.trim().length > 0;
  const secOk = typeof r.section === "string" && r.section.trim().length > 0;
  const txtOk = typeof r.text === "string" && r.text.trim().length > 0;
  const typOk = typeof r.type === "string" && r.type.trim().length > 0;
  if (!(idOk && secOk && txtOk && typOk)) {
    badField += 1;
    console.log(`    - rule ${r.id || "<no id>"} missing a non-empty id/section/text/type`);
  }
  if (idOk) {
    if (seenIds.has(r.id)) {
      dupIds += 1;
      console.log(`    - duplicate id: ${r.id}`);
    }
    seenIds.add(r.id);
  }
  if (!TYPES.has(r.type)) {
    badType += 1;
    console.log(`    - rule ${r.id}: type '${r.type}' not in {checkable,judgment,principle}`);
  }
}
check("(b) every rule has non-empty id/section/text/type", badField === 0);
check("(b) all rule ids are unique", dupIds === 0);
check("(b) every rule type ∈ {checkable,judgment,principle}", badType === 0);
check("(b) every rule id is prefixed 'tdd-'", rules.every((r) => typeof r.id === "string" && r.id.startsWith("tdd-")));

// ── (c) predicate / reviewer_criterion by type ───────────────────────────────────────────────────
let cErr = 0;
for (const r of rules) {
  if (r.type === "checkable") {
    const p = r.predicate;
    const ok =
      p && typeof p === "object" && !Array.isArray(p) &&
      typeof p.kind === "string" && p.kind.trim().length > 0 &&
      typeof p.spec === "string" && p.spec.trim().length > 0;
    if (!ok) {
      cErr += 1;
      console.log(`    - checkable rule ${r.id} lacks a {kind,spec} predicate`);
    } else if (!KINDS.has(p.kind)) {
      cErr += 1;
      console.log(`    - checkable rule ${r.id}: predicate.kind '${p.kind}' not in the allowed set`);
    }
    if (r.reviewer_criterion !== undefined) {
      cErr += 1;
      console.log(`    - checkable rule ${r.id} must NOT carry a reviewer_criterion`);
    }
  } else if (r.type === "judgment") {
    const ok = typeof r.reviewer_criterion === "string" && r.reviewer_criterion.trim().length > 0;
    if (!ok) {
      cErr += 1;
      console.log(`    - judgment rule ${r.id} lacks a non-empty reviewer_criterion`);
    }
    if (r.predicate !== undefined) {
      cErr += 1;
      console.log(`    - judgment rule ${r.id} must NOT carry a predicate`);
    }
  } else if (r.type === "principle") {
    if (r.predicate !== undefined || r.reviewer_criterion !== undefined) {
      cErr += 1;
      console.log(`    - principle rule ${r.id} must carry NEITHER predicate nor reviewer_criterion`);
    }
  }
}
check("(c) predicates/reviewer_criteria match each rule's type", cErr === 0);

// ── (d) sections_accounted covers every guide section header ─────────────────────────────────────
const sa = manifest.sections_accounted;
check("(d) sections_accounted is a non-empty object",
  sa && typeof sa === "object" && !Array.isArray(sa) && Object.keys(sa).length > 0);

const saKeys = sa && typeof sa === "object" ? Object.keys(sa) : [];
const saKeysLower = saKeys.map((k) => k.toLowerCase());
// numeric signatures present among the keys (keys like "§6 coverage ...").
const saNums = new Set();
for (const k of saKeys) {
  const m = k.match(/§\s*(\d+)/); // section-number tag inside a key
  if (m) saNums.add(Number(m[1]));
}

// Collect guide "## " headers, skipping fenced code blocks.
const headers = [];
let inFence = false;
for (const raw of guideText.split(/\r?\n/)) {
  const t = raw.trim();
  if (t.startsWith("```")) { inFence = !inFence; continue; }
  if (inFence) continue;
  if (raw.startsWith("## ")) headers.push(raw.slice(3).trim());
}
check("(d) guide exposes at least one '## ' section header", headers.length > 0);

const STOP = new Set(["the", "and", "a", "an", "of", "to", "is", "for", "before", "you", "your"]);
let missing = 0;
for (const h of headers) {
  const numMatch = h.match(/^(\d+)\.\s/);
  if (numMatch) {
    const n = Number(numMatch[1]);
    if (!saNums.has(n)) {
      missing += 1;
      console.log(`    - guide header "${h}" (§${n}) has no matching sections_accounted key`);
    }
  } else {
    // unnumbered header (e.g. "Commands and thresholds table (...)", "Source ledger")
    const firstWord = (h.toLowerCase().match(/[a-z][a-z0-9-]+/g) || []).find((w) => !STOP.has(w)) || "";
    const hit = firstWord && saKeysLower.some((k) => k.includes(firstWord));
    if (!hit) {
      missing += 1;
      console.log(`    - guide header "${h}" has no matching sections_accounted key`);
    }
  }
}
check("(d) every guide section header appears in sections_accounted", missing === 0);

// ── (e) faithfulness — best-effort, WARN-only ────────────────────────────────────────────────────
// A rule "traces" if any of its distinctive tokens (length ≥ 6, incl. hyphenated tech terms) appears
// in the guide text. This is a heuristic tripwire, not a hard gate (the .md is authoritative).
function distinctiveTokens(text) {
  const toks = (text.match(/[A-Za-z][A-Za-z0-9_-]{5,}/g) || []).map((w) => w.toLowerCase());
  return Array.from(new Set(toks));
}
for (const r of rules) {
  const toks = distinctiveTokens(r.text || "");
  const traced = toks.some((t) => guideLower.includes(t));
  if (!traced) warnings.push(`rule ${r.id}: no distinctive phrase found in the guide (review faithfulness)`);
}
if (warnings.length) {
  console.log("\nFAITHFULNESS WARNINGS (non-fatal):");
  for (const w of warnings) console.log(`  ! ${w}`);
} else {
  console.log("\n(e) faithfulness: every rule traces a distinctive phrase to the guide — no warnings.");
}

// ── summary ──────────────────────────────────────────────────────────────────────────────────────
const counts = { checkable: 0, judgment: 0, principle: 0 };
for (const r of rules) if (counts[r.type] !== undefined) counts[r.type] += 1;
console.log(
  `\nrules: ${rules.length} (checkable ${counts.checkable}, judgment ${counts.judgment}, principle ${counts.principle})`
);
console.log(`sections_accounted keys: ${saKeys.length}; guide headers: ${headers.length}`);
console.log(`\n${passed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
