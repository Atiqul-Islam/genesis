#!/usr/bin/env node
/* ADHERENCE harness — measure expertise application over time; stop ASSUMING it.

   Faithful Node (CommonJS, stdlib-only) port of adherence.py. Reads the JSONL runtime logs the hooks
   write (.genesis/hook-decisions.log + .genesis/review.log) and reports per agent / per rule. It
   measures; it does not enforce.

   Usage:
     node hooks/adherence.js [--log <path>] [--review <path>] [--agent <name>] [--json]
*/
"use strict";
const fs = require("fs");
const path = require("path");
const agent_ident = require("./agent_ident.js"); // sibling helper; travels with hooks/ in every install

const HOOK_DIR = __dirname;
const _RUNTIME = agent_ident.runtime_dir();
const DEFAULT_LOG = path.join(_RUNTIME, "hook-decisions.log");
const DEFAULT_REVIEW = path.join(_RUNTIME, "review.log");

// Map a validator block-reason string to a category (first match wins).
const REASON_CATS = [
  ["fabricated-rule-id", ["not in the manifest"]],
  ["below-floor-coverage", ["cite at least"]],
  ["fabricated-evidence", ["does not appear", "no such produced file", "points to"]],
  ["artifact-violation", ['"chain-of-thought"', "credential value", "lines (>"]],
  ["missing-declaration", ["did not credibly declare"]],
  ["unreadable", ["could not read"]],
];

function categorize(reason) {
  const low = String(reason).toLowerCase();
  for (const [cat, needles] of REASON_CATS) {
    if (needles.some((n) => low.includes(n.toLowerCase()))) return cat;
  }
  return "other";
}

function isFile(p) { try { return fs.statSync(p).isFile(); } catch (e) { return false; } }

function read_jsonl(p) {
  if (!p || !isFile(p)) return [];
  let text;
  try { text = fs.readFileSync(p, "utf8"); } catch (e) { return []; }
  const out = [];
  for (let line of text.split(/\r\n|\r|\n/)) {
    line = line.trim();
    if (!line) continue;
    try { out.push(JSON.parse(line)); } catch (e) { continue; }
  }
  return out;
}

function aggregate(records, review, only_agent) {
  only_agent = only_agent || null;
  const a = {
    window: { first: null, last: null },
    counts: { total: records.length },
    validate: {},      // agent -> {allow, block}
    gate_denies: {},   // rule -> n
    block_reasons: {}, // cat -> n
    cited: {},         // expertise -> {rule-id: n}
    by_day: {},        // day -> {allow, block}
    review: { pass: 0, block: 0, by_reviewer: {} },
  };
  const ensureVal = (agent) => { if (!a.validate[agent]) a.validate[agent] = { allow: 0, block: 0 }; return a.validate[agent]; };
  const ensureDay = (day) => { if (!a.by_day[day]) a.by_day[day] = { allow: 0, block: 0 }; return a.by_day[day]; };
  const ensureCited = (name) => { if (!a.cited[name]) a.cited[name] = {}; return a.cited[name]; };
  const ensureRev = (who) => { if (!a.review.by_reviewer[who]) a.review.by_reviewer[who] = { pass: 0, block: 0 }; return a.review.by_reviewer[who]; };

  for (const r of records) {
    const ts = r.ts;
    if (ts) {
      a.window.first = a.window.first === null ? ts : (ts < a.window.first ? ts : a.window.first);
      a.window.last = a.window.last === null ? ts : (ts > a.window.last ? ts : a.window.last);
    }
    const hook = r.hook;
    if (hook === "validate") {
      const agent = r.agent || "(unnamed)";
      if (only_agent && agent !== only_agent) continue;
      const dec = r.decision === "block" ? "block" : "allow";
      ensureVal(agent)[dec] += 1;
      if (ts) ensureDay(ts.slice(0, 10))[dec] += 1;
      if (dec === "block") {
        for (const reason of (r.reasons || [])) {
          const cat = categorize(reason);
          a.block_reasons[cat] = (a.block_reasons[cat] || 0) + 1;
        }
      } else {
        const citedObj = r.cited || {};
        for (const name of Object.keys(citedObj)) {
          for (const rid of citedObj[name]) {
            const c = ensureCited(name);
            c[rid] = (c[rid] || 0) + 1;
          }
        }
      }
    } else if (hook === "gate") {
      if (r.decision === "deny") {
        const rule = r.rule || "(unknown)";
        a.gate_denies[rule] = (a.gate_denies[rule] || 0) + 1;
      }
    }
  }
  for (const r of review) {
    const ag = (r.agent === undefined) ? null : r.agent;
    if (only_agent && ag !== null && ag !== only_agent) continue;
    const v = (r.decision === "block" || r.verdict === "block" || r.verdict === "fail") ? "block" : "pass";
    a.review[v] += 1;
    ensureRev(r.reviewer || "(reviewer)")[v] += 1;
  }
  return a;
}

function rate(d) {
  const tot = d.allow + d.block;
  return [tot ? d.block / tot : 0.0, tot];
}

// ---- Python-style format helpers ----
function padRight(s, w) { s = String(s); return s.length >= w ? s : s + " ".repeat(w - s.length); }
function padLeft(s, w) { s = String(s); return s.length >= w ? s : " ".repeat(w - s.length) + s; }
function pct(br) { return padLeft((br * 100).toFixed(1) + "%", 5); } // Python {:5.1%}
function pyRound(x) { // round-half-to-even, matching Python round()
  const f = Math.floor(x), diff = x - f;
  if (diff < 0.5) return f;
  if (diff > 0.5) return f + 1;
  return (f % 2 === 0) ? f : f + 1;
}
function mostCommon(obj) { return Object.entries(obj).sort((x, y) => y[1] - x[1]); } // stable: ties keep insertion order

