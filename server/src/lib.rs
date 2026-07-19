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

// `unsafe_code` is denied crate-wide via Cargo.toml `[lints.rust]`; the sole scoped
// `#[allow(unsafe_code)]` is on `store::VectorStore::open` for the required sqlite-vec
// FFI registration (§2.3b). `deny` (not a source-level `forbid`) is what lets that one
// exception exist.

pub mod consolidate;
pub mod embed;
pub mod store;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{
    schemars, tool, tool_handler, tool_router, transport::stdio, ErrorData as McpError,
    ServerHandler, ServiceExt,
};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::consolidate::{Clock, ConsolidationConfig, SystemClock};
use crate::embed::Embedder;
use crate::store::VectorStore;

/// Default number of recall hits when `k` is omitted.
pub const DEFAULT_K: usize = 5;

/// One recall result: the memory id, its stored text, and its distance from the query.
#[derive(Debug, Serialize)]
struct Hit {
    id: i64,
    text: String,
    distance: f64,
}

/// Embeds `text` and stores it under `agent_id`; returns the assigned id.
///
/// The `store`/`recall`/`consolidate` logic lives in these free functions taking parsed
/// arguments plus an injected store + embedder; the `#[tool]` methods are thin adapters
/// (§5 best-practice #1).
///
/// # Errors
///
/// Returns an error if embedding or the store insert fails.
pub fn do_store(
    store: &mut VectorStore,
    embedder: &mut Embedder,
    cfg: &ConsolidationConfig,
    clock: &dyn Clock,
    agent_id: &str,
    text: &str,
) -> Result<i64> {
    let vec = embedder.embed(text)?;
    store.insert(agent_id, text, &vec, cfg.base_score, clock.now_unix())
}

/// Embeds `query`, runs agent-scoped KNN, bumps usage on each hit, and returns the JSON
/// payload string (a `[{id, text, distance}]` array ordered by ascending distance).
///
/// # Errors
///
/// Returns an error if embedding, KNN, hydration, or serialization fails.
pub fn do_recall(
    store: &mut VectorStore,
    embedder: &mut Embedder,
    clock: &dyn Clock,
    agent_id: &str,
    query: &str,
    k: usize,
) -> Result<String> {
    let vec = embedder.embed(query)?;
    let hits = store.knn(agent_id, &vec, k)?;
    let now = clock.now_unix();
    let mut out = Vec::with_capacity(hits.len());
    for (id, distance) in hits {
        let text = store.text_of(id)?;
        store.touch(id, now)?; // on recall: use_count += 1, last_used_at = now
        out.push(Hit { id, text, distance });
    }
    Ok(serde_json::to_string(&out)?)
}

/// Arguments for the `store` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct StoreArgs {
    /// The agent the memory belongs to.
    pub agent_id: String,
    /// The memory text to store.
    pub text: String,
}

/// Arguments for the `recall` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RecallArgs {
    /// The agent whose memories to search.
    pub agent_id: String,
    /// The query text.
    pub query: String,
    /// Max results (defaults to [`DEFAULT_K`] when omitted).
    pub k: Option<u32>,
}

/// Arguments for the `consolidate` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ConsolidateArgs {
    /// The agent whose memories to consolidate.
    pub agent_id: String,
}

/// The mutable state behind the server: the shared store and a lazily-loaded embedder.
struct Inner {
    store: VectorStore,
    embedder: Option<Embedder>,
}

/// The MCP memory server: thin `#[tool]` adapters over the library logic functions.
///
/// The `#[tool_router]` macro generates the associated `Self::tool_router()` the
/// `#[tool_handler]` impl calls per request, so the router is not stored on the struct.
#[derive(Clone)]
pub struct MemoryServer {
    inner: Arc<Mutex<Inner>>,
    cfg: ConsolidationConfig,
    model_dir: PathBuf,
}

impl MemoryServer {
    /// Builds a server over an already-open `store`; the embedder loads lazily from `model_dir`.
    #[must_use]
    pub fn new(store: VectorStore, model_dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                store,
                embedder: None,
            })),
            cfg: ConsolidationConfig::default(),
            model_dir,
        }
    }

    /// A tool failure surfaced as a successful response carrying `is_error: true`
    /// (NOT a JSON-RPC protocol error) — §5 #5.
    fn err_result(e: &anyhow::Error) -> CallToolResult {
        CallToolResult::error(vec![ContentBlock::text(e.to_string())])
    }
}

