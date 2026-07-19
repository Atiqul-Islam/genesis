#!/usr/bin/env python3
# rust_crap_adapter.py — rust-code-analysis + cargo-llvm-cov JSON → radon-cc.json + coverage.json,
# then hand off to the UNCHANGED test/tools/crap.py (formula/thresholds/exit-2 reused verbatim).
#
# The Rust language backend for the spec-forge CRAP gate. It builds the two JSON files that
# crap.py already consumes, so the CRAP = CC^2*(1-cov/100)^3 + CC formula, the 30/8/4 thresholds,
# and the "exit 2 iff any function CRAP>8" block are provably identical to the Python path.
#
# Inputs (produced by /spec-crap before calling this):
#   test-results/rca/**/*.rs.json   from: rust-code-analysis-cli -m -p src/ -O json -o test-results/rca/
#   test-results/llvm-cov.json      from: cargo llvm-cov --json --release --output-path test-results/llvm-cov.json
# Outputs (consumed by crap.py):
#   test-results/radon-cc.json
#   test-results/coverage.json
import json, glob, subprocess, sys
from pathlib import Path


def normalize(p):
    return str(Path(p)).replace("\\", "/").lstrip("./")


def load_rca(rca_dir):                      # {file: [{type,name,classname,complexity,start,end}]}
    # rust-code-analysis-cli -o writes <rca_dir>/<inputpath>.rs.json (ext APPENDED; dir must pre-exist). [V H1]
    out = {}
    for jf in glob.glob(f"{rca_dir}/**/*.json", recursive=True):
        root = json.load(open(jf))
        if not root.get("name"):
            continue                                                   # root "name" is nullable — guard
        fpath = normalize(root["name"]); rows = []                     # root == the file-level "unit" FuncSpace

        def walk(sp, parent_kind, parent_name):
            k, nm = sp.get("kind"), sp.get("name")
            # closures ALSO serialize as kind=="function" (nested in a function) — skip them. [V H1]
            if k == "function" and parent_kind != "function" and nm:
                is_method = parent_kind in ("impl", "trait")           # trait default-body methods count too
                rows.append({"type": "method" if is_method else "function",
                             "name": nm, "classname": parent_name if is_method else "",
                             "complexity": int(sp["metrics"]["cyclomatic"]["sum"]),  # [V] leaf CC
                             "start": sp["start_line"], "end": sp["end_line"]})
            for c in sp.get("spaces", []):
                walk(c, k, nm)                                         # parent_kind is synthesized, not a JSON field
        walk(root, None, None); out[fpath] = rows
    return out


def load_llvm(cov_json):                    # {file: [{lo,hi,cov,tot,name}]} — region coverage per llvm fn
    per = {}
    for f in json.load(open(cov_json))["data"][0]["functions"]:
        code = [r for r in f["regions"] if r[7] == 0 and r[5] == 0]   # Kind==CodeRegion(0) AND FileID==0 [V/H1]
        if not code:
            code = [r for r in f["regions"] if r[7] == 0]             # fallback if every region is macro-expanded
        if not code:
            continue
        fpath = normalize(f["filenames"][0])                          # filenames is an array
        per.setdefault(fpath, []).append({
            "lo": min(r[0] for r in code), "hi": max(r[2] for r in code),   # r[0]=LineStart, r[2]=LineEnd
            "cov": sum(1 for r in code if r[4] > 0), "tot": len(code),      # r[4]=ExecutionCount
            "name": (f.get("name") or "").split("::")[-1]})                 # short name for fallback join
    return per


def emit(rca, llvm, outdir):
    radon, cov = {}, {"files": {}}
    for fpath, entries in rca.items():
        radon[fpath] = [{"type": e["type"], "name": e["name"], "classname": e["classname"],
                         "complexity": e["complexity"], "rank": "A"} for e in entries]
        agg = {id(e): [0, 0] for e in entries}                        # entry -> [covered_regions, total_regions]
        for lf in llvm.get(fpath, []):
            # attribute each llvm fn to the INNERMOST rca span that contains it (routes closures→inner). [H1]
            box = [e for e in entries if e["start"] <= lf["lo"] and e["end"] >= lf["hi"]]
            tgt = min(box, key=lambda e: e["end"] - e["start"]) if box \
                else next((e for e in entries if e["name"] == lf["name"]), None)  # suffix-name fallback
            if tgt:
                agg[id(tgt)][0] += lf["cov"]; agg[id(tgt)][1] += lf["tot"]
        fns = {}
        for e in entries:
            c, t = agg[id(e)]
            pct = (100.0 * c / t) if t else 0.0     # unmatched => 0% = crap.py's own default; trivial CC=1 → CRAP=2 (safe)
            key = f'{e["classname"]}.{e["name"]}' if e["type"] == "method" else e["name"]
            fns[key] = {"summary": {"percent_covered": pct}}
        cov["files"][fpath] = {"functions": fns}
    Path(outdir).mkdir(parents=True, exist_ok=True)
    json.dump(radon, open(f"{outdir}/radon-cc.json", "w"))
    json.dump(cov,   open(f"{outdir}/coverage.json", "w"))


if __name__ == "__main__":
    emit(load_rca("test-results/rca"), load_llvm("test-results/llvm-cov.json"), "test-results")
    sys.exit(subprocess.call([sys.executable, "test/tools/crap.py"]))   # reuse verbatim → propagates exit 2
