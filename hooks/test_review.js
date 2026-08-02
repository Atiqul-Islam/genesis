#!/usr/bin/env node
/* Node unit tests for review.js — the independent semantic reviewer (§22).

   Faithful port of test_review.py. Drives review.js as a subprocess with a MOCK judge (GENESIS_REVIEWER_CMD),
   so every control path is proven deterministically without a live model: pass, fail, position-swap
   inconsistency, reviewer error, unparseable output, disabled, no-reviewer (lenient + strict), and the loop
   guard. No network. Mirrors the Python contract + case count (11 cases).

   review.js writes runtime logs under the PROJECT's .genesis/ (cwd-derived), so the log lands in the
   throwaway workspace we run it from (cwd=root) — the real project log is never touched.

   The MOCK judge is a Node script written to a temp file. It is authored with String.raw so its regexes
   and \n escapes survive verbatim into the temp file.

   Run:  node hooks/test_review.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const cp = require("child_process");

const HERE = __dirname;
const REVIEW = path.join(HERE, "review.js");

const MOCK = String.raw`"use strict";
const fs = require("fs");
function readStdin() {
  try { return fs.readFileSync(0, "utf8"); }
  catch (e) {
    if (e && e.code === "EAGAIN") {
      const buf = Buffer.alloc(65536); const chunks = [];
      for (;;) { let n; try { n = fs.readSync(0, buf, 0, buf.length, null); } catch (e2) { if (e2 && e2.code === "EAGAIN") continue; if (e2 && e2.code === "EOF") break; return ""; } if (!n) break; chunks.push(Buffer.from(buf.slice(0, n))); }
      return Buffer.concat(chunks).toString("utf8");
    }
    return "";
  }
}
const mode = process.env.GENESIS_MOCK_MODE || "pass";
const prompt = readStdin();
if (mode === "error") process.exit(1);
if (mode === "garbage") { process.stdout.write("Looks fine to me. (no JSON)"); process.exit(0); }
const m = /RULES \(criteria to check\):\n([\s\S]*?)\n\nARTIFACTS/.exec(prompt);
const ids = [];
if (m) { const re = /^- (\S+):/gm; let mm; while ((mm = re.exec(m[1])) !== null) ids.push(mm[1]); }
function verdict() {
  if (mode === "fail") return "FAIL";
  if (mode === "inconsistent") return prompt.indexOf("PASS or FAIL") !== -1 ? "PASS" : "FAIL";
  return "PASS";
}
process.stdout.write(JSON.stringify({ verdicts: ids.map(function (i) { return { id: i, verdict: verdict(), reason: "mock" }; }) }));
`;

function make_ws() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "genesis-rev-"));
  const d = path.join(root, "release-manager");
  fs.mkdirSync(d, { recursive: true });
  fs.writeFileSync(path.join(d, "CLAUDE.md"), "# Ferry — Release Manager\n## Mission\nShip correct releases.\n## Boundaries\nNever push if CI is red.\n", { encoding: "utf8" });
  return root;
}

function run(root, agent, mock_path, mode, review, no_reviewer, stop_active) {
  const env = Object.assign({}, process.env);
  delete env.GENESIS_REVIEWER_CMD;
  if (no_reviewer) {
    env.PATH = ""; // hide `claude` so reviewer_cmd() -> null
  } else {
    env.GENESIS_REVIEWER_CMD = process.execPath + " " + mock_path;
  }
  if (mode) env.GENESIS_MOCK_MODE = mode;
  if (review) env.GENESIS_REVIEW = review;
  const payload = { transcript_path: "", stop_hook_active: !!stop_active };
  // process.execPath (absolute) launches the child even when PATH is emptied; cwd=root so review.js
  // resolves its runtime .genesis/ inside THIS throwaway workspace (project-derived path, item C).
  const p = cp.spawnSync(process.execPath, [REVIEW, root, agent], {
    input: JSON.stringify(payload), encoding: "utf8", timeout: 60000, env: env, cwd: root,
  });
  const out = (p.stdout || "").trim();
  if (!out) return [false, ""];
  let d;
  try { d = JSON.parse(out); } catch (e) { return [false, out]; }
  return [d.decision === "block", d.reason || ""];
}

function main() {
  let passed = 0, failed = 0;
  function check(name, cond) {
    if (cond) { passed += 1; } else { failed += 1; }
    console.log("  " + (cond ? "PASS" : "FAIL") + "  " + name);
  }

  const mockDir = fs.mkdtempSync(path.join(os.tmpdir(), "genesis-mock-"));
  const mock = path.join(mockDir, "reviewer_mock.js");
  fs.writeFileSync(mock, MOCK, { encoding: "utf8" });
  const root = make_ws();
  const review_log = path.join(root, ".genesis", "review.log"); // cwd-derived project runtime path (item C)
  try { if (fs.statSync(review_log).isFile()) fs.rmSync(review_log); } catch (e) { /* absent */ }

  let blocked, reason;

  // 1. all PASS -> allow
  [blocked] = run(root, "method", mock, "pass");
  check("all-PASS judge allows the stop", !blocked);

  // 2. all FAIL -> block, naming a real rule
  [blocked, reason] = run(root, "method", mock, "fail");
  check("a reviewer FAIL blocks", blocked && reason.indexOf("does not embody") !== -1 && reason.indexOf("#") !== -1);

  // 3. inconsistent across position-swap -> block (fail-closed)
  [blocked, reason] = run(root, "method", mock, "inconsistent");
  check("position-swap inconsistency blocks (fail-closed)", blocked && (reason.indexOf("position-swap") !== -1 || reason.indexOf("inconsistent") !== -1));

  // 4. reviewer errors (exit 1) -> block (fail-closed)
  [blocked, reason] = run(root, "method", mock, "error");
  check("reviewer error blocks (fail-closed)", blocked && reason.toLowerCase().indexOf("cannot certify") !== -1);

  // 5. unparseable output -> block (fail-closed)
  [blocked, reason] = run(root, "method", mock, "garbage");
  check("unparseable reviewer output blocks (fail-closed)", blocked && reason.toLowerCase().indexOf("unparseable") !== -1);

  // 6. GENESIS_REVIEW=off -> allow even though the judge would FAIL everything
  [blocked] = run(root, "method", mock, "fail", "off");
  check("GENESIS_REVIEW=off disables the semantic layer", !blocked);

  // 7. no reviewer available, default mode -> allow (skip, logged)
  [blocked] = run(root, "method", mock, null, null, true);
  check("no reviewer + lenient -> allow (skip)", !blocked);

  // 8. no reviewer available, strict -> block
  [blocked, reason] = run(root, "method", mock, null, "strict", true);
  check("no reviewer + strict -> block", blocked && reason.toLowerCase().indexOf("no reviewer") !== -1);

  // 9. stop_hook_active -> allow (loop guard) even with a failing judge
  [blocked] = run(root, "method", mock, "fail", null, false, true);
  check("stop_hook_active short-circuits to allow", !blocked);

  // 10. review.log captured verdicts from the runs above
  let logged = false;
  try {
    if (fs.statSync(review_log).isFile()) {
      logged = fs.readFileSync(review_log, "utf8").split(/\r\n|\r|\n/).some((ln) => ln.indexOf('"verdict"') !== -1);
    }
  } catch (e) { logged = false; }
  check("review.log captured verdicts (under the project .genesis/)", logged);

  // 11. an agent with no required expertise -> allow (nothing to review)
  [blocked] = run(root, "", mock, "fail");
  check("no-required-expertise agent allows", !blocked);

  try { fs.rmSync(review_log, { force: true }); } catch (e) { /* best-effort */ }
  try { fs.rmSync(mockDir, { recursive: true, force: true }); } catch (e) { /* best-effort */ }
  try { fs.rmSync(root, { recursive: true, force: true }); } catch (e) { /* best-effort */ }
  console.log("\n" + passed + " passed, " + failed + " failed");
  process.exit(failed ? 1 : 0);
}

if (require.main === module) {
  main();
}
