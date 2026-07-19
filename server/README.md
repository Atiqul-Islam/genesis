# genesis-memory-server

The Genesis MCP memory server — per-agent semantic memory over SQLite + `sqlite-vec`
KNN and local ONNX embeddings (`ort` + `tokenizers`). Runs fully offline over stdio;
any MCP client (Claude Code) can `store`, `recall`, and `consolidate` memories.

> **Status: greenfield scaffold.** Every function body is `unimplemented!("Implement
> via TDD")`. This crate is built **spec-driven** by the Genesis copy of the `/spec-forge`
> workflow (`.claude/skills/`), RED → GREEN, one commit per plan task. The scaffold's first
> real compile is the workflow's RED gate.

## Layout (flat `src/` — ifs Rust convention)

| File | Concern |
|---|---|
| `src/lib.rs` | crate root + `serve_stdio()` entry (rmcp `serve(stdio())`) |
| `src/main.rs` | thin binary → `genesis_memory::serve_stdio()` |
| `src/store.rs` | `VectorStore` — `vec0(embedding float[384])` insert + KNN |
| `src/embed.rs` | `Embedder` — ONNX mean-pool + L2-normalize, 384-dim |
| `src/consolidate.rs` | decay/recency, dedup/merge, summarize/evict |
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
