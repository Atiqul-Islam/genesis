//! BDD step definitions for `test/features/store.feature`.
//!
//! Source: `test/specs/genesis-memory-server.md` — acceptance criteria 3, 9, 10.
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

/// Scenario-scoped state for `store.feature`.
// Fields are populated only once the steps are implemented via TDD; at the RED stage
// they are write-only scaffold state.
#[allow(dead_code)]
#[derive(Debug, Default, cucumber::World)]
struct StoreWorld {
    /// Per-scenario temporary directory holding the SQLite database (§5 #6 hermeticity).
    db_dir: Option<TempDir>,
    /// Path to the per-scenario SQLite database inside [`StoreWorld::db_dir`].
    db_path: Option<PathBuf>,
    /// The real ONNX embedder, loaded from `server/models/` — never a mock.
    embedder: Option<Embedder>,
    /// The real `sqlite-vec` backed vector store opened at [`StoreWorld::db_path`].
    vector_store: Option<VectorStore>,
    /// The spawned `genesis-memory-server` child process, when a scenario drives stdio.
    server: Option<Child>,
    /// The agent id most recently used by a step.
    agent_id: Option<String>,
    /// The calibrated source / paraphrase / decoy fixture texts (calibration item 4).
    fixture_texts: Vec<String>,
    /// Vectors produced by the embedder during the scenario.
    vectors: Vec<Vec<f32>>,
    /// The parsed JSON array returned by the most recent `recall` call.
    last_recall: Option<serde_json::Value>,
    /// Error strings collected from calls that are expected to fail.
    errors: Vec<String>,
}

// ─── Given ───────────────────────────────────────────────────────────────────

#[given(regex = r"^a memory server with an empty database$")]
async fn a_memory_server_with_an_empty_database(_w: &mut StoreWorld) {
    // TODO: create a TempDir, point GENESIS_MEMORY_DB at a file inside it, and open the
    // real VectorStore + Embedder against it.
    unimplemented!("Implement via TDD — a memory server with an empty database");
}

#[given(regex = r#"^agent "([^"]*)" has stored the calibrated source text$"#)]
async fn agent_has_stored_the_calibrated_source_text(_w: &mut StoreWorld, _agent: String) {
    // TODO: call the real `store` tool with the committed calibration fixture source text.
    unimplemented!("Implement via TDD — agent has stored the calibrated source text");
}

#[given(regex = r#"^agent "([^"]*)" has stored the calibrated dissimilar decoy texts$"#)]
async fn agent_has_stored_the_calibrated_decoys(_w: &mut StoreWorld, _agent: String) {
    // TODO: store the committed deliberately-dissimilar decoy texts (calibration item 4).
    unimplemented!("Implement via TDD — agent has stored the calibrated dissimilar decoy texts");
}

#[given(regex = r"^a loaded embedder$")]
async fn a_loaded_embedder(_w: &mut StoreWorld) {
    // TODO: Embedder::load(model.onnx, tokenizer.json) — fail (never skip) if absent,
    // directing the developer to run scripts/fetch-model.
    unimplemented!("Implement via TDD — a loaded embedder");
}

#[given(regex = r"^an open vector store$")]
async fn an_open_vector_store(_w: &mut StoreWorld) {
    // TODO: VectorStore::open against the per-scenario temp database.
    unimplemented!("Implement via TDD — an open vector store");
}

// ─── When ────────────────────────────────────────────────────────────────────

#[when(regex = r#"^agent "([^"]*)" recalls with the calibrated paraphrase of the source text$"#)]
async fn agent_recalls_with_the_calibrated_paraphrase(_w: &mut StoreWorld, _agent: String) {
    // TODO: call the real `recall` tool with the committed paraphrase fixture.
    unimplemented!("Implement via TDD — agent recalls with the calibrated paraphrase of the source text");
}

#[when(regex = r#"^the embedder embeds "([^"]*)" twice in the same process$"#)]
async fn the_embedder_embeds_twice(_w: &mut StoreWorld, _text: String) {
    // TODO: call Embedder::embed twice on the same text, pushing both vectors into `vectors`.
    unimplemented!("Implement via TDD — the embedder embeds the same text twice in the same process");
}

#[when(regex = r"^a (\d+) element vector is passed to VectorStore insert and VectorStore knn$")]
async fn a_wrong_length_vector_is_passed(_w: &mut StoreWorld, _len: usize) {
    // TODO: call VectorStore::insert and VectorStore::knn with a vector of the given length
    // and collect both Results — neither call may panic.
    unimplemented!("Implement via TDD — a wrong-length vector is passed to VectorStore insert and knn");
}

// ─── Then ────────────────────────────────────────────────────────────────────

#[then(regex = r"^the recall result contains an entry whose text is the calibrated source text$")]
async fn recall_contains_the_calibrated_source_text(_w: &mut StoreWorld) {
    unimplemented!("Implement via TDD — the recall result contains an entry whose text is the calibrated source text");
}

#[then(regex = r"^that entry is first in the recall result$")]
async fn that_entry_is_first(_w: &mut StoreWorld) {
    unimplemented!("Implement via TDD — that entry is first in the recall result");
}

#[then(regex = r"^the two vectors have cosine similarity of at least ([0-9.]+)$")]
async fn the_two_vectors_have_cosine_at_least(_w: &mut StoreWorld, _cosine: f64) {
    unimplemented!("Implement via TDD — the two vectors have cosine similarity of at least the given bound");
}

#[then(regex = r"^both calls return an error$")]
async fn both_calls_return_an_error(_w: &mut StoreWorld) {
    unimplemented!("Implement via TDD — both calls return an error");
}

#[then(regex = r"^the test process has not panicked$")]
async fn the_test_process_has_not_panicked(_w: &mut StoreWorld) {
    unimplemented!("Implement via TDD — the test process has not panicked");
}

// ─── Runner ──────────────────────────────────────────────────────────────────

// The feature files live at repo-root `test/features/`; a `[[test]]` target runs with the
// package root (`server/`) as its working directory, hence the `../` prefix.
#[tokio::main]
async fn main() {
    StoreWorld::cucumber()
        .run_and_exit("../test/features/store.feature")
        .await;
}
