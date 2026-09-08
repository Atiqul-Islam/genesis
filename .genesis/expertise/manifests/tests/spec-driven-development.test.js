#!/usr/bin/env node
'use strict';

// Self-contained conformance test for the spec-driven-development expertise module.
// Deps: node:assert + node:fs ONLY. Exits non-zero on any hard failure.
//
// Verifies:
//   1. the manifest parses as JSON and has the gold-standard top-level shape;
//   2. every rule has id/section/text/type; ids are unique; type is valid;
//   3. checkable -> predicate{kind in allowed set, spec}; judgment -> reviewer_criterion;
//      principle -> neither predicate nor reviewer_criterion;
//   4. sections_accounted covers every "## " / "section" header in the guide (reports gaps);
//   5. every rule.section token resolves to a guide header (traceability, hard);
//   6. every rule.text shares vocabulary with the guide (traceability, best-effort/soft).

const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const HERE = __dirname;
const MANIFEST = path.resolve(HERE, '..', 'spec-driven-development.json');
const GUIDE = path.resolve(HERE, '..', '..', 'spec-driven-development.md');

const VALID_TYPES = new Set(['checkable', 'judgment', 'principle']);
const VALID_PRED_KINDS = new Set(['structure', 'declaration', 'wiring', 'regex', 'artifact', 'command']);

const failures = [];
const warnings = [];
function check(cond, msg) { if (!cond) failures.push(msg); }

// ---------------------------------------------------------------- load files
assert.ok(fs.existsSync(MANIFEST), 'manifest missing: ' + MANIFEST);
assert.ok(fs.existsSync(GUIDE), 'guide missing: ' + GUIDE);

const rawManifest = fs.readFileSync(MANIFEST, 'utf8');
let manifest;
try {
  manifest = JSON.parse(rawManifest);
} catch (e) {
  console.error('FATAL: manifest is not valid JSON: ' + e.message);
  process.exit(1);
}
const guide = fs.readFileSync(GUIDE, 'utf8');
const guideLower = guide.toLowerCase();

// ---------------------------------------------------------------- top-level shape
for (const key of ['expertise', 'source', 'note', 'schema', 'rules', 'sections_accounted']) {
  check(Object.prototype.hasOwnProperty.call(manifest, key), 'manifest missing top-level key: ' + key);
}
check(manifest.expertise === 'spec-driven-development',
  'expertise should be "spec-driven-development", got ' + JSON.stringify(manifest.expertise));
check(Array.isArray(manifest.rules) && manifest.rules.length > 0, 'rules must be a non-empty array');
check(manifest.sections_accounted && typeof manifest.sections_accounted === 'object',
  'sections_accounted must be an object');

// ---------------------------------------------------------------- per-rule checks
const rules = Array.isArray(manifest.rules) ? manifest.rules : [];
const seenIds = new Set();
const typeCounts = { checkable: 0, judgment: 0, principle: 0 };
const SECTION_MARK = String.fromCharCode(167); // the section sign

for (const r of rules) {
  const tag = r && r.id ? r.id : JSON.stringify(r);

  check(typeof r.id === 'string' && r.id.length > 0, 'rule has no id: ' + tag);
  check(typeof r.section === 'string' && r.section.length > 0, 'rule ' + tag + ' has no section');
  check(typeof r.text === 'string' && r.text.length > 0, 'rule ' + tag + ' has no text');
  check(typeof r.type === 'string' && VALID_TYPES.has(r.type), 'rule ' + tag + ' has invalid type: ' + r.type);

  if (typeof r.id === 'string') {
    check(!seenIds.has(r.id), 'duplicate rule id: ' + r.id);
    seenIds.add(r.id);
    check(/^sdd-/.test(r.id), "rule id should be prefixed 'sdd-': " + r.id);
  }

  if (VALID_TYPES.has(r.type)) typeCounts[r.type]++;

  if (r.type === 'checkable') {
    check(r.predicate && typeof r.predicate === 'object', 'checkable rule ' + tag + ' lacks a predicate');
    if (r.predicate && typeof r.predicate === 'object') {
      check(VALID_PRED_KINDS.has(r.predicate.kind),
        'checkable rule ' + tag + ' has invalid predicate.kind: ' + r.predicate.kind);
      check(typeof r.predicate.spec === 'string' && r.predicate.spec.length > 0,
        'checkable rule ' + tag + ' predicate lacks a non-empty spec');
    }
    check(!('reviewer_criterion' in r), 'checkable rule ' + tag + ' must NOT carry a reviewer_criterion');
  } else if (r.type === 'judgment') {
    check(typeof r.reviewer_criterion === 'string' && r.reviewer_criterion.length > 0,
      'judgment rule ' + tag + ' lacks a reviewer_criterion');
    check(!('predicate' in r), 'judgment rule ' + tag + ' must NOT carry a predicate');
  } else if (r.type === 'principle') {
    check(!('predicate' in r), 'principle rule ' + tag + ' must NOT carry a predicate');
    check(!('reviewer_criterion' in r), 'principle rule ' + tag + ' must NOT carry a reviewer_criterion');
  }
}