function render(a) {
  const L = [];
  const w = a.window;
  L.push("Genesis expertise-adherence report");
  L.push("  window: " + (w.first || "—") + " → " + (w.last || "—") + "   records: " + a.counts.total);

  L.push("\nTurn completion (validate Stop hook) — block-rate = how often finishing was refused:");
  if (Object.keys(a.validate).length === 0) L.push("  (no validate records yet)");
  for (const agent of Object.keys(a.validate).sort()) {
    const [br, tot] = rate(a.validate[agent]);
    L.push("  " + padRight(agent, 16) + " stops=" + padRight(tot, 4) + " blocked=" + padRight(a.validate[agent].block, 4) + " " +
           "clean=" + padRight(a.validate[agent].allow, 4) + " block-rate=" + pct(br));
  }

  L.push("\nDeterministic violations (bucket-a):");
  if (Object.keys(a.gate_denies).length) {
    L.push("  gate PreToolUse denies by rule:");
    for (const [rule, n] of mostCommon(a.gate_denies)) L.push("    " + padRight(rule, 16) + " " + n);
  }
  if (Object.keys(a.block_reasons).length) {
    L.push("  validate Stop block reasons by category:");
    for (const [cat, n] of mostCommon(a.block_reasons)) L.push("    " + padRight(cat, 22) + " " + n);
  }
  if (Object.keys(a.gate_denies).length === 0 && Object.keys(a.block_reasons).length === 0) L.push("  (none recorded)");

  L.push("\nPer-rule application rate (cited-with-evidence on a clean finish), per expertise:");
  if (Object.keys(a.cited).length === 0) L.push("  (no clean finishes with citations yet)");
  for (const name of Object.keys(a.cited).sort()) {
    const items = mostCommon(a.cited[name]);
    const shown = items.slice(0, 12).map(([rid, n]) => rid + "×" + n).join(", ");
    L.push("  " + name + ": " + items.length + " distinct rules cited — " + shown + (items.length > 12 ? " …" : ""));
  }

  const rv = a.review;
  if (rv.pass || rv.block) {
    const tot = rv.pass + rv.block;
    L.push("\nSemantic review (bucket-b, independent LLM-reviewer): " + rv.pass + "/" + tot + " passed, block-rate=" + pct(rv.block / tot));
    for (const who of Object.keys(rv.by_reviewer).sort()) {
      const d = rv.by_reviewer[who];
      const t = d.pass + d.block;
      L.push("  " + padRight(who, 16) + " pass=" + d.pass + " block=" + d.block + " block-rate=" + pct(t ? d.block / t : 0));
    }
  } else {
    L.push("\nSemantic review (bucket-b): no review.log yet — run the LLM-reviewer (§22) to populate.");
  }

  L.push("\nStability slice — validate block-rate by day (watch for an upward drift):");
  if (Object.keys(a.by_day).length === 0) L.push("  (no dated records)");
  for (const day of Object.keys(a.by_day).sort()) {
    const [br, tot] = rate(a.by_day[day]);
    const bar = "█".repeat(pyRound(br * 20));
    L.push("  " + day + "  n=" + padRight(tot, 4) + " block-rate=" + pct(br) + " " + bar);
  }
  return L.join("\n");
}

function to_plain(a) {
  const cp = {};
  cp.window = a.window;
  cp.counts = a.counts;
  cp.validate = {}; for (const k of Object.keys(a.validate)) cp.validate[k] = Object.assign({}, a.validate[k]);
  cp.gate_denies = Object.assign({}, a.gate_denies);
  cp.block_reasons = Object.assign({}, a.block_reasons);
  cp.cited = {}; for (const k of Object.keys(a.cited)) cp.cited[k] = Object.assign({}, a.cited[k]);
  cp.by_day = {}; for (const k of Object.keys(a.by_day)) cp.by_day[k] = Object.assign({}, a.by_day[k]);
  cp.review = { pass: a.review.pass, block: a.review.block, by_reviewer: {} };
  for (const k of Object.keys(a.review.by_reviewer)) cp.review.by_reviewer[k] = Object.assign({}, a.review.by_reviewer[k]);
  return cp;
}

function parseArgs(argv) {
  const args = { log: DEFAULT_LOG, review: DEFAULT_REVIEW, agent: null, json: false };
  for (let i = 0; i < argv.length; i++) {
    let a = argv[i];
    let inlineVal = null;
    if (a.startsWith("--") && a.indexOf("=") !== -1) {
      const eq = a.indexOf("=");
      inlineVal = a.slice(eq + 1);
      a = a.slice(0, eq);
    }
    const takeVal = () => (inlineVal !== null ? inlineVal : argv[++i]);
    if (a === "--log") args.log = takeVal();
    else if (a === "--review") args.review = takeVal();
    else if (a === "--agent") args.agent = takeVal();
    else if (a === "--json") args.json = true;
    // unknown args are ignored (argparse would error; harness stays lenient)
  }
  return args;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const records = read_jsonl(args.log);
  const review = read_jsonl(args.review);
  if (!records.length && !review.length) {
    process.stdout.write("No adherence records found at " + args.log + ".\n" +
      "Run some agent turns first — the gate/validate hooks populate it.\n");
    return;
  }
  const a = aggregate(records, review, args.agent);
  process.stdout.write((args.json ? JSON.stringify(to_plain(a), null, 2) : render(a)) + "\n");
}

if (require.main === module) {
  main();
}

module.exports = { categorize, read_jsonl, aggregate, rate, render, to_plain, REASON_CATS };
