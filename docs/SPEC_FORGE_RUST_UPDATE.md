# Updating spec-forge to build the Genesis Rust MCP memory server

**Implementation-ready update plan.** Written 2026-07-19. Author: Genesis research team (1 orchestrator + 5 researchers + 2 adversarial verifiers, ~659k tokens).

## Scope & method

Design the smallest faithful change set that lets the **`/spec-forge`** workflow build the **Genesis Rust MCP memory server** (`~/Downloads/genesis/server/`) end-to-end while satisfying **every** phase and gate — nothing skipped, nothing weakened. The update lives on a **copy** inside `~/Downloads/genesis/.claude/skills/` (Atiqul's own repo). The spec-forge sources were read **read-only**; no private repo was modified, no credential reproduced.

**Sources.** Primary, read this run: the spec-forge skills (`/mnt/c/Users/iatiq/Documents/temp/spec-driven-template/.claude/skills/…` + `/docs/…` + `test/tools/crap.py`); the ifs Rust skills (`/mnt/c/Users/iatiq/Documents/temp/ifs-fiber-insight-repository/.claude/skills/new-model` + `component-template-rust.md` + `scripts/run-tests.sh` + `ifs-supervisor{,-lean}`, `modify-model-forge`, real `denier-rust`/`outlier-rust` crates); crates.io API + docs.rs + upstream repos for rmcp/rusqlite/sqlite-vec/ort/tokenizers/cargo-llvm-cov/rust-code-analysis; LLVM + MCP + ONNX-Runtime docs. Every load-bearing fact is tagged **[V]** (verified from a cited primary source this run) or **[I]** (inferred; in §6 human-verify list).

**The thesis.** spec-forge's supervisor, routing table, handoff schema, hallucination audit, run-state/resume, checkpoint, and every `superpowers:*` skill are **language-agnostic** and copy **verbatim**. Only three narrow seams are Python-shaped and must be swapped for Rust: **(1) test execution**, **(2) the CRAP/quality gate**, **(3) scaffold + compile-to-stubs**. Language-branching is already an existing pattern — `verify-agent` already branches playwright-bdd vs pytest-bdd, and `new-model` already does `--lang` detection — so this is *extending an established mechanism*, not inventing one.

---

# 0. The spec-forge step/gate checklist the update MUST satisfy

`/spec-forge` = supervisor + **5 persistent silent** `forge-*` agents, **12 phases**, deterministic table routing (never LLM judgment). `/spec-forge` is a superset of `/spec-build` (adds Phase 0a Worktree + Phase 2.5 Plan + superpowers discipline). All [V] from the cited skills.

## 0.1 Ordered phases (owner → exit/gate)

| # | Phase | Owner | Exit / gate | Citation |
|---|---|---|---|---|
| 0 | Initiation | Supervisor | run dir `.planning/builds/<run_id>/`; `timeline.html` init; `state.json` (`mode:"spec-forge"`) | `spec-forge/SKILL.md:44-50` |
| 0a | **Worktree isolation** | Supervisor→`superpowers:using-git-worktrees` | worktree created → `state.json.worktree_path` | `:52-60` |
| 0b | Spawn agents | Supervisor | 5 `forge-*` spawned in parallel via `Task` (`general-purpose`), each `{status:ready}`, persist whole run | `:62-64` |
| 1 | Spec discovery (+ audit loop) | forge-spec-agent | `test/specs/<slug>.md` with **zero audit markers AND explicit user "Approve"**; optional `superpowers:brainstorming` | `:66-82` |
| 2 | Compile → Gherkin + stubs | forge-spec-agent | `test/features/<slug>.feature` + step defs + unit stubs; spec-structure gate | `:84-86` |
| 2.5 | **Implementation plan** | Supervisor→`superpowers:writing-plans` | `docs/superpowers/plans/<date>-<slug>.md` → `state.json.artifacts.plan_path` | `:88-101` |
| 3 | RED check | forge-verify-agent | **ALL tests fail** (healthy RED); premature pass → `regenerate_stubs` | `:103-105` |
| 4 | TDD inner loop | forge-dev + forge-verify | per plan task GREEN; **1 commit/task**; `test-driven-development`+`verification-before-completion`; iter≥3→`systematic-debugging` | `:107-118` |
| 5 | GREEN (full BDD+unit) | forge-verify-agent | all scenarios + all unit pass | `:120-122` |
| 6 | Simplify | forge-dev-agent | `simplify` skill on impl only; re-verify | `:124-126` |
| 7 | Verify post-simplify | forge-verify-agent | behavior preserved | `:128-130` |
| 8 | Regression | forge-verify-agent | **entire suite** passes | `:132-134` |
| 9 | Review | forge-review-agent | `/spec-crap` (**CRAP>8 fails**) + **local** `superpowers:requesting-code-review`; block if CRAP>8 OR any Critical/Important | `:136-148` |
| 10 | Docs | forge-docs-agent | `docs/architecture.md` synced; optional `/revise-claude-md` | `:150-152` |
| 11 | Finalization | Supervisor→`superpowers:finishing-a-development-branch` | run summary; terminate 5 agents; `state.current_phase=complete` | `:154-160` |

Double loop: **outer** BDD (phases 3,5,7,8), **inner** unit TDD (phase 4, red→green→refactor).

## 0.2 The gate set (all must survive — one-line master checklist)

1. 12 phases in order `0→0a→0b→1→2→2.5→3→4→5→6→7→8→9→10→11`, **no skipping**.
2. **Hallucination audit** blocks Phase 1 until **zero markers + explicit "Approve"** (6 marker types: invented_identifier, number_without_origin, implementation_specific, unconfirmed_edge_case, external_dependency, compound_requirement). `spec-build/SKILL.md:160-187`
3. **Spec-structure gate** blocks compile on non-UI-observable "Expected Behavior" / orphan criteria. `spec-compile/SKILL.md:49-52`
4. **RED-before-GREEN**: all tests fail first; premature pass → `regenerate_stubs`. `verify-agent/SKILL.md:33,108,114`
5. **No-mock BDD**: outer-loop runs against the real system (health-poll ≤10×2s); only unit tests may mock. `spec-driven-development.md:51,128,440,495`
6. **TDD Three Laws** + **one-commit-per-task**; iter≥3 → `systematic-debugging`; 3+ failed fixes → escalate. `forge-dev-agent/SKILL.md:20,23,142`; `routing-table.md:22-24,68-75`
7. **verification-before-completion** before every dev PASS (audited via `skills_invoked`). `forge-dev-agent/SKILL.md:21`; `handoff-schema.md:123`
8. **CRAP > 8 blocks** (exit 2); never lower the threshold. `spec-crap/SKILL.md:45,99,132`; `crap.py:21,106`
9. **Review-blocking**: CRAP>8 OR any Critical/Important local-subagent finding → back to dev. `forge-review-agent/SKILL.md:104`
10. **Worktree isolation** (0a) + **finishing-a-development-branch** (11). `spec-forge/SKILL.md:30,157`
11. **Deterministic table routing** on `(phase, status, next_responsibility[, iter])`; unmatched triple = ERROR→escalate. `routing-table.md:77-82`
12. **Run-state/resume**: `.planning/builds/<run-id>/{state.json,timeline.html,CHECKPOINT.md,<agent>-checkpoint.json}`; resume re-spawns all 5. `multi-agent-workflow.md:88-98`
13. **Strict handoff JSON** directive/verdict contract (+ `skills_invoked` for forge). `handoff-schema.md`
14. **Log-validation gate** inside every verify run: never GREEN if the log validator returns ISSUES. `verify-agent/SKILL.md:28,132`
15. 5 persistent silent specialists; **supervisor is the only user-facing voice**.

**Language-agnostic (transfers unchanged — do NOT rewrite):** phase order; all gate *semantics*; routing-table keys; handoff JSON schema (incl. `crap_report{file,function,cc,coverage,crap}` — numeric, language-neutral); `.planning/builds/` layout; checkpoint triggers; the 5-agent topology; every `superpowers:*` invocation. (`sfr-A-gates.md §4G` [V])

---

# 1. Rust equivalent for each Python-specific gate (CRAP is the crux)

The Python surface is narrow and localised: `pytest`/`pytest-bdd`/`playwright-bdd`, `radon`+`coverage.py`+`crap.py`, `ruff`/`black`, and Python stub conventions. Gate *semantics* (thresholds 30/8/4, phase order, routing, handoff) are untouched.

## 1.1 The CRAP gate — reuse `crap.py` UNCHANGED behind a Rust adapter (verified)

`crap.py` already computes `CRAP = CC²·(1−cov/100)³ + CC`, applies FAIL=8/ALERT=30/TARGET=4, sorts desc, and **exits 2 iff any function CRAP>8** (`crap.py:21,29,106` [V]). Re-implementing that in Rust re-opens formula verification for no benefit. **Lowest-risk path: keep `crap.py` byte-for-byte and write a thin adapter that produces the two JSON files it already consumes.** The formula, thresholds, and exit-2 block are then provably identical. (`sfr-B` design, `sfr-VB` 4/4 CONFIRMED.)

**CC source — `rust-code-analysis-cli` (Mozilla).** Per-function cyclomatic complexity via tree-sitter. Output is a tree of **spaces** (`kind ∈ {unit, function, impl, closure, …}`), each with `name`, `start_line`, `end_line`, `metrics`. **The leaf-function CC integer is `metrics.cyclomatic.sum`** — confirmed from source (`src/metrics/cyclomatic.rs` `Serialize` emits `sum, average, min, max`; for a leaf, `sum == CC`). [V, VB]
Command: `mkdir -p test-results/rca && rust-code-analysis-cli -m -p src/ -O json -o test-results/rca/` — **the `-o` dir must pre-exist** (else the CLI exits 1) and files land as `<dir>/<path>.rs.json` (extension *appended*, input tree mirrored). Alternatively drop `-o` and read the compact root JSON from stdout per file. [V H1: `rust-code-analysis-cli/src/{main,formats}.rs`]

**Coverage source — `cargo-llvm-cov` (`--json`).** **Critical gotcha (verified):** the per-function object has `name, count, regions, filenames, branches, mcdc_records` but **NO `percent`** — `percent` exists only in the per-file/totals `renderSummary`. You **compute** per-function region coverage from `data[0].functions[].regions`, where each region tuple is `[LineStart, ColStart, LineEnd, ColEnd, ExecutionCount, FileID, ExpandedFileID, Kind]` → **index 4 = exec count, index 7 = kind, `CodeRegion == 0`** (all confirmed from LLVM `CoverageExporterJson.cpp` + `CoverageMapping.h`). [V, VB]
Command: `cargo llvm-cov --json --release --output-path test-results/llvm-cov.json`.
Per-function `percent_covered = 100 · (#code regions with exec_count>0) / (#code regions)`; a function with `count==0` → 0% → CRAP≈CC²+CC (correct max-risk, mirrors coverage.py).

**The JOIN — by file path + line-range, NOT by name.** llvm-cov names are fully-qualified/monomorphized (`crate::mod::Type::method`); rust-code-analysis names are short (`method`). String-equating them silently mis-joins. The adapter joins each rca function's `[start_line, end_line]` against each llvm function's derived start line (`min` region `LineStart`), aggregating all monomorphized instantiations that fall in range, then writes `coverage.json` keyed by the rca short name (`classname.name` for impl methods, else `name`) so `crap.py`'s lookup succeeds unchanged. [V approach, VB CONFIRMED]

**Adapter (`test/tools/rust_crap_adapter.py`) — produces `radon-cc.json` + `coverage.json`, then calls the unchanged `crap.py`:**

```python
#!/usr/bin/env python3
# rust_crap_adapter.py — rust-code-analysis + cargo-llvm-cov JSON → radon-cc.json + coverage.json,
# then hand off to the UNCHANGED test/tools/crap.py (formula/thresholds/exit-2 reused verbatim).
import json, glob, subprocess, sys
from pathlib import Path

def normalize(p): return str(Path(p)).replace("\\", "/").lstrip("./")

def load_rca(rca_dir):                      # {file: [{type,name,classname,complexity,start,end}]}
    # rust-code-analysis-cli -o writes <rca_dir>/<inputpath>.rs.json (ext APPENDED; dir must pre-exist). [V H1]
    out = {}
    for jf in glob.glob(f"{rca_dir}/**/*.json", recursive=True):
        root = json.load(open(jf))
        if not root.get("name"): continue                          # root "name" is nullable — guard
        fpath = normalize(root["name"]); rows = []                 # root == the file-level "unit" FuncSpace
        def walk(sp, parent_kind, parent_name):
            k, nm = sp.get("kind"), sp.get("name")
            # closures ALSO serialize as kind=="function" (nested in a function) — skip them. [V H1]
            if k == "function" and parent_kind != "function" and nm:
                is_method = parent_kind in ("impl", "trait")       # trait default-body methods count too
                rows.append({"type": "method" if is_method else "function",
                             "name": nm, "classname": parent_name if is_method else "",
                             "complexity": int(sp["metrics"]["cyclomatic"]["sum"]),  # [V] leaf CC
                             "start": sp["start_line"], "end": sp["end_line"]})
            for c in sp.get("spaces", []): walk(c, k, nm)          # parent_kind is synthesized, not a JSON field
        walk(root, None, None); out[fpath] = rows
    return out

def load_llvm(cov_json):                    # {file: [{lo,hi,cov,tot,name}]} — region coverage per llvm fn
    per = {}
    for f in json.load(open(cov_json))["data"][0]["functions"]:
        code = [r for r in f["regions"] if r[7] == 0 and r[5] == 0]  # Kind==CodeRegion(0) AND FileID==0 [V/H1]
        if not code: code = [r for r in f["regions"] if r[7] == 0]   # fallback if every region is macro-expanded
        if not code: continue
        fpath = normalize(f["filenames"][0])                        # filenames is an array
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
        agg = {id(e): [0, 0] for e in entries}                       # entry -> [covered_regions, total_regions]
        for lf in llvm.get(fpath, []):
            # attribute each llvm fn to the INNERMOST rca span that contains it (routes closures→inner). [H1]
            box = [e for e in entries if e["start"] <= lf["lo"] and e["end"] >= lf["hi"]]
            tgt = min(box, key=lambda e: e["end"] - e["start"]) if box \
                  else next((e for e in entries if e["name"] == lf["name"]), None)  # suffix-name fallback
            if tgt: agg[id(tgt)][0] += lf["cov"]; agg[id(tgt)][1] += lf["tot"]
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
```

**Calibration caveat (honest limit).** spec-crap notes "threshold numbers assume radon calibration" (`spec-crap/SKILL.md:44` [V]). rust-code-analysis's cyclomatic count is close but **not bit-identical** to radon's. The `>8` line is a fixed constant; keep it, but **re-validate the threshold feel on the actual Genesis crate** before trusting absolute numbers. This is the one CRAP item that cannot map exactly — closest substitute: keep `>8`, calibrate empirically. [V caveat]

**Adapter robustness (VB + H1, baked into the code above):** llvm has no per-function `start_line`, so derive `[lo,hi]` from `min(LineStart)/max(LineEnd)` over regions **filtered to `Kind==0 AND FileID==0`** (drops macro-expansion regions whose line is at the macro site, outside the function's textual span); join by **span overlap → innermost** rca function (routes a closure's coverage to the closure's enclosing fn, not double-counted), with a short-name suffix fallback; skip closures (a `function` nested in a `function`) and route `impl`/`trait` methods by synthesized `parent_kind`. Unmatched functions default to **0%** — matching `crap.py`'s own `.get(key, 0.0)` — which does not cause false FAILs because a trivial `CC=1` function scores `CRAP=2` even at 0%. Still spot-check line-range alignment on one real crate before trusting the gate. [V H1]

## 1.2 The rest of the Python surface → Rust

| Python gate / tool | Rust replacement | Exact command | Block computed by |
|---|---|---|---|
| **CRAP** (`crap.py`) | `rust-code-analysis-cli` + `cargo-llvm-cov` + adapter → **unchanged `crap.py`** | see §1.1 | `crap.py` exits 2 iff any CRAP>8 [V] |
| **pytest-bdd / playwright-bdd** (outer loop) | **cucumber-rs** (`cucumber` crate) | `cargo test --test bdd_<name>` (harness=false) | undefined/`unimplemented!()` step panics → RED; real World+deps → no-mock GREEN [I] |
| **pytest** unit (inner loop) | `cargo test` | `cargo test` / `cargo test --release` / `cargo test <module>::<test>` | any failing `#[test]` → non-zero exit [I] |
| **coverage threshold** (`--cov-fail-under`) | `cargo-llvm-cov` built-in | `cargo llvm-cov --fail-under-lines 80` | tool exits 1 below MIN [V] |
| **ruff / black** | `cargo fmt` + `clippy` | `cargo fmt --check` ; `cargo clippy --all-targets -- -D warnings` | non-zero on unformatted / any lint [V ifs run-tests.sh] |
| **mypy** | the compiler | `cargo check` (implicit in test/build) | type error → compile fail [I] |
| `pytest.fail("Not implemented")` stub → RED | `unimplemented!("Implement via TDD")` | in stub bodies | panics at runtime → test fails → RED [V ifs stub convention] |

**GREEN gets stricter for Rust (ifs pattern, [V] `run-tests.sh:24-33`, `new-model/SKILL.md:203`).** Rust "GREEN" is defined to **include fmt + clippy**, not just tests:
```
cargo fmt --check  &&  cargo clippy --all-targets -- -D warnings  &&  cargo test --release
```
This sequence becomes the Rust branch of Phase 5 GREEN and Phase 8 Regression — strengthening, never weakening, the gate.

---

# 2. The Rust MCP memory-server stack (Cargo.toml + versions + shapes)

Net-new — none of this exists in the source repos. All versions/APIs read from crates.io + upstream **this run**; `sfr-VC` re-verified 19/21 claims and caught 2 corrections (folded in below).

## 2.1 Version matrix [V unless noted]

| Crate | Pin | Notes |
|---|---|---|
| `rmcp` | **2.2.0** | official Rust MCP SDK; **2.x not 0.x**; `#[tool_router]/#[tool]/#[tool_handler]/ServerHandler`; `default` already pulls `macros`+`server` |
| `rusqlite` | **0.40.1** | `bundled` (compiles SQLite in) |
| `sqlite-vec` | **0.1.9** | stable (0.1.10 is alpha); register via `sqlite3_auto_extension(sqlite3_vec_init)` |
| `ort` | **=2.0.0-rc.12** | ONNX Runtime; **no stable 2.0** — pin exactly (RCs break each other); default api feature is **api-24** (not api-27), default also pulls `tls-native` |
| `ndarray` | **0.17** | ort's pinned companion |
| `tokenizers` | **0.23.1** | `from_file`/`encode`/`get_ids`/`get_attention_mask` |
| `bytemuck` | 1.x | `cast_slice::<f32,u8>` for vec0 blobs (avoids zerocopy 0.7→0.8 `AsBytes`→`IntoBytes` churn) |
| `cucumber` | **0.23.0** | BDD outer loop (ifs used 0.21; use latest for greenfield) |
| dev: `assert_cmd` 2.2.2, `insta` 1.48.0, `approx` 0.5.1, `tempfile` 3 | | integration/golden/epsilon/temp-DB |
| tool: `cargo-llvm-cov` 0.8.x, `rust-code-analysis-cli` | | CRAP inputs (§1.1) |

## 2.2 `Cargo.toml` (server crate — resolves; VC-confirmed) + the ifs lint gate

```toml
[package]
name = "genesis-memory-server"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "genesis-memory-server"
path = "src/main.rs"

[lib]
name = "genesis_memory"
path = "src/lib.rs"

[dependencies]
# MCP server (official Rust SDK)
rmcp       = { version = "2.2.0", features = ["server", "macros", "transport-io"] }  # transport-io is REQUIRED for stdio() — `server` alone lacks tokio/io-std [V H2]; `server` already implies macros+schemars
schemars   = "1"
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
# async runtime
tokio   = { version = "1", features = ["macros", "rt", "rt-multi-thread", "io-std", "signal"] }
anyhow  = "1"
thiserror = "2"
# vector store
rusqlite   = { version = "0.40.1", features = ["bundled"] }
sqlite-vec = "0.1.9"
bytemuck   = "1"
# embeddings
ort        = { version = "=2.0.0-rc.12", features = ["download-binaries", "ndarray", "std"] }
ndarray    = "0.17"
tokenizers = "0.23.1"
# logging
tracing            = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }

[dev-dependencies]
cucumber  = { version = "0.23", features = ["macros"] }
assert_cmd = "2.2"
approx    = "0.5"
insta     = "1"
tempfile  = "3"

# --- ifs lint gate, verbatim from denier-rust/common-rust (crate-level; genesis has no workspace) ---
[lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"
unreachable_pub = "warn"

[lints.clippy]
pedantic = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unimplemented = "warn"      # so unimplemented!() scaffold stubs compile
module_name_repetitions = "allow"
missing_errors_doc = "warn"
missing_panics_doc = "warn"

[profile.release]
lto = "fat"
codegen-units = 1
panic = "unwind"
strip = "debuginfo"

# cucumber suites are harness=false bins (one per tool feature) — the template lacks these, MUST be added:
[[test]]
name = "bdd_store"
path = "tests/bdd/store_steps.rs"
harness = false
# … bdd_recall, bdd_consolidate — each harness=false with explicit path
```

Notes: `ort` pinned with `=` (RCs break). If the host must not download ORT at build, swap `download-binaries`→`load-dynamic` + set `ORT_DYLIB_PATH`. `download-binaries` keeps the Genesis binary self-contained.

## 2.3 Minimal working shapes (VC-confirmed against upstream examples)

**(a) rmcp stdio server exposing one tool** (`counter.rs`/`counter_stdio.rs`, main):
```rust
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*, schemars, tool, tool_handler, tool_router, transport::stdio};

#[derive(Clone)]
pub struct MemoryServer { tool_router: ToolRouter<MemoryServer> /* + store + embedder */ }

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RecallArgs { pub query: String, pub k: Option<u32> }

#[tool_router]
impl MemoryServer {
    pub fn new() -> Self { Self { tool_router: Self::tool_router() } }
    #[tool(description = "Recall the k most relevant memories for a query")]
    async fn recall(&self, Parameters(RecallArgs { query, k }): Parameters<RecallArgs>)
        -> Result<CallToolResult, McpError> {
        let hits = /* embed(query) -> sqlite-vec KNN */ format!("results for {query} (k={k:?})");
        Ok(CallToolResult::success(vec![ContentBlock::text(hits)]))
    }
}
#[tool_handler]
impl ServerHandler for MemoryServer {
    fn get_info(&self) -> ServerInfo {                                  // sync fn (verified 2.2.0)
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)   // pin: default()=LATEST=V_2025_11_25
    }
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = MemoryServer::new().serve(stdio()).await?;
    service.waiting().await?; Ok(())
}
```

> **rmcp 2.2.0 gotchas (all [V] H2, docs.rs/rmcp/2.2.0):** use **`ContentBlock::text`** — a bare `Content` type does **not exist** in 2.2.0 (`Content::text` won't compile). `ServerInfo::new(caps)` is a real assoc fn (`ServerInfo = InitializeResult`) and `get_info` is a **sync** `fn`. Feature `transport-io` is **required** for `stdio()`. Integration-test client needs `features = ["client", "transport-child-process"]`. All of `CallToolResult`, `ContentBlock`, `ServerInfo`, `ServerCapabilities`, `ProtocolVersion`, `Implementation` live in `rmcp::model`.

**(b) sqlite-vec create / insert / KNN** (`asg017/sqlite-vec` `demo.rs`, main):
```rust
use rusqlite::{ffi::sqlite3_auto_extension, Connection, params};
use sqlite_vec::sqlite3_vec_init;
unsafe { sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ()))); }
let db = Connection::open("memory.db")?;
db.execute("CREATE VIRTUAL TABLE vec_items USING vec0(embedding float[384])", [])?;   // MiniLM/bge = 384
let emb: Vec<f32> = /* embed */ vec![0.0; 384];
db.prepare("INSERT INTO vec_items(rowid, embedding) VALUES (?, ?)")?
  .execute(params![42_i64, bytemuck::cast_slice::<f32,u8>(&emb)])?;
// KNN — use the VERIFIED form (ORDER BY distance LIMIT k); `MATCH ? AND k=?` is unverified:
let rows: Vec<(i64,f64)> = db.prepare(
    "SELECT rowid, distance FROM vec_items WHERE embedding MATCH ?1 ORDER BY distance LIMIT 5")?
  .query_map([bytemuck::cast_slice::<f32,u8>(&q)], |r| Ok((r.get(0)?, r.get(1)?)))?
  .collect::<Result<_,_>>()?;
```
Default `vec0` distance is L2; because the pipeline L2-normalizes (below), **L2 order == cosine order**, so keep the default.

**(c) `embed(text) -> Vec<f32>` — ort + tokenizers, mean-pool + L2-normalize** (`pykeio/ort` sentence-transformers example, main). **⚠ Model/pooling must match (VC must-fix):**
- **Primary recommendation: `all-MiniLM-L6-v2`** (384-dim, **mean pooling** — HF `1_Pooling/config.json` `pooling_mode_mean_tokens: true` [V]). The code below is mean-pool, so it is *correct for MiniLM*.
- **Alternative: `bge-small-en-v1.5`** (384-dim, stronger MTEB retrieval) — but it uses **CLS pooling** (`pooling_mode_cls_token: true` [V]). If you choose bge, **replace mean-pool with `pooled[d] = hidden[[0,0,d]]`** and prepend the query instruction prefix. Pairing bge with mean-pool is a correctness bug.

```rust
fn embed(session: &mut ort::session::Session, tok: &tokenizers::Tokenizer, text: &str) -> ort::Result<Vec<f32>> {
    let enc = tok.encode(text, true).map_err(|e| ort::Error::new(e.to_string()))?;
    let seq = enc.len();
    let ids:  Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
    let mask: Vec<i64> = enc.get_attention_mask().iter().map(|&x| x as i64).collect();
    let a_ids  = ort::value::TensorRef::from_array_view(([1usize, seq], &*ids))?;
    let a_mask = ort::value::TensorRef::from_array_view(([1usize, seq], &*mask))?;
    // add token_type_ids (all-zero) as a 3rd input if the .onnx requires it — dump Session::inputs to confirm
    let outputs = session.run(ort::inputs![a_ids, a_mask])?;
    let hidden = outputs[0].try_extract_array::<f32>()?.into_dimensionality::<ndarray::Ix3>().unwrap();
    let dim = hidden.shape()[2];
    let (mut pooled, mut denom) = (vec![0f32; dim], 0f32);   // MEAN pool (MiniLM). For bge: pooled[d]=hidden[[0,0,d]]
    for t in 0..seq { let m = mask[t] as f32; if m == 0.0 { continue; }
        denom += m; for d in 0..dim { pooled[d] += hidden[[0,t,d]] * m; } }
    let denom = denom.max(1e-9); for d in 0..dim { pooled[d] /= denom; }
    let norm = pooled.iter().map(|x| x*x).sum::<f32>().sqrt().max(1e-12);
    for d in 0..dim { pooled[d] /= norm; }                   // L2 normalize
    Ok(pooled)
}
```
The raw HF `onnx/model.onnx` emits `last_hidden_state` (output[0]); do steps above yourself. (A sentence-transformers ONNX bundle that bakes in pooling would expose a pooled output at index[1] — match the code to the export you ship.)

## 2.4 Consolidation (per-agent memory) — concrete + testable (design, [I])

Schema: `memories(id, agent_id, text, created_at, last_used_at, use_count, base_score, superseded_by)` + `vec_items USING vec0(embedding float[384])` (rowid == memories.id).
- **Decay/recency:** `effective = base_score · exp(-λ·age_days) · (1 + β·ln(1+use_count))`, `λ=ln2/30` (30-day half-life), `β=0.15`; bump `use_count`+set `last_used_at` on recall.
- **Dedup/compress on insert:** KNN top-1 within `agent_id`; cosine `= 1 − L2²/2` (normalized); `≥ TAU_MERGE (0.95)` → merge into higher-score row (sum `use_count`, set loser `superseded_by`), no new vector.
- **Summarize/evict** when `count > CAP`: rank by `effective`, evict bottom E% below `THETA_EVICT` and older than `MIN_AGE`; for dense clusters (cosine radius ≥ `TAU_SIM`, size ≥ K) emit one summary row (`base_score=max`, `use_count=sum`), set members' `superseded_by`.
- **Determinism knobs for tests:** inject `now` (clock trait) + the summarizer; expose every threshold as config → each is assertable within 1e-6.

---

# 3. The exact per-file update plan for `genesis/.claude/skills/`

**Preconditions (one-time, before the first `/spec-forge` run):**

- **P-1 — Scaffold the greenfield server** (Genesis `server/` is empty: no `Cargo.toml`, no `src/`, no `tests/` [V]). Add a small skill `spec-scaffold/SKILL.md` (or lay down by hand once) that writes `server/Cargo.toml` (§2.2), stub `src/{lib.rs, main.rs, store.rs, embed.rs, consolidate.rs}` with `unimplemented!("Implement via TDD")` bodies, and empty `tests/{bdd/,golden/}`. This mirrors `new-model` Phase 2's **skip-if-exists** scaffold guard and keeps the *supervisor untouched* (scaffolding is a precondition, not a new phase). Import the **flat** layout convention — never the nested `component_manager/…` tree (that tree in `new-model/SKILL.md:99-118` is stale; `component-template-rust.md` + real crates are flat [V]).
- **P-2 — Copy `test/tools/crap.py` verbatim** into `genesis/test/tools/crap.py`, add the new `test/tools/rust_crap_adapter.py` (§1.1), and copy `test/tools/timeline_writer.py` (see edit E-9).
- **P-3 — Ensure a Python 3 dev interpreter is on PATH** (genesis already ships Python hooks `gate.py`/`validate.py`/`inject.py`, so Python is already a dev dependency — `crap.py`+adapter+timeline_writer stay Python; the *product* stays pure Rust). Ensure `cargo-llvm-cov` + `rust-code-analysis-cli` are installed (`cargo install cargo-llvm-cov rust-code-analysis-cli`).

**Copy set → `genesis/.claude/skills/`.** Bring the whole `/spec-forge` family: `spec-forge/` (+`routing-table.md`,`handoff-schema.md`), `forge-{spec,dev,verify,review,docs}-agent/`, and the leaf skills `spec-{create,compile,test,crap,simplify}/`, plus the base `{spec,dev,verify,review,docs}-agent/` (the forge-* variants delegate to them). Also copy `docs/spec-driven-development.md` + `docs/multi-agent-workflow.md` for the agents to read.

## 3.1 Files copied VERBATIM (0 edits — proven language-agnostic)

- `spec-forge/routing-table.md` — the `(phase,status,next_responsibility)→agent` table has zero language tokens. **0 edits.** [V]
- `spec-forge/handoff-schema.md` — directive/verdict JSON; `crap_report{file,function,cc,coverage,crap}` + `skills_invoked` are numeric/string. **0 edits.** [V]
- `forge-docs-agent/`, `docs-agent/` — docs are language-neutral (`docs/architecture.md`). **0 edits.**
- All `superpowers:*` skill invocations (worktree, brainstorming, writing-plans, TDD, systematic-debugging, verification-before-completion, requesting/receiving-code-review, finishing-a-development-branch) — **0 edits.**

## 3.2 Files EDITED — the language-backend swap ("in <file>, change X → Y")

**E-0 — `spec-forge/SKILL.md` (minimal).** Add one Phase-0 sub-step: *detect language and write `state.json.language`* using the `new-model` Step 0b rule (`--lang` flag → else `Cargo.toml`→rust / `pyproject.toml`→python → else default). In Phase 2 wording, change "`test/unit/test_<slug>.py` stubs" → "language-appropriate unit-test stubs (Rust: `#[test]` fns + `tests/bdd/<slug>_steps.rs`)". Routing/gates/phase order untouched. (New sibling file `spec-forge/language-detection.md`, lifted from `new-model/SKILL.md:27-35`.)

**E-1 — `verify-agent/SKILL.md` (canonical test behavior — the core edit).** Add a **Rust branch** alongside the existing playwright-bdd / pytest-bdd branches (this is the established pattern):
- allowed-tools: add `Bash(cargo *)`.
- §Phase 2 (generate tests): Rust → *no `bddgen`*; cucumber-rs reads `.feature` directly; assert `tests/bdd/*_steps.rs` exist.
- §Phase 4 (execute): BDD → `cargo test --release --test 'bdd_*'`; unit → `cargo test`; `tdd_green_check` → `cargo test <module>::<test>`; `regression` → `cargo test --release` (whole suite).
- **§Phase 5/8 GREEN definition (Rust):** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release` (fmt+clippy are part of GREEN — ifs pattern).
- §Phase 5 red_check counts: parse `cargo test` output (or add `cargo-nextest` / libtest `--format json`) into `bdd`/`unit` `{total,passed,failed}`. RED condition (`passed==0 && failed==total`) unchanged.
- Log-validation, verdict schema, no-mock app lifecycle — unchanged (for a server, "start the real app" = spawn the MCP server over stdio and run the initialize handshake; for library-level BDD, drive the real lib+SQLite+ONNX directly).

**E-2 — `forge-verify-agent/SKILL.md`.** allowed-tools: `Bash(pytest *)` → add `Bash(cargo *)`. Body delegates to `verify-agent` (E-1), so no other change.

**E-3 — `spec-test/SKILL.md`.** Mirror E-1's Rust branch (it duplicates the pytest/playwright commands).

**E-4 — `spec-crap/SKILL.md` (CRAP skill).**
- allowed-tools: `Bash(pytest *), Bash(radon *), Bash(python *)` → `Bash(cargo *), Bash(rust-code-analysis-cli *), Bash(cargo-llvm-cov *), Bash(python *)` (python kept for adapter+crap.py).
- §Preflight: check `rust-code-analysis-cli --version` + `cargo llvm-cov --version` (was `radon --version` + `python -c "import coverage"`).
- §Coverage: `cargo llvm-cov --json --release --output-path test-results/llvm-cov.json` (was `pytest --cov=src --cov-report=json:…`).
- §CC: `mkdir -p test-results/rca && rust-code-analysis-cli -m -p src/ -O json -o test-results/rca/` (was `radon cc -s --json src/ > …`; note the `-o` dir must pre-exist and files are `*.rs.json`).
- §Compute: `python test/tools/rust_crap_adapter.py` (was `python test/tools/crap.py`; adapter builds the two JSONs then calls crap.py). **Thresholds 30/8/4 and exit-2 semantics unchanged.** Add the calibration note (§1.1).

**E-5 — `forge-review-agent/SKILL.md`.** Same command swap as E-4 in its inline **Step 1 (CRAP)**; allowed-tools swap. **Step 2 (local `superpowers:requesting-code-review` over `BASE_SHA..HEAD_SHA`) unchanged.** Blocking `= above_fail_count>0 OR any Critical/Important` **unchanged.** Add to the reviewer prompt: check the Genesis accuracy rules (status labels, no over-claiming production features). This preserves the one real spec-forge divergence (local review, no GitHub PR).

**E-6 — `spec-compile/SKILL.md` + `spec-create/SKILL.md` + `spec-agent/SKILL.md` (stub generation).** Add a Rust branch:
- BDD steps: generate `tests/bdd/<slug>_steps.rs` (cucumber-rs: `#[derive(Debug,Default,World)]`, `#[given/when/then(regex=…)]` async fns, `#[tokio::main] World::cucumber().run_and_exit("test/features/<slug>.feature")`) with `unimplemented!()` bodies — plus the matching **`[[test]] name=… path=… harness=false`** block in `Cargo.toml` (**the ifs template omits this — it must be generated**, per `sfr-D §4` [V]).
- Unit stubs: `#[test] fn <ac>() { unimplemented!("Implement via TDD") }` in `src/…#[cfg(test)] mod tests` or `tests/*.rs` (was `test/unit/test_<slug>.py` with `pytest.fail(...)`). The `unimplemented!()` panic guarantees RED (clippy `unimplemented = "warn"`, so it compiles).
- The spec-structure gate (non-UI-observable Expected Behavior halts compile) is language-agnostic — unchanged.

**E-7 — `dev-agent/SKILL.md`.** Replace the Python style rules (`type hints, pathlib, f-strings, mutable default args`, `dev-agent/SKILL.md:37-39`) with Rust idioms enforced by clippy pedantic + the lint gate (no `unwrap/expect/panic/todo` in `src/`, `?`-propagation, `thiserror`). Keep the size limits (modules <300, fns <50). `forge-dev-agent`: add `Bash(cargo *)` to allowed-tools (quick TDD feedback).

**E-8 — `spec-simplify/SKILL.md`.** Include/exclude globs: exclude `target/`, `Cargo.lock` (was `.venv/`, `node_modules/`, `package.json`); keep `src/`.

**E-9 — `test/tools/timeline_writer.py` (keep Python, one edit).** Its actor set is hardcoded to the 5 **base** agent names (`timeline_writer.py:69-72` [V]); add the `forge-*` actor names so forge verdicts render. Pure dev-tool; stays Python.

**E-10 (optional) — `new state.json.language` field.** Written in Phase 0 (E-0). Genesis is Rust-only, so this could be hardcoded `"rust"`; parameterizing keeps the copy reusable and matches the `new-model` precedent.

**Net:** supervisor lifecycle, routing table, handoff schema, audit, state/resume, checkpoint, all superpowers skills — **unchanged**. Edits confined to the 3 seams + 2 new tool files + 1 scaffold precondition.

---

# 4. Full step/gate → Rust-satisfaction mapping (proves nothing is skipped)

| spec-forge step / gate | Language? | How satisfied for the Rust Genesis server | Verdict |
|---|---|---|---|
| P0 Initiation, `state.json`, `timeline.html` | agnostic | verbatim; add `state.json.language="rust"`; `timeline_writer.py` +forge actors (E-9) | ✅ full |
| P0a Worktree isolation | agnostic | `superpowers:using-git-worktrees` verbatim | ✅ full |
| P0b Spawn 5 forge-* agents | agnostic | verbatim (Task/general-purpose) | ✅ full |
| P1 Spec discovery + **hallucination audit** (6 markers) | agnostic | verbatim; audits the memory-server spec | ✅ full |
| P2 Compile → `.feature` + stubs | **Rust** | `.feature` kept; Rust stubs `tests/bdd/*_steps.rs` + `#[test] unimplemented!()` + `[[test]] harness=false` blocks (E-6) | ✅ full |
| P2 **Spec-structure gate** | agnostic | verbatim | ✅ full |
| P2.5 Implementation plan | agnostic | `superpowers:writing-plans` verbatim | ✅ full |
| P3 **RED check** (all fail) | **Rust** | `cargo test`; `unimplemented!()` stubs panic → RED; count-parse (E-1) | ✅ full |
| P4 TDD loop, **Three Laws**, 1-commit/task | **Rust**(exec) | `superpowers:test-driven-development` verbatim; `cargo test <module>` inner loop | ✅ full |
| P4 **systematic-debugging** iter≥3 | agnostic | verbatim (routing on iter count) | ✅ full |
| P4 **verification-before-completion** | agnostic | verbatim (audited via `skills_invoked`) | ✅ full |
| P5 **GREEN** (full) | **Rust** | `cargo fmt --check && cargo clippy --all-targets -D warnings && cargo test --release` (E-1) | ✅ **strengthened** |
| P5 **No-mock BDD** | **Rust** | cucumber-rs over real SQLite + real ONNX + real stdio server; no mocks | ✅ full |
| P6 Simplify | **Rust** | `simplify` skill; Rust globs (E-8) | ✅ full |
| P7 Verify post-simplify | **Rust** | E-1 branch | ✅ full |
| P8 Regression (whole suite) | **Rust** | `cargo test --release` full + fmt/clippy (E-1) | ✅ full |
| P9 **CRAP > 8** | **Rust** | rust-code-analysis + cargo-llvm-cov → adapter → **unchanged crap.py**; exit-2 preserved (§1.1) | ✅ full* (calibration caveat) |
| P9 **Review-blocking** (local subagent) | agnostic | `superpowers:requesting-code-review` over `BASE_SHA..HEAD_SHA`; block on CRAP>8 OR Critical/Important — verbatim (E-5) | ✅ full |
| P9 `receiving-code-review` on fixes | agnostic | verbatim | ✅ full |
| P10 Docs | agnostic | `docs/architecture.md` verbatim | ✅ full |
| P11 Finalization | agnostic | `superpowers:finishing-a-development-branch` verbatim | ✅ full |
| **Table-driven routing** | agnostic | `routing-table.md` verbatim (0 edits) | ✅ full |
| **Handoff JSON schema** | agnostic | `handoff-schema.md` verbatim; `crap_report` numeric shape carries over | ✅ full |
| **Run-state / resume / checkpoint** | agnostic | `.planning/builds/<run-id>/…` verbatim; resume re-spawns 5 | ✅ full |
| **Log-validation gate** | agnostic | verbatim (general-purpose subagent) | ✅ full |

**The only non-exact map:** the CRAP `>8` threshold's *absolute calibration* (radon CC ≠ rust-code-analysis CC). The gate, formula, exit-2, and blocking are identical; only the numeric feel needs one empirical re-validation on the Genesis crate. Honest substitute: keep `>8`, calibrate once. Everything else is full-fidelity.

---

# 5. Best practices for spec-driven Rust MCP development

1. **Unit-test tool handlers directly, RED first.** rmcp tools are plain typed fns (`fn recall(&self, Parameters(a): Parameters<RecallArgs>) -> Result<…>`); test them in `#[cfg(test)] mod tests` **before** the `#[tool]`/router/transport wiring. Keep `store/recall/consolidate` *logic* in a `memory` module taking parsed args + injected store/embedder; the `#[tool]` method is a thin adapter. Small fns → CC ≤ ~4 → low CRAP. [V]
2. **BDD one `.feature` per tool, no mocks.** cucumber-rs `store.feature`/`recall.feature`/`consolidate.feature`, each `Given/When/Then` over **real** SQLite + **real** ONNX. `World` holds a `tempfile` DB path + a loaded model handle → hermetic & isolated. Never edit a `.feature` to pass (anti-cheating rule). [V]
3. **Golden-vector tests for embeddings.** Freeze `(model.onnx, tokenizer.json, pooling, normalization)`; embed a fixed input; assert against a committed golden. Determinism knobs (ort): `with_deterministic_compute(true)`, `with_intra_threads(1)`, parallel-exec disabled (default), pinned optimization level + CPU EP, `commit_from_file`. Assert absolute tol `1e-4` on the L2-normalized vector (`approx::assert_abs_diff_eq!`) **or** `cosine ≥ 0.9999` (more portable). Tune epsilon in the **same profile you gate on** (release). Pin the model file by SHA-256 as a fixture. [V]
4. **Vector-store correctness tests.** Insert known vectors → assert KNN neighbours + order + distance. Exact-match query → `distance == 0.0` (cheap deterministic anchor). Dimension mismatch must return `Err` (`assert!(result.is_err())`), **not panic**. [V]
5. **Integration over stdio — real JSON-RPC lifecycle.** `initialize → notifications/initialized → tools/list → tools/call`. **A tool failure is a *successful* response with `isError: true`, not a JSON-RPC error** — assert on `isError:true` for failing tools; reserve protocol-error assertions for malformed requests. Harness: (A) rmcp's own client via `TokioChildProcess` (needs `client`+`transport-child-process`) — highest fidelity, can back the cucumber `When`/`Then`; (B) `assert_cmd` `write_stdin` for exact wire/error framing. [V]
6. **Determinism + hermeticity everywhere** — the precondition for a *stable* CRAP coverage number (flaky tests → flaky coverage → unstable gate) and for RED/GREEN to mean anything: fresh temp SQLite per scenario, explicit sqlite-vec load, no wall-clock/RNG in tested logic (inject clock/ids), release profile for the coverage run.
7. **Don't bypass the gate to satisfy it.** Raise coverage / lower CC to beat CRAP — never lower the threshold or delete tests. Add a failing scenario that reproduces a bug *before* fixing it. One commit per plan task. [V]

---

# 6. Verified-vs-Inferred appendix

## 6.1 VERIFIED this run (primary source cited)
- **spec-forge = 12 phases + the full gate set** exactly as §0 (read from the skills). Routing table + handoff schema language-agnostic. [`sfr-A-gates.md`]
- **CRAP:** formula/thresholds/exit-2 (`crap.py`); **`metrics.cyclomatic.sum`** = leaf CC (rust-code-analysis source); llvm-cov per-function object **has no `percent`**, region tuple **idx4=exec_count, idx7=kind, CodeRegion==0** (LLVM source); join must be file+line-range. Adapter reuses `crap.py` byte-identical. [`sfr-B`, `sfr-VB` 4/4 CONFIRMED]
- **Stack:** rmcp **2.2.0** (`#[tool_router]/#[tool]/#[tool_handler]`, `serve(stdio())`), rusqlite **0.40.1** bundled, sqlite-vec **0.1.9** (`sqlite3_auto_extension`+`sqlite3_vec_init`; `vec0(embedding float[384])`; `WHERE … MATCH ?1 ORDER BY distance LIMIT k`), ort **=2.0.0-rc.12** (no stable; `commit_from_file`/`TensorRef::from_array_view`/`run(ort::inputs![])`/`try_extract_array::<f32>()`), ndarray **0.17**, tokenizers **0.23.1**. Cargo.toml resolves. [`sfr-C`, `sfr-VC` 19/21]
- **Model pooling:** all-MiniLM-L6-v2 = mean pool; bge-small-en-v1.5 = **CLS pool** (both HF `1_Pooling/config.json`). Code shown is mean-pool → pair with MiniLM (or switch bge to CLS). [`sfr-VC` REFUTED the original bge+mean-pool pairing]
- **ifs Rust gate:** GREEN = `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release` (`run-tests.sh:24-33`); lint block (unsafe=forbid; unwrap/expect/panic/todo=deny; pedantic=warn) crate-level; flat `src/`; `unimplemented!("Implement via TDD")` stubs; cucumber-rs `[[test]] harness=false` bins wired only in outlier-rust (template lacks them). [`sfr-D`]
- **Genesis state:** `server/` empty, no `Cargo.toml`, no `.claude/` anywhere → greenfield. [`sfr-D`]
- **MCP:** lifecycle order; tool failure = success response with `isError:true`. [`sfr-E`]
- **Hardening pass (H1/H2):** rust-code-analysis root = unit `FuncSpace` (`name`=path, `kind`="unit"); `start_line`/`end_line`/`metrics.cyclomatic.sum` are the literal field names; closures & trait-default methods **also** serialize as `kind:"function"` (adapter now guards them); `-o` dir must pre-exist, files are `*.rs.json`. rmcp 2.2.0: `ContentBlock::text` (no `Content` type); `transport-io` **required** for `stdio()`; `ServerInfo::new(caps)` + **sync** `get_info` now source-verified (upgrades VC #5); `ProtocolVersion::default()`=`V_2025_11_25` so pinning `V_2024_11_05` is deliberate. [`sfr-H1`, `sfr-H2`]

## 6.2 INFERRED — verify before/at implementation
1. **CRAP threshold calibration** — rust-code-analysis CC ≠ radon CC; re-validate the `>8` feel on the Genesis crate. (only non-exact gate map)
2. **CRAP adapter join** — the closure/trait guards, `FileID==0` region filter, overlap→innermost containment, and unmatched→0% are now in the code (§1.1); still **run it against one real `cargo llvm-cov --json` + `rust-code-analysis-cli` dump** to confirm the region tuple indices (idx4/idx7, pre-verified by VB but not re-dumped) and the line-range alignment before trusting the gate.
3. **rust-code-analysis-cli flags** (`-m -p -O json -o`) — confirm vs `--help`.
4. **cucumber-rs 0.23 runner API** — ifs uses 0.21 `run_and_exit`; the quickstart shows `World::run(...)`. Confirm the 0.23 macro/runner surface + that `/spec-compile` emits both `tests/bdd/*_steps.rs` and the `[[test]] harness=false` blocks.
5. **sqlite-vec** `distance_metric=cosine` DDL and `k=?` KNN clause — UNVERIFIABLE this run; use the verified `ORDER BY distance LIMIT n`.
6. **ONNX export shape** — confirm the chosen model emits `last_hidden_state` (output[0], needs manual pooling) vs a pooled output; confirm whether `token_type_ids` (3rd input) is required (`Session::inputs` dump).
7. **ort default `api-24`** (not api-27) + default `tls-native` — cosmetic; Cargo.toml still resolves.
8. **Consolidation** (§2.4) — net-new design, no prior art; all thresholds are placeholders to tune.
9. **`timeline_writer.py` / crap.py / adapter as Python dev-tools** — fine given Genesis already ships Python hooks; if a pure-Rust toolchain is mandated, port crap.py to an `xtask` (formula is trivial; re-owns verification — §1.1 C.5).

---

## Appendix: research artifacts (job tmp, this session)
`sfr-A-gates.md` (gate checklist), `sfr-B-crap-rust.md` + `sfr-VB-verify-crap.md` (CRAP), `sfr-C-mcp-stack.md` + `sfr-VC-verify-stack.md` (stack), `sfr-D-ifs-genesis.md` (ifs+genesis), `sfr-E-bestpractices.md` (best practices), `sfr-Z-mainloop-firsthand.md` (orchestrator first-hand grounding).
