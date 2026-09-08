#!/usr/bin/env node
"use strict";

// Self-contained test for the `genesis-stack-mastery` expertise module.
// Deps: node:assert + node:fs only. Exit non-zero on any failure.
//
// Asserts (mirrors the sibling manifests' structural + faithfulness contract, plus a
// version-pin check unique to this knowledge-heavy module):
//   1. STRUCTURE  — manifest schema, typed ID'd rules, predicate/reviewer_criterion by type.
//   2. FAITHFULNESS — every rule id is accounted for in a guide section; every version token a
//      rule mentions appears verbatim in the guide (the manifest invents no version).
//   3. VERSION-PIN — every version string a rule carries is a concrete pinned version (has
//      digits); no rule pins/verifies against "latest" or "*".
//   4. HOUSE RULES — neither artifact leaks the banned research phrase or a credential value.

const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const HERE = __dirname;
const MANIFEST_PATH = path.join(HERE, "..", "genesis-stack-mastery.json");
const GUIDE_PATH = path.join(HERE, "..", "..", "genesis-stack-mastery.md");

let failures = 0;
function check(desc, fn) {
  try {
    fn();
    console.log("  ok   - " + desc);
  } catch (e) {
    failures++;
    console.error("  FAIL - " + desc + "\n         " + (e && e.message ? e.message : String(e)));
  }
}

// ---- load ---------------------------------------------------------------------------------
assert.ok(fs.existsSync(MANIFEST_PATH), "manifest file missing: " + MANIFEST_PATH);
assert.ok(fs.existsSync(GUIDE_PATH), "guide file missing: " + GUIDE_PATH);
const rawManifest = fs.readFileSync(MANIFEST_PATH, "utf8");
const guide = fs.readFileSync(GUIDE_PATH, "utf8");
let m;
try {
  m = JSON.parse(rawManifest);
} catch (e) {
  console.error("manifest is not valid JSON: " + e.message);
  process.exit(1);
}

