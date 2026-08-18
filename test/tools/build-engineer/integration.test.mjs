#!/usr/bin/env node
// integration.test.mjs
//
// Mode-detection + no-drift integration test for the build-engineer recipe.
// Node-native: node:assert + node:fs only. Exits non-zero on any failure.

import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { detectMode, scaffoldSummary } from './detect_mode.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REAL_REPO = path.join(__dirname, 'fixtures', 'real-repo');
const EMPTY_FOLDER = path.join(__dirname, 'fixtures', 'empty-folder');

let pass = 0;
let fail = 0;
const failures = [];

function check(name, fn) {
  try {
    fn();
    pass += 1;
    console.log(`PASS ${name}`);
  } catch (err) {
    fail += 1;
    failures.push(`${name}: ${err.message}`);
    console.log(`FAIL ${name}: ${err.message}`);
  }
}

check('fixtures/real-repo exists and is a directory', () => {
  assert.ok(fs.existsSync(REAL_REPO), `missing ${REAL_REPO}`);
  assert.ok(fs.statSync(REAL_REPO).isDirectory(), `${REAL_REPO} is not a directory`);
});

check('fixtures/empty-folder exists and is a directory', () => {
  assert.ok(fs.existsSync(EMPTY_FOLDER), `missing ${EMPTY_FOLDER}`);
  assert.ok(fs.statSync(EMPTY_FOLDER).isDirectory(), `${EMPTY_FOLDER} is not a directory`);
});

check("detectMode(fixtures/real-repo) === 'deep-read'", () => {
  assert.strictEqual(detectMode(REAL_REPO), 'deep-read');
});

check("detectMode(fixtures/empty-folder) === 'grill'", () => {
  assert.strictEqual(detectMode(EMPTY_FOLDER), 'grill');
});

check('detectMode is deterministic across repeat calls (real-repo, no-drift)', () => {
  const first = detectMode(REAL_REPO);
  const second = detectMode(REAL_REPO);
  const third = detectMode(REAL_REPO);
  assert.strictEqual(first, second);
  assert.strictEqual(second, third);
  assert.strictEqual(first, 'deep-read');
});

check('detectMode is deterministic across repeat calls (empty-folder, no-drift)', () => {
  const first = detectMode(EMPTY_FOLDER);
  const second = detectMode(EMPTY_FOLDER);
  const third = detectMode(EMPTY_FOLDER);
  assert.strictEqual(first, second);
  assert.strictEqual(second, third);
  assert.strictEqual(first, 'grill');
});

check('scaffoldSummary is byte-identical across repeat calls (real-repo, no-drift)', () => {
  const a = scaffoldSummary(REAL_REPO);
  const b = scaffoldSummary(REAL_REPO);
  assert.strictEqual(a, b, 'scaffoldSummary output drifted across identical calls');
});

check('scaffoldSummary is byte-identical across repeat calls (empty-folder, no-drift)', () => {
  const a = scaffoldSummary(EMPTY_FOLDER);
  const b = scaffoldSummary(EMPTY_FOLDER);
  assert.strictEqual(a, b, 'scaffoldSummary output drifted across identical calls');
});

console.log('---');
console.log(`${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.error('FAILURES:');
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}
process.exit(0);
