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

// Source: test/specs/genesis-memory-server.md — Implementation Requirements
// (groups "Tool API (agent scoping)", "Recall response payload", "Server wiring").
#[cfg(test)]
mod tests {
    // ─── Tool API (agent scoping) ────────────────────────────────────────────

    /// Accept `store` arguments as `StoreArgs { agent_id: String, text: String }`.
    #[test]
    fn store_args_are_agent_id_and_text() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Accept `store` arguments as StoreArgs {{ agent_id: String, text: String }}"
        );
    }

    /// Accept `recall` arguments as `RecallArgs { agent_id: String, query: String, k: Option<u32> }`.
    #[test]
    fn recall_args_are_agent_id_query_and_optional_k() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Accept `recall` arguments as RecallArgs {{ agent_id: String, query: String, k: Option<u32> }}"
        );
    }

    /// Accept `consolidate` arguments as `ConsolidateArgs { agent_id: String }`.
    #[test]
    fn consolidate_args_are_agent_id() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Accept `consolidate` arguments as ConsolidateArgs {{ agent_id: String }}"
        );
    }

    /// Default `k` to 5 when the `recall` arguments omit it.
    #[test]
    fn recall_k_defaults_to_five() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Default `k` to 5 when the `recall` arguments omit it");
    }

    /// Serve all agents from one SQLite database rather than one database per agent.
    #[test]
    fn one_sqlite_database_serves_all_agents() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Serve all agents from one SQLite database rather than one database per agent"
        );
    }

    /// Scope every `memories` query by `agent_id`.
    #[test]
    fn every_memories_query_is_scoped_by_agent_id() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Scope every `memories` query by `agent_id`");
    }

    // ─── Recall response payload ─────────────────────────────────────────────

    /// Serialize the `recall` result as a JSON array of objects.
    #[test]
    fn recall_result_serializes_as_a_json_array_of_objects() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Serialize the `recall` result as a JSON array of objects"
        );
    }

    /// Give each `recall` result object exactly the keys `id`, `text`, and `distance`.
    #[test]
    fn recall_result_objects_have_exactly_id_text_and_distance() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Give each `recall` result object exactly the keys `id`, `text`, and `distance`"
        );
    }

    /// Order the `recall` result array by ascending `distance`, nearest first.
    #[test]
    fn recall_result_array_is_ordered_by_ascending_distance() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Order the `recall` result array by ascending `distance`, nearest first"
        );
    }

    /// Carry that JSON array as the string inside a single `ContentBlock::text`.
    #[test]
    fn recall_json_array_is_carried_in_a_single_content_block_text() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Carry that JSON array as the string inside a single `ContentBlock::text`"
        );
    }

    // ─── Server wiring ───────────────────────────────────────────────────────

    /// Keep `store` / `recall` / `consolidate` logic in library modules taking parsed
    /// arguments plus an injected store and embedder.
    #[test]
    fn tool_logic_lives_in_library_modules_with_injected_store_and_embedder() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Keep store / recall / consolidate logic in library modules taking parsed arguments plus an injected store and embedder"
        );
    }

    /// Build server info as `ServerInfo::new(ServerCapabilities::builder().enable_tools().build())`.
    #[test]
    fn server_info_is_built_from_enable_tools_capabilities() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Build server info as ServerInfo::new(ServerCapabilities::builder().enable_tools().build())"
        );
    }

    /// Pin the advertised protocol version to `ProtocolVersion::V_2024_11_05`.
    #[test]
    fn protocol_version_is_pinned_to_v_2024_11_05() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Pin the advertised protocol version to ProtocolVersion::V_2024_11_05"
        );
    }

    /// Return tool text payloads with `ContentBlock::text`.
    #[test]
    fn tool_text_payloads_use_content_block_text() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Return tool text payloads with `ContentBlock::text`");
    }

    /// Return tool failures as a `CallToolResult` with `isError` true rather than a JSON-RPC error.
    #[test]
    fn tool_failures_return_call_tool_result_with_is_error_true() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Return tool failures as a CallToolResult with isError true rather than a JSON-RPC error"
        );
    }

    /// Return `Ok(())` from `main` after `service.waiting().await?` so stdin EOF yields exit status 0.
    #[test]
    fn main_returns_ok_after_waiting_so_stdin_eof_exits_zero() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Return Ok(()) from main after service.waiting().await? so stdin EOF yields exit status 0"
        );
    }
}
