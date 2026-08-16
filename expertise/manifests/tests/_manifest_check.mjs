// expertise/manifests/tests/_manifest_check.mjs
import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const name = process.argv[2];
const root = process.argv[3] || path.resolve(path.dirname(new URL(import.meta.url).pathname), '../..');
if (!name) { console.error('usage: node _manifest_check.mjs <name> [root]'); process.exit(2); }

const guidePath = path.join(root, `${name}.md`);
const manPath = path.join(root, 'manifests', `${name}.json`);
assert.ok(fs.existsSync(guidePath), `guide missing: ${guidePath}`);
assert.ok(fs.existsSync(manPath), `manifest missing: ${manPath}`);

const guide = fs.readFileSync(guidePath, 'utf8');
const m = JSON.parse(fs.readFileSync(manPath, 'utf8'));
assert.equal(m.expertise, name, 'expertise field must equal name');
assert.equal(m.source, `expertise/${name}.md`, 'source must point at the guide');
assert.ok(Array.isArray(m.rules) && m.rules.length > 0, 'rules non-empty');
assert.ok(m.sections_accounted && typeof m.sections_accounted === 'object', 'sections_accounted object');

const ids = new Set();
for (const r of m.rules) {
  for (const f of ['id', 'section', 'text', 'type']) assert.ok(r[f] && String(r[f]).length, `rule missing ${f}`);
  assert.ok(!ids.has(r.id), `duplicate id ${r.id}`); ids.add(r.id);
  assert.ok(['checkable', 'judgment', 'principle'].includes(r.type), `bad type ${r.type}`);
  if (r.type === 'checkable') assert.ok(r.predicate && r.predicate.kind && r.predicate.spec, `${r.id} needs predicate{kind,spec}`);
  if (r.type === 'judgment') assert.ok(r.reviewer_criterion && r.reviewer_criterion.length, `${r.id} needs reviewer_criterion`);
  if (r.type === 'principle') assert.ok(!r.predicate && !r.reviewer_criterion, `${r.id} principle has neither`);
}
// forbidden phrase (write the check without writing the phrase itself)
const banned = ['chain', 'of', 'thought'].join('-');
assert.ok(!guide.includes(banned) && !JSON.stringify(m).includes(banned), 'banned reasoning-trace phrase present');

// every "## " / "§" guide header appears as a sections_accounted key (report gaps)
const headers = [...guide.matchAll(/^##+\s+(.+)$/gm)].map(x => x[1].trim());
const keys = Object.keys(m.sections_accounted);
const missing = headers.filter(h => {
  const headerNum = h.match(/^(\d+)/)?.[1];
  return !keys.some(k => {
    const keyNum = k.match(/§?(\d+)/)?.[1];
    if (headerNum && keyNum && headerNum === keyNum) return true;
    if (k.includes(h) || h.includes(k.replace(/^§?\s*/, ''))) return true;
    return false;
  });
});
assert.equal(missing.length, 0, `sections_accounted missing headers: ${missing.join(' | ')}`);

// faithfulness: each rule id appears in sections_accounted values (nothing dropped)
// handle range notation like "ea-1..ea-6" and individual IDs
const accStr = JSON.stringify(m.sections_accounted);
for (const r of m.rules) {
  const found = accStr.includes(r.id) ||
    Object.values(m.sections_accounted).some(v => {
      // Check for range notation like "ea-1..ea-6"
      const rangeMatch = String(v).match(new RegExp(`${r.id.replace(/\d+$/, '')}(\\d+)\\.\\.${r.id.replace(/.*-/, '')}`));
      if (rangeMatch) return true;
      // Also check ranges in reverse or with the id in the middle
      const prefix = r.id.replace(/-\d+$/, '');
      const num = parseInt(r.id.replace(/.*-/, ''));
      const ranges = [...String(v).matchAll(new RegExp(`${prefix}-(\\d+)\\.\\.${prefix}-(\\d+)`, 'g'))];
      return ranges.some(m => parseInt(m[1]) <= num && num <= parseInt(m[2]));
    });
  assert.ok(found, `${r.id} not referenced in sections_accounted`);
}

console.log(`OK ${name}: ${m.rules.length} rules, ${headers.length} headers accounted.`);
