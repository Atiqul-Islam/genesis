//! BDD step definitions for `test/features/recall.feature`.
//!
//! Source: `test/specs/genesis-memory-server.md` — acceptance criteria 4, 5, 6, 7, 16.
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
use genesis_memory::embed::Embedder;
use genesis_memory::store::VectorStore;
use tempfile::TempDir;

/// Scenario-scoped state for `recall.feature`.
// Fields are populated only once the steps are implemented via TDD; at the RED stage
// they are write-only scaffold state.
#[allow(dead_code)]
#[derive(Debug, Default, cucumber::World)]
struct RecallWorld {
    /// Per-scenario temporary directory holding the SQLite database (§5 #6 hermeticity).
    db_dir: Option<TempDir>,
    /// Path to the per-scenario SQLite database inside [`RecallWorld::db_dir`].
    db_path: Option<PathBuf>,
    /// The real ONNX embedder, loaded from `server/models/` — never a mock.
    embedder: Option<Embedder>,
    /// The real `sqlite-vec` backed vector store opened at [`RecallWorld::db_path`].
    vector_store: Option<VectorStore>,
    /// The spawned `genesis-memory-server` child process, when a scenario drives stdio.
    server: Option<Child>,
    /// Texts stored during the scenario, in insertion order.
    stored_texts: Vec<String>,
    /// The raw string carried by the text content block of the last `recall` result.
    last_recall_text: Option<String>,
    /// The parsed JSON array returned by the most recent `recall` call.
    last_recall: Option<serde_json::Value>,
}

// ─── Given ───────────────────────────────────────────────────────────────────

#[given(regex = r"^a memory server with an empty database$")]
async fn a_memory_server_with_an_empty_database(_w: &mut RecallWorld) {
    // TODO: create a TempDir, point GENESIS_MEMORY_DB at a file inside it, and open the
    // real VectorStore + Embedder against it.
    unimplemented!("Implement via TDD — a memory server with an empty database");
}

#[given(regex = r#"^agent "([^"]*)" has stored (\d+) distinct memories$"#)]
async fn agent_has_stored_n_distinct_memories(_w: &mut RecallWorld, _agent: String, _n: usize) {
    // TODO: call the real `store` tool n times with distinct texts.
    unimplemented!("Implement via TDD — agent has stored N distinct memories");
}

#[given(regex = r#"^agent "([^"]*)" has stored the memory "([^"]*)"$"#)]
async fn agent_has_stored_the_memory(_w: &mut RecallWorld, _agent: String, _text: String) {
    // TODO: call the real `store` tool with the given agent_id and text.
    unimplemented!("Implement via TDD — agent has stored the given memory");
}

// ─── When ────────────────────────────────────────────────────────────────────

#[when(regex = r#"^agent "([^"]*)" recalls "([^"]*)" with k of (\d+)$"#)]
async fn agent_recalls_with_k(_w: &mut RecallWorld, _agent: String, _query: String, _k: u32) {
    // TODO: call the real `recall` tool with an explicit k and capture the content block.
    unimplemented!("Implement via TDD — agent recalls the query with an explicit k");
}

#[when(regex = r#"^agent "([^"]*)" recalls "([^"]*)" without k$"#)]
async fn agent_recalls_without_k(_w: &mut RecallWorld, _agent: String, _query: String) {
    // TODO: call the real `recall` tool with the k argument omitted (default 5).
    unimplemented!("Implement via TDD — agent recalls the query without k");
}

// ─── Then ────────────────────────────────────────────────────────────────────

#[then(regex = r"^the recall result contains exactly (\d+) entr(?:y|ies)$")]
async fn the_recall_result_contains_exactly_n_entries(_w: &mut RecallWorld, _n: usize) {
    unimplemented!("Implement via TDD — the recall result contains exactly N entries");
}

#[then(regex = r"^the distance values in the recall result are non-decreasing$")]
async fn distances_are_non_decreasing(_w: &mut RecallWorld) {
    unimplemented!("Implement via TDD — the distance values in the recall result are non-decreasing");
}

#[then(regex = r#"^the first recall entry has text "([^"]*)"$"#)]
async fn the_first_recall_entry_has_text(_w: &mut RecallWorld, _text: String) {
    unimplemented!("Implement via TDD — the first recall entry has the given text");
}

#[then(regex = r"^the first recall entry has distance exactly ([0-9.]+)$")]
async fn the_first_recall_entry_has_distance(_w: &mut RecallWorld, _distance: f64) {
    unimplemented!("Implement via TDD — the first recall entry has exactly the given distance");
}

#[then(regex = r#"^no recall entry has text "([^"]*)"$"#)]
async fn no_recall_entry_has_text(_w: &mut RecallWorld, _text: String) {
    unimplemented!("Implement via TDD — no recall entry has the given text");
}

#[then(regex = r"^the text content block of the recall result parses as a JSON array$")]
async fn the_content_block_parses_as_a_json_array(_w: &mut RecallWorld) {
    unimplemented!("Implement via TDD — the text content block of the recall result parses as a JSON array");
}

#[then(regex = r"^every element of that array has exactly the keys id and text and distance$")]
async fn every_element_has_exactly_the_three_keys(_w: &mut RecallWorld) {
    unimplemented!("Implement via TDD — every element of that array has exactly the keys id, text and distance");
}

#[then(regex = r"^in every element id is an integer, text is a string and distance is a number$")]
async fn every_element_has_the_right_types(_w: &mut RecallWorld) {
    unimplemented!("Implement via TDD — in every element id is an integer, text is a string and distance is a number");
}

// ─── Runner ──────────────────────────────────────────────────────────────────

// The feature files live at repo-root `test/features/`; a `[[test]]` target runs with the
// package root (`server/`) as its working directory, hence the `../` prefix.
#[tokio::main]
async fn main() {
    RecallWorld::cucumber()
        .run_and_exit("../test/features/recall.feature")
        .await;
}
