//! BDD step definitions for `test/features/consolidate.feature`.
//!
//! Source: `test/specs/genesis-memory-server.md` — acceptance criteria 8 and 13.
//!
//! No mocks (`docs/SPEC_FORGE_RUST_UPDATE.md` §5 #2): real SQLite + `sqlite-vec`, the real
//! ONNX embedder, and the real server binary. Hermeticity (§5 #6): the `World` owns a
//! `tempfile::TempDir`, so every scenario gets a fresh SQLite database.
//!
//! Every step body is `unimplemented!()` — it compiles (crate sets clippy
//! `unimplemented = "warn"`) and panics at runtime, which is the healthy RED state.

use std::path::PathBuf;
use std::process::Child;

use cucumber::{World as _, given, then, when};
use genesis_memory::consolidate::ConsolidationConfig;
use genesis_memory::embed::Embedder;
use genesis_memory::store::VectorStore;
use tempfile::TempDir;

/// Scenario-scoped state for `consolidate.feature`.
// Fields are populated only once the steps are implemented via TDD; at the RED stage
// they are write-only scaffold state.
#[allow(dead_code)]
#[derive(Debug, Default, cucumber::World)]
struct ConsolidateWorld {
    /// Per-scenario temporary directory holding the SQLite database (§5 #6 hermeticity).
    db_dir: Option<TempDir>,
    /// Path to the per-scenario SQLite database inside [`ConsolidateWorld::db_dir`].
    db_path: Option<PathBuf>,
    /// The real ONNX embedder, loaded from `server/models/` — never a mock.
    embedder: Option<Embedder>,
    /// The real `sqlite-vec` backed vector store opened at [`ConsolidateWorld::db_path`].
    vector_store: Option<VectorStore>,
    /// The spawned `genesis-memory-server` child process, when a scenario drives stdio.
    server: Option<Child>,
    /// The thresholds the scenario pins (`tau_merge`, `lambda`, `beta`, `base_score`, `cap`).
    config: Option<ConsolidationConfig>,
    /// The shared subject text of the near-duplicate pair, used as the recall query.
    shared_subject: Option<String>,
    /// Ids of the near-duplicate pair, in insertion order.
    pair_ids: Vec<i64>,
    /// The id of the row whose `superseded_by` became non-null after `consolidate`.
    superseded_id: Option<i64>,
    /// The parsed JSON array returned by the most recent `recall` call.
    last_recall: Option<serde_json::Value>,
}

// ─── Given ───────────────────────────────────────────────────────────────────

#[given(regex = r"^a memory server with an empty database$")]
async fn a_memory_server_with_an_empty_database(_w: &mut ConsolidateWorld) {
    // TODO: create a TempDir, point GENESIS_MEMORY_DB at a file inside it, and open the
    // real VectorStore + Embedder against it.
    unimplemented!("Implement via TDD — a memory server with an empty database");
}

#[given(
    regex = r#"^agent "([^"]*)" has stored two memories whose cosine similarity is at or above tau_merge$"#
)]
async fn agent_has_stored_a_near_duplicate_pair(_w: &mut ConsolidateWorld, _agent: String) {
    // TODO: store the committed near-duplicate fixture pair and assert their cosine,
    // derived from L2 as 1 - L2^2 / 2, is >= cfg.tau_merge.
    unimplemented!("Implement via TDD — agent has stored two memories whose cosine similarity is at or above tau_merge");
}

#[given(regex = r#"^agent "([^"]*)" has two memories that a consolidate call has merged$"#)]
async fn agent_has_two_merged_memories(_w: &mut ConsolidateWorld, _agent: String) {
    // TODO: store the near-duplicate pair, run consolidate, and record the superseded id.
    unimplemented!("Implement via TDD — agent has two memories that a consolidate call has merged");
}

// ─── When ────────────────────────────────────────────────────────────────────

#[when(
    regex = r#"^agent "([^"]*)" recalls the shared subject of those two memories with k of (\d+)$"#
)]
async fn agent_recalls_the_shared_subject(
    _w: &mut ConsolidateWorld,
    _agent: String,
    _k: u32,
) {
    // TODO: call the real `recall` tool with the shared subject as the query.
    unimplemented!("Implement via TDD — agent recalls the shared subject of those two memories");
}

#[when(regex = r#"^agent "([^"]*)" consolidates$"#)]
async fn agent_consolidates(_w: &mut ConsolidateWorld, _agent: String) {
    // TODO: call the real `consolidate` tool for the given agent_id.
    unimplemented!("Implement via TDD — agent consolidates");
}

// ─── Then ────────────────────────────────────────────────────────────────────

#[then(regex = r"^the recall result contains exactly (\d+) entr(?:y|ies)$")]
async fn the_recall_result_contains_exactly_n_entries(_w: &mut ConsolidateWorld, _n: usize) {
    unimplemented!("Implement via TDD — the recall result contains exactly N entries");
}

#[then(regex = r"^the recall result does not contain the superseded memory$")]
async fn the_recall_result_omits_the_superseded_memory(_w: &mut ConsolidateWorld) {
    unimplemented!("Implement via TDD — the recall result does not contain the superseded memory");
}

#[then(regex = r"^no recall entry has the id of the row whose superseded_by is non-null$")]
async fn no_recall_entry_has_the_superseded_id(_w: &mut ConsolidateWorld) {
    unimplemented!("Implement via TDD — no recall entry has the id of the row whose superseded_by is non-null");
}

// ─── Runner ──────────────────────────────────────────────────────────────────

// The feature files live at repo-root `test/features/`; a `[[test]]` target runs with the
// package root (`server/`) as its working directory, hence the `../` prefix.
#[tokio::main]
async fn main() {
    ConsolidateWorld::cucumber()
        .run_and_exit("../test/features/consolidate.feature")
        .await;
}
