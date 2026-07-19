//! `genesis_memory` — the Genesis MCP memory server.
//!
//! Exposes per-agent semantic memory to any MCP client (Claude Code) over stdio:
//! `store` a memory, `recall` the k most relevant, and `consolidate` (decay / dedup /
//! summarize). Backed by SQLite + `sqlite-vec` for KNN and a local ONNX sentence
//! embedder (`ort` + `tokenizers`), so it runs fully offline under Claude Max.
//!
//! # Architecture
//!
//! - [`store`] — the `vec0(embedding float[384])` vector store (insert + KNN).
//! - [`embed`] — local ONNX embeddings (mean-pool + L2-normalize, 384-dim).
//! - [`consolidate`] — decay/recency scoring, dedup/merge, summarize/evict.
//!
//! Tool handlers stay thin: the `store`/`recall`/`consolidate` **logic** lives in these
//! modules (unit-testable with an injected store + embedder), and the `#[tool]` methods
//! are thin adapters over them (see `docs/SPEC_FORGE_RUST_UPDATE.md` §5 best-practice #1).
//!
//! Every function body below is an `unimplemented!("Implement via TDD")` stub — the
//! greenfield starting point the `/spec-forge` run fills in, RED → GREEN, one commit
//! per plan task.

#![forbid(unsafe_code)]

pub mod consolidate;
pub mod embed;
pub mod store;

use anyhow::Result;

/// Runs the MCP memory server over stdio: builds the `MemoryServer` (store + embedder),
/// registers the `store` / `recall` / `consolidate` tools, and serves `stdio()` until
/// the client disconnects.
///
/// # Errors
///
/// Returns an error if the store, embedder, or stdio transport fails to initialise.
pub async fn serve_stdio() -> Result<()> {
    // TODO(spec): MemoryServer::new(store, embedder).serve(stdio()).await?.waiting().await
    //             — rmcp 2.2.0 shape in docs/SPEC_FORGE_RUST_UPDATE.md §2.3a.
    unimplemented!("Implement via TDD — rmcp serve(stdio()) wiring (§2.3a)")
}