const VALID_TYPES = new Set(["checkable", "judgment", "principle"]);
const VALID_PRED_KINDS = new Set(["structure", "declaration", "wiring", "regex", "artifact", "command"]);
// numeric semver-ish token: needs a dot between digit groups so "18"/"2021"/"384"/"2024-11-05" are excluded.
const VERSION_RE = /\d+\.\d+(?:\.\d+)?(?:-[0-9A-Za-z.]+)?/g;
// a wildcard *pin/verify* (not a prose mention of the word): "= *", "@latest", 'version = "latest"'.
const WILDCARD_PIN_RE = /(?:version\s*=\s*|=\s*|@)["']?(?:latest|\*)["']?/i;

console.log("\n# genesis-stack-mastery manifest\n");

// ---- 1. top-level shape -------------------------------------------------------------------
check("expertise id is genesis-stack-mastery", () => {
  assert.strictEqual(m.expertise, "genesis-stack-mastery");
});
check("source points at the guide .md", () => {
  assert.ok(/genesis-stack-mastery\.md$/.test(m.source), "source: " + m.source);
});
check("has a non-empty note and schema block", () => {
  assert.ok(typeof m.note === "string" && m.note.length > 20, "note missing/short");
  assert.ok(m.schema && typeof m.schema === "object", "schema block missing");
  for (const k of ["id", "section", "text", "type"]) {
    assert.ok(typeof m.schema[k] === "string" && m.schema[k].length > 0, "schema key missing: " + k);
  }
});
check("rules is a non-empty array", () => {
  assert.ok(Array.isArray(m.rules) && m.rules.length > 0, "rules missing/empty");
});
check("sections_accounted is a non-empty object", () => {
  assert.ok(m.sections_accounted && typeof m.sections_accounted === "object", "missing");
  assert.ok(Object.keys(m.sections_accounted).length > 0, "empty");
});

// ---- 2. per-rule structure ----------------------------------------------------------------
const ids = new Set();
const typeCounts = { checkable: 0, judgment: 0, principle: 0 };
check("every rule is well-formed and typed; ids unique + gsm- prefixed", () => {
  for (const r of m.rules) {
    assert.ok(r && typeof r === "object", "rule not an object");
    assert.ok(typeof r.id === "string" && r.id.length > 0, "rule missing id");
    assert.ok(/^gsm-/.test(r.id), "rule id not gsm- prefixed: " + r.id);
    assert.ok(!ids.has(r.id), "duplicate rule id: " + r.id);
    ids.add(r.id);
    assert.ok(typeof r.section === "string" && r.section.length > 0, "rule missing section: " + r.id);
    assert.ok(typeof r.text === "string" && r.text.length > 0, "rule missing text: " + r.id);
    assert.ok(VALID_TYPES.has(r.type), "bad type on " + r.id + ": " + r.type);
    typeCounts[r.type]++;
  }
});
check("checkable rules carry a {kind, spec} predicate with a valid kind", () => {
  for (const r of m.rules) {
    if (r.type !== "checkable") continue;
    assert.ok(r.predicate && typeof r.predicate === "object", "no predicate on " + r.id);
    assert.ok(VALID_PRED_KINDS.has(r.predicate.kind), "bad predicate.kind on " + r.id + ": " + r.predicate.kind);
    assert.ok(typeof r.predicate.spec === "string" && r.predicate.spec.length > 0, "no predicate.spec on " + r.id);
    assert.ok(!("reviewer_criterion" in r), "checkable rule also has reviewer_criterion: " + r.id);
  }
});
check("judgment rules carry a reviewer_criterion (and no predicate)", () => {
  for (const r of m.rules) {
    if (r.type !== "judgment") continue;
    assert.ok(typeof r.reviewer_criterion === "string" && r.reviewer_criterion.length > 0, "no reviewer_criterion on " + r.id);
    assert.ok(!("predicate" in r), "judgment rule also has predicate: " + r.id);
  }
});
check("principle rules carry neither predicate nor reviewer_criterion", () => {
  for (const r of m.rules) {
    if (r.type !== "principle") continue;
    assert.ok(!("predicate" in r), "principle rule has predicate: " + r.id);
    assert.ok(!("reviewer_criterion" in r), "principle rule has reviewer_criterion: " + r.id);
  }
});
check("has both actionable rules and (knowledge-heavy) principle facts", () => {
  assert.ok(typeCounts.checkable + typeCounts.judgment >= 8, "too few actionable rules: " + JSON.stringify(typeCounts));
  assert.ok(typeCounts.principle >= 8, "too few principle facts: " + JSON.stringify(typeCounts));
});

// ---- 3. sections_accounted covers every rule id -------------------------------------------
const accountedBlob = Object.values(m.sections_accounted).join(" ");
check("every rule id appears in sections_accounted (nothing dropped)", () => {
  const missing = [...ids].filter((id) => !new RegExp("(^|[^A-Za-z0-9-])" + id + "([^A-Za-z0-9-]|$)").test(accountedBlob));
  assert.strictEqual(missing.length, 0, "rule ids not accounted for: " + missing.join(", "));
});
check("sections_accounted references only real rule ids", () => {
  const referenced = accountedBlob.match(/gsm-[a-z0-9-]+/g) || [];
  const bad = referenced.filter((id) => !ids.has(id));
  assert.strictEqual(bad.length, 0, "unknown rule ids in sections_accounted: " + [...new Set(bad)].join(", "));
});
check("sections_accounted spans the guide (toolchain..appendix)", () => {
  const keys = Object.keys(m.sections_accounted).join(" ");
  for (const anchor of ["§0", "§1", "§2", "§3.2", "§3.3", "§3.4", "§4", "§5", "§6", "§7", "§8"]) {
    assert.ok(keys.includes(anchor), "no section accounted for anchor " + anchor);
  }
});

// ---- 4. version-pin + faithfulness --------------------------------------------------------
let totalVersionTokens = 0;
check("no rule pins/verifies against 'latest' or '*' (concrete pins only)", () => {
  for (const r of m.rules) {
    assert.ok(!WILDCARD_PIN_RE.test(r.text), "rule " + r.id + " uses a wildcard/latest pin: " + r.text);
  }
});
check("every version token in a rule is concrete (has digits)", () => {
  for (const r of m.rules) {
    const toks = r.text.match(VERSION_RE) || [];
    for (const t of toks) {
      totalVersionTokens++;
      assert.ok(/\d/.test(t), "non-numeric version token on " + r.id + ": " + t);
    }
  }
  assert.ok(totalVersionTokens >= 30, "expected many pinned versions, saw " + totalVersionTokens);
});
check("every version a rule mentions appears verbatim in the guide (manifest invents no version)", () => {
  const missing = [];
  for (const r of m.rules) {
    const toks = r.text.match(VERSION_RE) || [];
    for (const t of toks) {
      if (!guide.includes(t)) missing.push(r.id + " -> " + t);
    }
  }
  assert.strictEqual(missing.length, 0, "rule versions absent from guide: " + missing.join(", "));
});

// ---- 5. guide sanity (present, substantial, carries the anchor pins) ----------------------
check("guide is substantial and section-structured", () => {
  assert.ok(guide.length > 8000, "guide unexpectedly small: " + guide.length + " bytes");
  for (const h of ["## 1.", "## 3.", "## 4.", "## 5.", "## 6.", "## 7.", "## 8."]) {
    assert.ok(guide.includes(h), "guide missing heading " + h);
  }
  for (const w of ["server/", "cli/", "hook/", "genesis-memory.js"]) {
    assert.ok(guide.includes(w), "guide missing unit reference " + w);
  }
});
check("guide carries the load-bearing anchor pins", () => {
  for (const v of ["1.93.0", "rmcp", "2.2.0", "0.39.0", "0.37.0", "2.0.0-rc.13", "sqlite-vec", "0.2.0-beta", "2024-11-05"]) {
    assert.ok(guide.includes(v), "guide missing anchor " + v);
  }
});

// ---- 6. house rules -----------------------------------------------------------------------
check("no banned research phrase in either artifact", () => {
  // Built from fragments so this test file never contains the literal banned phrase.
  const banned = ["chain", "of", "thought"].join("-");
  assert.ok(!guide.toLowerCase().includes(banned), "guide contains the banned research phrase");
  assert.ok(!rawManifest.toLowerCase().includes(banned), "manifest contains the banned research phrase");
});
check("no obvious credential value written", () => {
  // house rule: describe presence, never a value. Guard against pasted secret shapes.
  const credRe = /(?:AKIA[0-9A-Z]{16}|ghp_[0-9A-Za-z]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|xox[baprs]-[0-9A-Za-z-]{10,})/;
  assert.ok(!credRe.test(guide), "guide contains a credential-shaped value");
  assert.ok(!credRe.test(rawManifest), "manifest contains a credential-shaped value");
});

// ---- report -------------------------------------------------------------------------------
console.log("\nrules: " + m.rules.length + "  types: " + JSON.stringify(typeCounts) + "  version-tokens: " + totalVersionTokens);
if (failures > 0) {
  console.error("\n" + failures + " check(s) FAILED.");
  process.exit(1);
}
console.log("\nAll checks passed.");
process.exit(0);