// ---------------------------------------------------------------- header coverage
// Collect guide section headers (lines beginning with "## "), ignoring any line
// inside a fenced code block (```-delimited) — template examples contain "## " lines
// that are NOT real section headers.
const headerLines = [];
let inFence = false;
for (const line of guide.split('\n')) {
  if (/^\s*```/.test(line)) { inFence = !inFence; continue; }
  if (!inFence && /^##\s+/.test(line)) headerLines.push(line);
}
check(headerLines.length > 0, 'guide has no "## " section headers');

// Extract the section token (mark + number) from each header.
const tokenRe = new RegExp(SECTION_MARK + '\\s*\\d+');
function sectionToken(line) {
  const m = line.match(tokenRe);
  return m ? m[0].replace(/\s+/g, '') : null;
}
const headerTokens = headerLines.map(sectionToken);
headerTokens.forEach((tok, i) => {
  check(tok !== null, 'guide header has no section token: ' + headerLines[i].trim());
});

// sections_accounted keys — normalize their section tokens.
const accountedKeys = Object.keys(manifest.sections_accounted || {});
const accountedTokens = new Set(
  accountedKeys.map((k) => (k.match(tokenRe) || [''])[0].replace(/\s+/g, '')).filter(Boolean)
);

// Every guide header token must appear in sections_accounted.
const uncovered = [];
for (const tok of headerTokens) {
  if (tok && !accountedTokens.has(tok)) uncovered.push(tok);
}
check(uncovered.length === 0, 'sections_accounted does not cover guide headers: ' + uncovered.join(', '));

// Every sections_accounted entry must point to real rule ids OR be a rationale note.
const idRe = /sdd-\d+/g;
for (const [k, v] of Object.entries(manifest.sections_accounted || {})) {
  const refIds = String(v).match(idRe) || [];
  const isRationale = /rationale/i.test(String(v));
  check(refIds.length > 0 || isRationale,
    'sections_accounted["' + k + '"] names no sdd rule and is not a rationale note');
  for (const id of refIds) {
    check(seenIds.has(id), 'sections_accounted["' + k + '"] references unknown rule id: ' + id);
  }
}

// Every rule's section token must resolve to a real guide header (hard traceability).
const headerTokenSet = new Set(headerTokens.filter(Boolean));
for (const r of rules) {
  if (typeof r.section !== 'string') continue;
  const toks = (r.section.match(new RegExp(SECTION_MARK + '\\s*\\d+', 'g')) || []).map((t) => t.replace(/\s+/g, ''));
  check(toks.length > 0, 'rule ' + r.id + ' section has no section token: ' + r.section);
  for (const t of toks) {
    check(headerTokenSet.has(t), 'rule ' + r.id + ' cites section ' + t + ' with no matching guide header');
  }
}

// Every rule id should be referenced by sections_accounted (nothing orphaned).
const allAccountedIds = new Set(
  Object.values(manifest.sections_accounted || {}).flatMap((v) => String(v).match(idRe) || [])
);
for (const id of seenIds) {
  if (!allAccountedIds.has(id)) warnings.push('rule ' + id + ' is not referenced by any sections_accounted entry');
}

// ---------------------------------------------------------------- soft text traceability
// Best-effort: each rule.text should share a distinctive token (>=6 chars) with the guide.
const STOP = new Set(['should', 'always', 'never', 'before', 'change', 'system', 'without']);
for (const r of rules) {
  if (typeof r.text !== 'string') continue;
  const words = (r.text.toLowerCase().match(/[a-z][a-z-]{5,}/g) || []).filter((w) => !STOP.has(w));
  const hit = words.some((w) => guideLower.includes(w));
  if (!hit && words.length > 0) {
    warnings.push('rule ' + r.id + ': no distinctive word from its text found in the guide (weak traceability)');
  }
}

// ---------------------------------------------------------------- report
console.log('spec-driven-development manifest: ' + rules.length + ' rules ' +
  '(checkable=' + typeCounts.checkable + ', judgment=' + typeCounts.judgment + ', principle=' + typeCounts.principle + ')');
console.log('guide headers: ' + headerLines.length + '; sections_accounted entries: ' + accountedKeys.length);

if (warnings.length) {
  console.log('\n' + warnings.length + ' warning(s):');
  for (const w of warnings) console.log('  - ' + w);
}

if (failures.length) {
  console.error('\nFAIL: ' + failures.length + ' error(s):');
  for (const f of failures) console.error('  - ' + f);
  process.exit(1);
}

console.log('\nPASS: all assertions hold.');
process.exit(0);
