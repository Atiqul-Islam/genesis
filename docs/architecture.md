# Genesis MCP Memory Server — Architecture

Per-agent semantic memory exposed to any MCP client (Claude Code) over stdio. A memory is
embedded with a local ONNX sentence model, stored in SQLite with a `sqlite-vec` vector index,
and recalled by nearest-neighbour search — all offline, no network at request time.

Built spec-driven by the Genesis `/spec-forge` workflow. Source of truth: the ratified spec
`test/specs/genesis-memory-server.md` (17 acceptance criteria) and `docs/SPEC_FORGE_RUST_UPDATE.md`
§2/§5. This document describes what was implemented (crate `genesis_memory`, at `server/`).

## Component map (flat `src/`)

| File | Responsibility |
|---|---|
| `src/main.rs` | Thin binary: `#[tokio::main]` → `genesis_memory::serve_stdio()`. |
| `src/lib.rs` | MCP server wiring: `MemoryServer` (rmcp `#[tool_router]`/`#[tool]`/`#[tool_handler]`), the `store`/`recall`/`consolidate` tool adapters, the library logic functions `do_store`/`do_recall`, and `serve_stdio()`. |
| `src/store.rs` | `VectorStore` — SQLite + `sqlite-vec`: schema, agent-scoped `insert`/`knn`, and the consolidation helper methods. |
| `src/embed.rs` | `Embedder` — ONNX Runtime (`ort`) + `tokenizers`: text → 384-dim L2-normalized vector; model provenance constants. |
| `src/consolidate.rs` | `consolidate()` — decay/recency scoring + dedup/merge; the injectable `Clock`. |
| `tests/bdd/` | cucumber-rs BDD suites (one `harness=false` bin per feature), driving the real system. |
| `tests/golden/` | Frozen golden embedding vector (determinism fixture). |
| `tests/fixtures/` | Calibrated AC3 paraphrase/decoy retrieval fixture. |

## Layering (thin adapters over injected logic)

