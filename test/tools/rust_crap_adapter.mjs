#!/usr/bin/env node
// rust_crap_adapter.mjs — rust-code-analysis + cargo-llvm-cov JSON → radon-cc.json + coverage.json,
// then hand off to the UNCHANGED test/tools/crap.mjs (formula/thresholds/exit-2 reused verbatim).
//
// Faithful Node port of rust_crap_adapter.py (stdlib only).
//
// The Rust language backend for the spec-forge CRAP gate. It builds the two JSON files that
// crap.mjs already consumes, so the CRAP = CC^2*(1-cov/100)^3 + CC formula, the 30/8/4 thresholds,
// and the "exit 2 iff any function CRAP>8" block are provably identical to the crap.mjs path.
//
// Inputs (produced by /spec-crap before calling this):
//   test-results/rca/**/*.rs.json   from: rust-code-analysis-cli -m -p src/ -O json -o test-results/rca/
//   test-results/llvm-cov.json      from: cargo llvm-cov --json --release --output-path test-results/llvm-cov.json
// Outputs (consumed by crap.mjs):
//   test-results/radon-cc.json
//   test-results/coverage.json

import { readFileSync, readdirSync, writeFileSync, mkdirSync, realpathSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// Mirror Python str(PurePosixPath(p)) then .replace("\\","/").lstrip("./")
function pyPosixPath(p) {
  p = String(p);
  if (p === "") return ".";
  let i = 0;
  while (i < p.length && p[i] === "/") i++;
  let root = "";
  if (i === 1) root = "/";
  else if (i === 2) root = "//";
  else if (i >= 3) root = "/";
  const rest = p.slice(i);
  const parts = rest.split("/").filter((seg) => seg !== "" && seg !== ".");
  if (parts.length === 0) return root === "" ? "." : root;
  return root + parts.join("/");
}

function normalize(p) {
  return pyPosixPath(p).replace(/\\/g, "/").replace(/^[./]+/, "");
}

// Recursive glob of *.json under dir (equivalent to glob("<dir>/**/*.json", recursive=True)).
// Sorted for deterministic output ordering.
function globJson(dir) {
  let names;
  try {
    names = readdirSync(dir, { recursive: true });
  } catch {
    return [];
  }
  return names
    .map((n) => n.split("\\").join("/"))
    .filter((n) => n.endsWith(".json"))
    .sort()
    .map((n) => join(dir, n));
}

function loadRca(rcaDir) {
  // {file: [{type,name,classname,complexity,start,end}]}
  // rust-code-analysis-cli -o writes <rca_dir>/<inputpath>.rs.json (ext APPENDED; dir must pre-exist).
  const out = {};
  for (const jf of globJson(rcaDir)) {
    const root = JSON.parse(readFileSync(jf, "utf-8"));
    if (!root.name) continue; // root "name" is nullable — guard
    const fpath = normalize(root.name); // root == the file-level "unit" FuncSpace
    const rows = [];

    const walk = (sp, parentKind, parentName) => {
      const k = sp.kind;
      const nm = sp.name;
      // closures ALSO serialize as kind=="function" (nested in a function) — skip them.
      if (k === "function" && parentKind !== "function" && nm) {
        const isMethod = parentKind === "impl" || parentKind === "trait"; // trait default-body methods count too
        rows.push({
          type: isMethod ? "method" : "function",
          name: nm,
          classname: isMethod ? parentName : "",
          complexity: Math.trunc(Number(sp.metrics.cyclomatic.sum)), // leaf CC
          start: sp.start_line,
          end: sp.end_line,
        });
      }
      for (const c of sp.spaces || []) {
        walk(c, k, nm); // parentKind is synthesized, not a JSON field
      }
    };
    walk(root, null, null);
    out[fpath] = rows;
  }
  return out;
}

function loadLlvm(covJson) {
  // {file: [{lo,hi,cov,tot,name}]} — region coverage per llvm fn
  const per = {};
  const data = JSON.parse(readFileSync(covJson, "utf-8"));
  for (const f of data.data[0].functions) {
    let code = f.regions.filter((r) => r[7] === 0 && r[5] === 0); // Kind==CodeRegion(0) AND FileID==0
    if (code.length === 0) {
      code = f.regions.filter((r) => r[7] === 0); // fallback if every region is macro-expanded
    }
    if (code.length === 0) continue;
    const fpath = normalize(f.filenames[0]); // filenames is an array
    if (!(fpath in per)) per[fpath] = [];
    per[fpath].push({
      lo: Math.min(...code.map((r) => r[0])), // r[0]=LineStart
      hi: Math.max(...code.map((r) => r[2])), // r[2]=LineEnd
      cov: code.filter((r) => r[4] > 0).length, // r[4]=ExecutionCount
      tot: code.length,
      name: ((f.name || "").split("::").pop()), // short name for fallback join
    });
  }
  return per;
}

function emit(rca, llvm, outdir) {
  const radon = {};
  const cov = { files: {} };
  for (const [fpath, entries] of Object.entries(rca)) {
    radon[fpath] = entries.map((e) => ({
      type: e.type,
      name: e.name,
      classname: e.classname,
      complexity: e.complexity,
      rank: "A",
    }));
    // entry object -> [covered_regions, total_regions]
    const agg = new Map();
    for (const e of entries) agg.set(e, [0, 0]);
    for (const lf of llvm[fpath] || []) {
      // attribute each llvm fn to the INNERMOST rca span that contains it (routes closures→inner).
      const box = entries.filter((e) => e.start <= lf.lo && e.end >= lf.hi);
      let tgt;
      if (box.length > 0) {
        // argmin over (end - start); Python min() keeps the first on ties.
        tgt = box[0];
        let best = tgt.end - tgt.start;
        for (let i = 1; i < box.length; i++) {
          const span = box[i].end - box[i].start;
          if (span < best) {
            best = span;
            tgt = box[i];
          }
        }
      } else {
        tgt = entries.find((e) => e.name === lf.name) || null; // suffix-name fallback
      }
      if (tgt) {
        const pair = agg.get(tgt);
        pair[0] += lf.cov;
        pair[1] += lf.tot;
      }
    }
    const fns = {};
    for (const e of entries) {
      const [c, t] = agg.get(e);
      const pct = t ? (100.0 * c) / t : 0.0; // unmatched => 0% = crap.mjs's own default; trivial CC=1 → CRAP=2 (safe)
      const key = e.type === "method" ? `${e.classname}.${e.name}` : e.name;
      fns[key] = { summary: { percent_covered: pct } };
    }
    cov.files[fpath] = { functions: fns };
  }
  mkdirSync(outdir, { recursive: true });
  writeFileSync(join(outdir, "radon-cc.json"), JSON.stringify(radon));
  writeFileSync(join(outdir, "coverage.json"), JSON.stringify(cov));
}

function isMainModule() {
  try {
    return realpathSync(fileURLToPath(import.meta.url)) === realpathSync(process.argv[1]);
  } catch {
    return false;
  }
}

if (isMainModule()) {
  emit(loadRca("test-results/rca"), loadLlvm("test-results/llvm-cov.json"), "test-results");
  // reuse verbatim → propagates exit 2
  const res = spawnSync(process.execPath, ["test/tools/crap.mjs"], { stdio: "inherit" });
  process.exit(res.status === null ? 1 : res.status);
}

export { normalize, loadRca, loadLlvm, emit };