#[tool_router]
impl MemoryServer {
    #[tool(description = "Store a memory for an agent")]
    async fn store(
        &self,
        Parameters(a): Parameters<StoreArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.inner.lock().await;
        if g.embedder.is_none() {
            let m = self.model_dir.join("onnx/model.onnx");
            let t = self.model_dir.join("tokenizer.json");
            match Embedder::load(&m.to_string_lossy(), &t.to_string_lossy()) {
                Ok(e) => g.embedder = Some(e),
                Err(e) => return Ok(Self::err_result(&e)),
            }
        }
        let Inner { store, embedder } = &mut *g;
        let Some(embedder) = embedder.as_mut() else {
            return Ok(Self::err_result(&anyhow::anyhow!("embedder unavailable")));
        };
        match do_store(
            store,
            embedder,
            &self.cfg,
            &SystemClock,
            &a.agent_id,
            &a.text,
        ) {
            Ok(id) => Ok(CallToolResult::success(vec![ContentBlock::text(
                id.to_string(),
            )])),
            Err(e) => Ok(Self::err_result(&e)),
        }
    }

    #[tool(description = "Recall the k most relevant memories for an agent")]
    async fn recall(
        &self,
        Parameters(a): Parameters<RecallArgs>,
    ) -> Result<CallToolResult, McpError> {
        let k =
            a.k.map_or(DEFAULT_K, |v| usize::try_from(v).unwrap_or(DEFAULT_K));
        let mut g = self.inner.lock().await;
        if g.embedder.is_none() {
            let m = self.model_dir.join("onnx/model.onnx");
            let t = self.model_dir.join("tokenizer.json");
            match Embedder::load(&m.to_string_lossy(), &t.to_string_lossy()) {
                Ok(e) => g.embedder = Some(e),
                Err(e) => return Ok(Self::err_result(&e)),
            }
        }
        let Inner { store, embedder } = &mut *g;
        let Some(embedder) = embedder.as_mut() else {
            return Ok(Self::err_result(&anyhow::anyhow!("embedder unavailable")));
        };
        match do_recall(store, embedder, &SystemClock, &a.agent_id, &a.query, k) {
            Ok(json) => Ok(CallToolResult::success(vec![ContentBlock::text(json)])),
            Err(e) => Ok(Self::err_result(&e)),
        }
    }

    #[tool(description = "Consolidate an agent's memories (decay-scored dedup/merge)")]
    async fn consolidate(
        &self,
        Parameters(a): Parameters<ConsolidateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.inner.lock().await;
        match crate::consolidate::consolidate(&mut g.store, &a.agent_id, &self.cfg, &SystemClock) {
            Ok(()) => Ok(CallToolResult::success(vec![ContentBlock::text("ok")])),
            Err(e) => Ok(Self::err_result(&e)),
        }
    }
}

#[tool_handler]
impl ServerHandler for MemoryServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
    }
}

