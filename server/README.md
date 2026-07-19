# genesis-memory-server

The Genesis MCP memory server — per-agent semantic memory over SQLite + `sqlite-vec`
KNN and local ONNX embeddings (`ort` + `tokenizers`). Runs fully offline over stdio;
any MCP client (Claude Code) can `store`, `recall`, and `consolidate` memories.

> **Status: v1 implemented.** `store` / `recall` / `consolidate` (decay-scored dedup/merge)
> are built and tested — 86 unit tests + 17 BDD scenarios pass in release against the real
> ONNX model, real SQLite, and the real spawned stdio server (no mocks). Built **spec-driven**
> by the Genesis copy of the `/spec-forge` workflow (`.claude/skills/`), RED → GREEN, one
> commit per plan task.
>
> **Out of scope (v2):** `consolidate`'s summarize/evict pass (its thresholds are unsourced —
> see the ratified spec's "Out of scope (v2)"); `cap` is retained in config but unused in v1.

## Layout (flat `src/` — ifs Rust convention)

| File | Concern |
|---|---|
| `src/lib.rs` | crate root + `serve_stdio()` entry (rmcp `serve(stdio())`) |
| `src/main.rs` | thin binary → `genesis_memory::serve_stdio()` |
| `src/store.rs` | `VectorStore` — `vec0(embedding float[384])` insert + KNN |
| `src/embed.rs` | `Embedder` — ONNX mean-pool + L2-normalize, 384-dim |
| `src/consolidate.rs` | decay/recency scoring + dedup/merge (summarize/evict is v2, deferred) |
| `tests/bdd/` | cucumber-rs suites (one `harness=false` bin per tool) |
| `tests/golden/` | frozen embedding golden vectors |

## Build / test (the Rust GREEN gate)

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release
```

## CRAP gate (quality)

```
mkdir -p test-results/rca
rust-code-analysis-cli -m -p src/ -O json -o test-results/rca/
cargo llvm-cov --json --release --output-path test-results/llvm-cov.json
python ../test/tools/rust_crap_adapter.py    # → radon-cc.json + coverage.json → crap.py (exit 2 iff CRAP>8)
```

Prereqs: `cargo install cargo-llvm-cov rust-code-analysis-cli` and a Python 3 interpreter.