The `#[tool]` methods are **thin adapters**. All logic lives in free functions / store methods
that take parsed arguments plus an **injected** store + embedder (§5 best-practice #1), which is
what makes them unit-testable without the MCP transport:

```
MCP client ── stdio JSON-RPC ──▶ MemoryServer::{store,recall,consolidate}   (rmcp adapters)
                                        │
                                        ▼
                     do_store / do_recall / consolidate::consolidate        (pure-ish logic)
                                        │
                        ┌───────────────┼────────────────┐
                        ▼               ▼                 ▼
                    Embedder        VectorStore       ConsolidationConfig + Clock
                   (ort+tok)      (rusqlite+vec)
```

The embedder is loaded **lazily** on first `store`/`recall` (shared by both adapters via
`MemoryServer::load_embedder_into`), behind a `tokio::sync::Mutex` so the single store + session
are serialized across concurrent tool calls.

## Data model

One shared SQLite database (`GENESIS_MEMORY_DB`, default `genesis-memory.db`), every query scoped
by a caller-supplied `agent_id`:

- `memories(id INTEGER PRIMARY KEY, agent_id, text, created_at, last_used_at, use_count,
  base_score, superseded_by → memories.id)`
- `vec_items USING vec0(embedding float[384])`, with `vec_items.rowid == memories.id` (the id is
  assigned by SQLite via `last_insert_rowid()` in one place, `VectorStore::insert`).

## Request flows

- **store** → embed `text` → `INSERT` the row (base_score from config) + its embedding under the
  same rowid, in one transaction → returns the assigned id.
- **recall** → embed `query` → agent-scoped KNN → hydrate text, bump `use_count`/`last_used_at` on
  each hit → return a JSON `[{id, text, distance}]` array (ascending distance) inside one
  `ContentBlock::text`. `k` defaults to 5.
- **consolidate** → one decay-scored dedup/merge pass: repeatedly merge the first near-duplicate
  pair (cosine ≥ `tau_merge`) into the higher-`effective` survivor (sum `use_count`, set the
  loser's `superseded_by`, no new vector row) until none remain.

### KNN under `sqlite-vec` (the one non-obvious query)

`vec0`'s KNN (`WHERE embedding MATCH ? ORDER BY distance LIMIT k`) does not accept extra
predicates, so agent scoping and the `superseded_by` exclusion cannot ride on it. Instead the
verified KNN runs as an **inner subquery over a candidate pool sized to the whole table**, then an
**outer join** to `memories` applies `agent_id = ? AND superseded_by IS NULL ORDER BY distance
LIMIT k` — so the outer filter can never under-return.

### Embedding pipeline

`all-MiniLM-L6-v2` (384-dim), fetched + SHA-256-pinned by `scripts/fetch-model` into
`server/models/` (never committed). Pipeline: tokenize (special tokens) → ONNX session run with
three named inputs `input_ids` / `attention_mask` / **`token_type_ids`** (all-zeros — **required**
by this export, confirmed empirically, §6.2 #6) → attention-mask-weighted **mean pool** over
`last_hidden_state` → **L2-normalize** (with `1e-9`/`1e-12` clamps). Determinism knobs pinned
(CPU EP, single intra-thread, deterministic compute, fixed opt level) so the golden vector is
reproducible. Because vectors are L2-normalized, `vec0`'s default L2 distance orders identically
to cosine.

### Consolidation scoring

`effective = base_score · exp(−λ·age_days) · (1 + β·ln(1 + use_count))`, `age_days` from
`created_at` (§2.4 + spec D2). Defaults: `λ = ln2/30` (30-day half-life), `β = 0.15`,
`tau_merge = 0.95`, `base_score = 1.0`. `cosine = 1 − L2²/2` (valid for normalized vectors).
`now` is injected via a `Clock` (`SystemClock` in production, `FixedClock` in tests).

## Key decisions & deviations from source

- **`unsafe_code = "deny"`** (not `forbid`) with exactly one scoped `#[allow(unsafe_code)]` on
  `VectorStore::open` for the required `sqlite3_auto_extension(sqlite3_vec_init)` FFI registration
  (§2.3b). Unsafe stays denied everywhere else.
- **`token_type_ids` is a required third ONNX input** for this export (§6.2 #6 was flagged
  unverified; confirmed by the real model erroring without it).
- **D8 — dedup runs consolidate-only**, not at `store` time (a documented deviation from §2.4's
  "Dedup/compress on insert").
- **D9 — AC12**: the MCP SDK silently drops syntactically-invalid (byte-garbage) JSON rather than
  replying; the protocol-error path (`-32600`) is asserted for structurally-invalid-but-parseable
  requests (§5 #5).

## Out of scope (v2)

`consolidate`'s **summarize/evict** pass is deferred: its thresholds (`THETA_EVICT`, `MIN_AGE`,
`TAU_SIM`, `K`, `E%`) are unsourced (§6.2 #8). `cap` (`10_000`) is retained in config as the future
eviction trigger, unused in v1.

## Quality gates (as run)

- **GREEN** = `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
  --release`. Lints: no `unwrap`/`expect`/`panic`/`todo` in `src/` (tests exempt via
  `clippy.toml`); pedantic on (except the cosmetic `doc_markdown`).
- **No-mock BDD**: 17 scenarios over real SQLite + real ONNX + the real spawned stdio binary;
  86 unit tests. Embeddings pinned by a golden vector; the AC3 retrieval fixture is calibrated
  against the real model.
- **CRAP gate** (`test/tools/rust_crap_adapter.py` → `crap.py`): `CRAP = CC²·(1−cov/100)³ + CC`,
  **exit 2 iff any function > 8** — all functions ≤ 8 (highest 7.1).

## Build / run

```
scripts/fetch-model                                       # one-time: fetch + pin the ONNX model
cargo run --manifest-path server/Cargo.toml --release     # serve MCP over stdio
```

The client sets `GENESIS_MEMORY_DB` (DB path) and, if the model is not in the default
`server/models/`, `GENESIS_MODEL_DIR`.
