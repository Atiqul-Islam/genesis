#!/usr/bin/env node
/* Node tests that the superpowers discipline skills are vendored self-contained into Genesis.

   Faithful port of test_vendored_skills.py. Pure filesystem inspection (no subprocess, no logs). Proves the
   vendoring is complete and dangling-reference-free:
     * ZERO `superpowers:` skill references remain anywhere under skills/.
     * Every skill the spec-forge / forge-* workflow references now exists under skills/<name>/SKILL.md.
     * The full transitive closure (11 skills) is present and every `../<sibling>` cross-skill path resolves.
     * Third-party license/attribution is preserved.
   Mirrors the Python contract + case count (17 cases).

   Run:  node hooks/test_vendored_skills.js
*/
"use strict";
const fs = require("fs");
const path = require("path");

const HERE = __dirname;
const REPO = path.dirname(HERE); // hooks/ -> plugin root
const SKILLS = path.join(REPO, "skills");

// The complete transitive closure vendored from superpowers 6.1.1.
const VENDORED = [
  "brainstorming",
  "writing-plans",
  "using-git-worktrees",
  "test-driven-development",
  "systematic-debugging",
  "verification-before-completion",
  "requesting-code-review",
  "receiving-code-review",
  "finishing-a-development-branch",
  "subagent-driven-development",
  "executing-plans",
];

// Genesis workflow skills that drove the dependency (they reference the vendored discipline skills).
const REFERENCING_DIRS = [
  "spec-forge",
  "forge-dev-agent",
  "forge-docs-agent",
  "forge-review-agent",
  "forge-spec-agent",
  "forge-verify-agent",
];

function isDir(p) { try { return fs.statSync(p).isDirectory(); } catch (e) { return false; } }
function isFile(p) { try { return fs.statSync(p).isFile(); } catch (e) { return false; } }

function skill_dirs() {
  const out = new Set();
  let entries;
  try { entries = fs.readdirSync(SKILLS, { withFileTypes: true }); } catch (e) { return out; }
  for (const e of entries) if (e.isDirectory()) out.add(e.name);
  return out;
}

function walkFiles(dir) {
  const out = [];
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch (e) { return out; }
  for (const e of entries) {
    const full = path.join(dir, e.name);
    if (e.isDirectory()) out.push.apply(out, walkFiles(full));
    else out.push(full);
  }
  return out;
}

function read(p) {
  try { return fs.readFileSync(p, "utf8"); } catch (e) { return ""; }
}

function main() {
  let passed = 0, failed = 0;
  function check(name, cond) {
    if (cond) { passed += 1; } else { failed += 1; }
    console.log("  " + (cond ? "PASS" : "FAIL") + "  " + name);
  }

  const dirs = skill_dirs();

  // ---- 1. ZERO `superpowers:` references anywhere under skills/ ----
  const offenders = [];
  for (const p of walkFiles(SKILLS)) {
    if (read(p).indexOf("superpowers:") !== -1) offenders.push(path.relative(REPO, p));
  }
  check("no `superpowers:` refs remain under skills/ (found in: " + JSON.stringify(offenders) + ")", offenders.length === 0);

  // ---- 2. Every vendored skill exists as skills/<name>/SKILL.md ----
  for (const name of VENDORED) {
    check("vendored skill present: " + name + "/SKILL.md",
          isFile(path.join(SKILLS, name, "SKILL.md")));
  }

  // ---- 3. Every skill referenced by spec-forge / forge-* exists under skills/ ----
  const vendored_set = new Set(VENDORED);
  const referenced = new Set();
  const tok = /`([a-z][a-z0-9-]+)`/g;
  for (const d of REFERENCING_DIRS) {
    const ddir = path.join(SKILLS, d);
    if (!isDir(ddir)) continue;
    for (const f of walkFiles(ddir)) {
      if (!f.endsWith(".md")) continue;
      const text = read(f);
      let m;
      tok.lastIndex = 0;
      while ((m = tok.exec(text)) !== null) {
        if (vendored_set.has(m[1])) referenced.add(m[1]);
      }
    }
  }
  check("spec-forge/forge-* reference at least the core discipline skills",
        ["test-driven-development", "verification-before-completion"].every((n) => referenced.has(n)));
  const missing_referenced = Array.from(referenced).filter((n) => !dirs.has(n)).sort();
  check("every discipline skill referenced by spec-forge/forge-* exists (missing: " + JSON.stringify(missing_referenced) + ")",
        missing_referenced.length === 0);

  // ---- 4. Transitive-closure integrity: every `../<sibling>` ref inside a vendored skill resolves ----
  const rel = /\.\.\/([A-Za-z0-9_-]+)\//g;
  const dangling = [];
  for (const name of VENDORED) {
    const ndir = path.join(SKILLS, name);
    for (const f of walkFiles(ndir)) {
      if (!(f.endsWith(".md") || f.endsWith(".sh"))) continue;
      const text = read(f);
      let m;
      rel.lastIndex = 0;
      while ((m = rel.exec(text)) !== null) {
        const target = m[1];
        if (target === "scripts" || target === "references") continue;
        const candidate = path.join(SKILLS, target);
        if (!dirs.has(target) && !fs.existsSync(candidate)) {
          if (target.indexOf("-") !== -1) dangling.push(name + ": ../" + target + "/");
        }
      }
    }
  }
  check("no dangling ../<sibling> skill refs inside vendored skills (dangling: " + JSON.stringify(dangling) + ")",
        dangling.length === 0);

  // ---- 5. License / attribution preserved ----
  const lic_path = path.join(SKILLS, "VENDORED-superpowers-LICENSE");
  const lic = isFile(lic_path) ? read(lic_path) : "";
  check("skills/VENDORED-superpowers-LICENSE exists with MIT + copyright holder",
        lic.indexOf("MIT License") !== -1 && lic.indexOf("Jesse Vincent") !== -1);
  const notice = read(path.join(REPO, "NOTICE.md"));
  check("NOTICE.md attributes superpowers (name + copyright + source URL)",
        notice.indexOf("superpowers") !== -1
        && notice.indexOf("Jesse Vincent") !== -1
        && notice.indexOf("github.com/obra/superpowers") !== -1);

  console.log("\n" + passed + " passed, " + failed + " failed");
  process.exit(failed ? 1 : 0);
}

if (require.main === module) {
  main();
}
