// Compute per-function CRAP scores and report offenders.
//
// Faithful Node port of crap.py (stdlib only). Behavior — formula, thresholds,
// table layout, summary lines, and exit codes — is identical to the Python.
//
// Reads:
//   test-results/radon-cc.json   (radon cc -s --json output)
//   test-results/coverage.json   (coverage.py JSON report, coverage.py >= 7.6)
//
// Writes a table to stdout, sorted by CRAP desc. Exits 2 if any function has
// CRAP > FAIL_THRESHOLD.
//
// Formula: CRAP(f) = CC(f)^2 * (1 - cov(f)/100)^3 + CC(f)

import { existsSync, readFileSync, realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";

const ALERT_THRESHOLD = 30.0;
const FAIL_THRESHOLD = 8.0;
const TARGET_THRESHOLD = 4.0;

const RADON_JSON = "test-results/radon-cc.json";
const COVERAGE_JSON = "test-results/coverage.json";

// ---------------------------------------------------------------------------
// Python-compatible fixed-decimal formatting (round-half-to-even on the EXACT
// double value), matching CPython's f"{x:.1f}" / "%.1f" behavior. Uses the exact
// binary value of the IEEE-754 double via BigInt so half-way cases are resolved
// against the true value, not a lossy decimal approximation.
// ---------------------------------------------------------------------------
const _fbuf = new DataView(new ArrayBuffer(8));

function pyFixed(x, dec) {
  if (Number.isNaN(x)) return "nan";
  if (!Number.isFinite(x)) return x > 0 ? "inf" : "-inf";
  const neg = x < 0 || Object.is(x, -0);
  const sign = neg ? "-" : "";
  const a = Math.abs(x);
  const TEN = 10n ** BigInt(dec);

  if (a === 0) {
    return dec > 0 ? `${sign}0.${"0".repeat(dec)}` : `${sign}0`;
  }

  // Decompose a = m * 2^e exactly.
  _fbuf.setFloat64(0, a);
  const bits = _fbuf.getBigUint64(0);
  const rawExp = Number((bits >> 52n) & 0x7ffn);
  const frac = bits & 0xfffffffffffffn;
  let m;
  let e;
  if (rawExp === 0) {
    m = frac;
    e = -1074; // subnormal
  } else {
    m = frac | (1n << 52n);
    e = rawExp - 1075;
  }

  // q = round-half-even(a * 10^dec) as an integer.
  let q;
  if (e >= 0) {
    q = m * (1n << BigInt(e)) * TEN; // exact integer; no fraction to round
  } else {
    const D = 1n << BigInt(-e);
    const N = m * TEN;
    let qf = N / D;
    const twice = (N % D) * 2n;
    if (twice > D) qf += 1n;
    else if (twice === D && qf % 2n === 1n) qf += 1n;
    q = qf;
  }

  let s = q.toString();
  if (dec === 0) return sign + s;
  if (s.length <= dec) s = "0".repeat(dec - s.length + 1) + s;
  const intPart = s.slice(0, s.length - dec);
  const fracPart = s.slice(s.length - dec);
  return `${sign}${intPart}.${fracPart}`;
}

// Python str(float): whole-valued floats render as "N.0"; others shortest round-trip.
function pyFloatStr(x) {
  if (Number.isInteger(x)) return x.toFixed(1);
  return String(x);
}

// Python str.format: '>N' pad-left, '<N' pad-right (no truncation).
function padLeft(s, w) {
  s = String(s);
  return s.length >= w ? s : " ".repeat(w - s.length) + s;
}
function padRight(s, w) {
  s = String(s);
  return s.length >= w ? s : s + " ".repeat(w - s.length);
}

// ---------------------------------------------------------------------------

export { pyFixed };

export function crap(cc, covPct) {
  const gap = 1.0 - (covPct / 100.0);
  return cc * cc * (gap ** 3) + cc;
}

export function normalize(path) {
  // Mirror Python: path.replace("\\", "/").lstrip("./")
  return path.replace(/\\/g, "/").replace(/^[./]+/, "");
}

export function coverageKey(entry) {
  if (entry.type === "method" && entry.classname) {
    return `${entry.classname}.${entry.name}`;
  }
  return entry.name;
}

export function loadCoverage(path) {
  // Return {normalized_file_path: {function_key: percent_covered}}.
  if (!existsSync(path)) return {};
  const data = JSON.parse(readFileSync(path, "utf-8"));
  const out = {};
  const files = data.files || {};
  for (const [filePath, fileData] of Object.entries(files)) {
    const funcs = {};
    const fns = (fileData && fileData.functions) || {};
    for (const [name, fn] of Object.entries(fns)) {
      const summary = (fn && fn.summary) || {};
      const pct = summary.percent_covered;
      funcs[name] = (pct === undefined || pct === null) ? 0.0 : pct;
    }
    out[normalize(filePath)] = funcs;
  }
  return out;
}

export function collectRows(radonData, coverageMap) {
  const rows = [];
  for (const [filePath, entries] of Object.entries(radonData)) {
    const norm = normalize(filePath);
    const covFuncs = coverageMap[norm] || {};
    for (const entry of entries) {
      if (entry.type === "class") continue;
      const key = coverageKey(entry);
      const covPct = (key in covFuncs) ? covFuncs[key] : 0.0;
      const cc = (entry.complexity === undefined || entry.complexity === null) ? 0 : entry.complexity;
      rows.push({
        file: norm,
        name: key,
        cc: cc,
        cov: covPct,
        crap: crap(cc, covPct),
      });
    }
  }
  // stable sort by crap descending (Array.prototype.sort is stable in Node)
  rows.sort((a, b) => b.crap - a.crap);
  return rows;
}

export function printReport(rows) {
  if (rows.length === 0) {
    console.log("CRAP Report: no functions analyzed");
    return;
  }
  console.log(
    `${padLeft("CRAP", 7)}  ${padLeft("CC", 4)}  ${padLeft("Cov%", 6)}  ${padRight("Function", 40)}  File`
  );
  console.log("-".repeat(100));
  for (const r of rows) {
    console.log(
      `${padLeft(pyFixed(r.crap, 1), 7)}  ${padLeft(String(r.cc), 4)}  ` +
      `${padLeft(pyFixed(r.cov, 1), 6)}  ${padRight(r.name, 40)}  ${r.file}`
    );
  }
}

export function main() {
  if (!existsSync(RADON_JSON)) {
    console.log(`CRAP check skipped — ${RADON_JSON} not found`);
    return 0;
  }
  const radonData = JSON.parse(readFileSync(RADON_JSON, "utf-8"));
  const coverageMap = loadCoverage(COVERAGE_JSON);
  const rows = collectRows(radonData, coverageMap);
  printReport(rows);

  const aboveTarget = rows.filter((r) => r.crap > TARGET_THRESHOLD).length;
  const aboveFail = rows.filter((r) => r.crap > FAIL_THRESHOLD).length;
  const aboveAlert = rows.filter((r) => r.crap > ALERT_THRESHOLD).length;
  console.log();
  console.log(`Summary: ${rows.length} functions analyzed`);
  console.log(`  above target (>${pyFloatStr(TARGET_THRESHOLD)}): ${aboveTarget}`);
  console.log(`  above fail   (>${pyFloatStr(FAIL_THRESHOLD)}): ${aboveFail}`);
  console.log(`  above alert  (>${pyFloatStr(ALERT_THRESHOLD)}): ${aboveAlert}`);
  return aboveFail ? 2 : 0;
}

function isMainModule() {
  try {
    return realpathSync(fileURLToPath(import.meta.url)) === realpathSync(process.argv[1]);
  } catch {
    return false;
  }
}

if (isMainModule()) {
  process.exit(main());
}