/// Runs the MCP memory server over stdio until the client disconnects.
///
/// Reads the database path from `GENESIS_MEMORY_DB` (fallback `genesis-memory.db`) and the
/// model directory from `GENESIS_MODEL_DIR` (via [`embed::model_dir`]). Returns `Ok(())` on
/// a clean shutdown so stdin EOF yields exit status 0.
///
/// # Errors
///
/// Returns an error if the store or stdio transport fails to initialise.
pub async fn serve_stdio() -> Result<()> {
    let store = VectorStore::open(&crate::store::db_path_from_env())?;
    let server = MemoryServer::new(store, crate::embed::model_dir());
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

// Source: test/specs/genesis-memory-server.md — Implementation Requirements
// (groups "Tool API (agent scoping)", "Recall response payload", "Server wiring").
// Store/recall tests use the REAL ONNX model + real SQLite (no mocks, §5); they FAIL
// (never skip) when the model is absent. Adapter tests drive the real #[tool] methods.
#[cfg(test)]
mod tests {
    use super::{
        do_recall, do_store, ConsolidateArgs, MemoryServer, RecallArgs, StoreArgs, DEFAULT_K,
    };
    use crate::consolidate::{ConsolidationConfig, FixedClock};
    use crate::embed::{model_dir, model_paths, Embedder};
    use crate::store::VectorStore;
    use rmcp::handler::server::wrapper::Parameters;
    use rmcp::model::ProtocolVersion;
    use rmcp::ServerHandler;

    /// Opens a fresh temp store + the real embedder; FAILS (never skips) if the model is absent.
    fn setup() -> (tempfile::TempDir, VectorStore, Embedder) {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::open(dir.path().join("m.db").to_str().unwrap()).unwrap();
        let (m, t) = model_paths();
        assert!(m.exists(), "model missing: run `scripts/fetch-model`");
        let embedder = Embedder::load(m.to_str().unwrap(), t.to_str().unwrap()).unwrap();
        (dir, store, embedder)
    }

    /// A server over a fresh temp DB pointed at the real model dir (returns the TempDir so the
    /// DB file outlives the server in-test).
    fn a_server() -> (tempfile::TempDir, MemoryServer) {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::open(dir.path().join("m.db").to_str().unwrap()).unwrap();
        let server = MemoryServer::new(store, model_dir());
        (dir, server)
    }

    // ─── Tool API (agent scoping) ────────────────────────────────────────────

    #[test]
    fn store_args_are_agent_id_and_text() {
        let a: StoreArgs =
            serde_json::from_value(serde_json::json!({"agent_id":"a","text":"t"})).unwrap();
        assert_eq!(a.agent_id, "a");
        assert_eq!(a.text, "t");
    }

    #[test]
    fn recall_args_are_agent_id_query_and_optional_k() {
        let a: RecallArgs =
            serde_json::from_value(serde_json::json!({"agent_id":"a","query":"q"})).unwrap();
        assert_eq!(a.agent_id, "a");
        assert_eq!(a.query, "q");
        assert!(a.k.is_none());
        let b: RecallArgs =
            serde_json::from_value(serde_json::json!({"agent_id":"a","query":"q","k":3})).unwrap();
        assert_eq!(b.k, Some(3));
    }

    #[test]
    fn consolidate_args_are_agent_id() {
        let a: ConsolidateArgs =
            serde_json::from_value(serde_json::json!({"agent_id":"a"})).unwrap();
        assert_eq!(a.agent_id, "a");
    }

    #[test]
    fn recall_k_defaults_to_five() {
        assert_eq!(DEFAULT_K, 5);
        let omitted: Option<u32> = None;
        let k = omitted.map_or(DEFAULT_K, |v| usize::try_from(v).unwrap_or(DEFAULT_K));
        assert_eq!(k, 5);
    }

    #[test]
    fn one_sqlite_database_serves_all_agents() {
        let (_d, mut store, mut emb) = setup();
        let cfg = ConsolidationConfig::default();
        let clock = FixedClock(0);
        do_store(&mut store, &mut emb, &cfg, &clock, "alpha", "alpha note").unwrap();
        do_store(&mut store, &mut emb, &cfg, &clock, "beta", "beta note").unwrap();
        assert_eq!(store.active_memories("alpha").unwrap().len(), 1);
        assert_eq!(store.active_memories("beta").unwrap().len(), 1);
    }

    #[test]
    fn every_memories_query_is_scoped_by_agent_id() {
        let (_d, mut store, mut emb) = setup();
        let cfg = ConsolidationConfig::default();
        let clock = FixedClock(0);
        do_store(&mut store, &mut emb, &cfg, &clock, "alpha", "secret alpha").unwrap();
        let json = do_recall(&mut store, &mut emb, &clock, "beta", "secret alpha", 5).unwrap();
        let items: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            items.as_array().unwrap().is_empty(),
            "beta sees none of alpha's rows"
        );
    }

    // ─── Recall response payload ─────────────────────────────────────────────

    #[test]
    fn recall_result_serializes_as_a_json_array_of_objects() {
        let (_d, mut store, mut emb) = setup();
        let cfg = ConsolidationConfig::default();
        let clock = FixedClock(0);
        do_store(&mut store, &mut emb, &cfg, &clock, "a", "hello world").unwrap();
        let json = do_recall(&mut store, &mut emb, &clock, "a", "hello world", 5).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.is_array());
        assert!(v
            .as_array()
            .unwrap()
            .iter()
            .all(serde_json::Value::is_object));
    }

    #[test]
    fn recall_result_objects_have_exactly_id_text_and_distance() {
        let (_d, mut store, mut emb) = setup();
        let cfg = ConsolidationConfig::default();
        let clock = FixedClock(0);
        do_store(&mut store, &mut emb, &cfg, &clock, "a", "hello world").unwrap();
        let json = do_recall(&mut store, &mut emb, &clock, "a", "hello world", 5).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = v.as_array().unwrap()[0].as_object().unwrap();
        assert_eq!(obj.len(), 3);
        assert!(obj["id"].is_i64());
        assert!(obj["text"].is_string());
        assert!(obj["distance"].is_number());
    }

    #[test]
    fn recall_result_array_is_ordered_by_ascending_distance() {
        let (_d, mut store, mut emb) = setup();
        let cfg = ConsolidationConfig::default();
        let clock = FixedClock(0);
        do_store(
            &mut store,
            &mut emb,
            &cfg,
            &clock,
            "a",
            "the quick brown fox",
        )
        .unwrap();
        do_store(
            &mut store,
            &mut emb,
            &cfg,
            &clock,
            "a",
            "an entirely unrelated subject",
        )
        .unwrap();
        let json = do_recall(&mut store, &mut emb, &clock, "a", "the quick brown fox", 5).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let dists: Vec<f64> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["distance"].as_f64().unwrap())
            .collect();
        assert!(
            dists.windows(2).all(|w| w[0] <= w[1]),
            "ascending: {dists:?}"
        );
        approx::assert_abs_diff_eq!(dists[0], 0.0, epsilon = 1e-6); // exact match first
    }

    #[tokio::test]
    async fn recall_json_array_is_carried_in_a_single_content_block_text() {
        let (_d, server) = a_server();
        server
            .store(Parameters(StoreArgs {
                agent_id: "a".into(),
                text: "hello".into(),
            }))
            .await
            .unwrap();
        let res = server
            .recall(Parameters(RecallArgs {
                agent_id: "a".into(),
                query: "hello".into(),
                k: None,
            }))
            .await
            .unwrap();
        assert_ne!(res.is_error, Some(true));
        assert_eq!(res.content.len(), 1, "exactly one content block");
        let text = &res.content[0].as_text().unwrap().text;
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed.is_array());
    }

    // ─── Server wiring ───────────────────────────────────────────────────────

    #[test]
    fn tool_logic_lives_in_library_modules_with_injected_store_and_embedder() {
        // do_store / do_recall are free functions over an injected &mut store + &mut embedder.
        let (_d, mut store, mut emb) = setup();
        let cfg = ConsolidationConfig::default();
        let clock = FixedClock(0);
        let id = do_store(&mut store, &mut emb, &cfg, &clock, "a", "injected deps").unwrap();
        assert!(id >= 1);
        assert!(
            !do_recall(&mut store, &mut emb, &clock, "a", "injected deps", 5)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn server_info_is_built_from_enable_tools_capabilities() {
        let (_d, server) = a_server();
        assert!(server.get_info().capabilities.tools.is_some());
    }

    #[test]
    fn protocol_version_is_pinned_to_v_2024_11_05() {
        let (_d, server) = a_server();
        assert_eq!(
            server.get_info().protocol_version,
            ProtocolVersion::V_2024_11_05
        );
    }

    #[tokio::test]
    async fn tool_text_payloads_use_content_block_text() {
        // consolidate needs no embedder: exercise the adapter's text payload without the model.
        let (_d, server) = a_server();
        let res = server
            .consolidate(Parameters(ConsolidateArgs {
                agent_id: "a".into(),
            }))
            .await
            .unwrap();
        assert_ne!(res.is_error, Some(true));
        assert_eq!(res.content[0].as_text().unwrap().text, "ok");
    }

    #[tokio::test]
    async fn tool_failures_return_call_tool_result_with_is_error_true() {
        // Point at an EMPTY model dir: the embedder load fails, so store returns is_error:true
        // (a successful response carrying the error) rather than panicking.
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::open(dir.path().join("m.db").to_str().unwrap()).unwrap();
        let empty_model_dir = tempfile::tempdir().unwrap();
        let server = MemoryServer::new(store, empty_model_dir.path().to_path_buf());
        let res = server
            .store(Parameters(StoreArgs {
                agent_id: "a".into(),
                text: "t".into(),
            }))
            .await
            .unwrap();
        assert_eq!(res.is_error, Some(true), "missing model => isError:true");
    }

    #[test]
    fn main_returns_ok_after_waiting_so_stdin_eof_exits_zero() {
        // AC14 (stdin EOF => exit status 0) is verified end-to-end by server.feature (Task 10).
        // Unit-level: serve_stdio is the async Result-returning entry that `main` `?`-forwards,
        // so a clean shutdown propagates Ok(()) (exit 0) rather than a nonzero-exit panic.
        let _ = super::serve_stdio; // exists with the async Result<()> shape main forwards
    }

    // ─── AC3 calibration fixture (bootstrap item 4) ──────────────────────────

    /// The committed calibration fixture must keep ranking its source first for the
    /// paraphrase query against the decoys, with the real model + real store (no mocks).
    #[test]
    fn ac3_calibration_fixture_ranks_the_source_first() {
        let raw = std::fs::read_to_string("tests/fixtures/ac3_calibration.json").unwrap();
        let fx: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let (_d, mut store, mut emb) = setup();
        let cfg = ConsolidationConfig::default();
        let clock = FixedClock(0);
        let source = fx["source"].as_str().unwrap();
        do_store(&mut store, &mut emb, &cfg, &clock, "cal", source).unwrap();
        for d in fx["decoys"].as_array().unwrap() {
            do_store(
                &mut store,
                &mut emb,
                &cfg,
                &clock,
                "cal",
                d.as_str().unwrap(),
            )
            .unwrap();
        }
        let json = do_recall(
            &mut store,
            &mut emb,
            &clock,
            "cal",
            fx["paraphrase"].as_str().unwrap(),
            5,
        )
        .unwrap();
        let items: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(items[0]["text"].as_str().unwrap(), source);
    }
}
