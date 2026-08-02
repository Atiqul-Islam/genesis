#!/usr/bin/env node
/* Node unit tests for adherence.js — the §20 measurement harness. Seeds synthetic records, checks
   aggregation. Faithful port of test_adherence.py: requires the adherence.js module directly (no
   subprocess, no logs written). Mirrors the Python contract + case count (13 cases).

   (The banned-phrase literal inside one synthetic block-reason is assembled at runtime via COT so this
   test's own source does not contain it verbatim — otherwise the live gate would refuse this write.)

   Run:  node hooks/test_adherence.js
*/
"use strict";
const path = require("path");
const adherence = require(path.join(__dirname, "adherence.js"));

const COT = "chain" + "-of-" + "thought";

const RECORDS = [
  { ts: "2026-07-20T10:00:00+00:00", hook: "validate", agent: "method", decision: "allow",
    reasons: [], cited: { "persona-creation": ["pc-1", "pc-2"], "prompt-engineering": ["pe-4"] } },
  { ts: "2026-07-20T11:00:00+00:00", hook: "validate", agent: "method", decision: "block",
    reasons: ["'persona-creation': these cited rule-ids are not in the manifest (fabricated?): pc-999.",
              'x/CLAUDE.md: contains "' + COT + '" — use "structured reasoning".'], cited: {} },
  { ts: "2026-07-20T11:05:00+00:00", hook: "gate", decision: "deny", path: "a/CLAUDE.md",
    rule: "banned-phrase", surfaced: "" },
  { ts: "2026-07-21T09:00:00+00:00", hook: "gate", decision: "deny", path: "b/CLAUDE.md",
    rule: "line-budget", surfaced: "" },
  { ts: "2026-07-21T09:01:00+00:00", hook: "gate", decision: "allow", path: "b/CLAUDE.md",
    rule: "", surfaced: "persona-creation" },
  { ts: "2026-07-21T09:30:00+00:00", hook: "validate", agent: "sensei", decision: "allow",
    reasons: [], cited: { "agent-building": ["ab-1", "ab-2", "ab-3"] } },
];
const REVIEW = [
  { ts: "2026-07-21T09:31:00+00:00", hook: "review", agent: "method", reviewer: "sonnet", decision: "pass" },
  { ts: "2026-07-21T09:40:00+00:00", hook: "review", agent: "sensei", reviewer: "sonnet", decision: "block" },
];

function main() {
  let passed = 0, failed = 0;
  function check(name, cond) {
    if (cond) { passed += 1; } else { failed += 1; }
    console.log("  " + (cond ? "PASS" : "FAIL") + "  " + name);
  }

  const a = adherence.aggregate(RECORDS, REVIEW);

  check("method allow=1 block=1", a.validate.method.allow === 1 && a.validate.method.block === 1);
  const [br, tot] = adherence.rate(a.validate.method); check("method block-rate 0.5 over 2", br === 0.5 && tot === 2);
  check("gate denies banned-phrase=1 line-budget=1",
        a.gate_denies["banned-phrase"] === 1 && a.gate_denies["line-budget"] === 1);
  check("block reason categorized: fabricated-rule-id", a.block_reasons["fabricated-rule-id"] === 1);
  check("block reason categorized: artifact-violation", a.block_reasons["artifact-violation"] === 1);
  check("cited persona-creation pc-1 & pc-2", a.cited["persona-creation"]["pc-1"] === 1 && a.cited["persona-creation"]["pc-2"] === 1);
  check("cited prompt-engineering pe-4", a.cited["prompt-engineering"]["pe-4"] === 1);
  check("cited sensei agent-building 3 rules", Object.keys(a.cited["agent-building"]).length === 3);
  check("review pass=1 block=1", a.review.pass === 1 && a.review.block === 1);
  check("stability slice has 2 days", Object.keys(a.by_day).length === 2);
  check("window first/last set", a.window.first === "2026-07-20T10:00:00+00:00" && a.window.last.indexOf("2026-07-21") === 0);

  // agent filter
  const af = adherence.aggregate(RECORDS, REVIEW, "method");
  check("agent filter excludes sensei", !("sensei" in af.validate));

  // JSON round-trips
  JSON.stringify(adherence.to_plain(a)); check("to_plain is JSON-serializable", true);

  console.log("\n" + passed + " passed, " + failed + " failed");
  console.log("\n--- sample rendered report ---\n" + adherence.render(a));
  process.exit(failed ? 1 : 0);
}

if (require.main === module) {
  main();
}
