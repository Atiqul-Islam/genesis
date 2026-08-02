//! BDD step definitions for `test/features/store.feature`.
//!
//! Source: `test/specs/genesis-memory-server.md` — acceptance criteria 3, 9, 10.
//!
//! No mocks (`docs/SPEC_FORGE_RUST_UPDATE.md` §5 #2): real SQLite + `sqlite-vec`, the real
//! ONNX embedder, and the real server binary. Hermeticity (§5 #6): the `World` owns a
//! `tempfile::TempDir`, so every scenario gets a fresh SQLite database.

// These are `harness = false` test binaries, which clippy's `allow-unwrap-in-tests`
// (server/clippy.toml) does not reach — unwrap/expect-on-failure IS the intended test
// behaviour (a failed unwrap is a failed scenario). Every cucumber step is `async` by
// convention even when it does not await.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::unused_async)]

use std::path::PathBuf;
use std::process::Child;

use cucumber::{given, then, when, World as _};
use genesis_memory::consolidate::{ConsolidationConfig, FixedClock};
use genesis_memory::embed::Embedder;
use genesis_memory::store::VectorStore;
use genesis_memory::{do_recall, do_store};
use tempfile::TempDir;

/// Scenario-scoped state for `store.feature`.
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
    /// Error labels collected from calls that are expected to fail.
    errors: Vec<String>,
}

/// Resolves the real model + tokenizer paths, FAILING (never skipping) if the model is absent.
fn model() -> (String, String) {
    let (m, t) = genesis_memory::embed::model_paths();
    assert!(m.exists(), "model missing: run `node scripts/fetch-model.mjs`");
    (
        m.to_string_lossy().into_owned(),
        t.to_string_lossy().into_owned(),
    )
}

/// Loads the committed AC3 calibration fixture (bootstrap item 4).
fn ac3() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string("tests/fixtures/ac3_calibration.json").unwrap())
        .unwrap()
}

// ─── Given ───────────────────────────────────────────────────────────────────

#[given(regex = r"^a memory server with an empty database$")]
async fn a_memory_server_with_an_empty_database(w: &mut StoreWorld) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("m.db");
    let store = VectorStore::open(path.to_str().unwrap()).unwrap();
    let (m, t) = model();
    w.embedder = Some(Embedder::load(&m, &t).unwrap());
    w.vector_store = Some(store);
    w.db_path = Some(path);
    w.db_dir = Some(dir);
}

#[given(regex = r#"^agent "([^"]*)" has stored the calibrated source text$"#)]
async fn agent_has_stored_the_calibrated_source_text(w: &mut StoreWorld, agent: String) {
    let fx = ac3();
    let src = fx["source"].as_str().unwrap().to_string();
    let cfg = ConsolidationConfig::default();
    do_store(
        w.vector_store.as_mut().unwrap(),
        w.embedder.as_mut().unwrap(),
        &cfg,
        &FixedClock(0),
        &agent,
        &src,
    )
    .unwrap();
    w.agent_id = Some(agent);
    w.fixture_texts.push(src);
}

#[given(regex = r#"^agent "([^"]*)" has stored the calibrated dissimilar decoy texts$"#)]
async fn agent_has_stored_the_calibrated_decoys(w: &mut StoreWorld, agent: String) {
    let fx = ac3();
    let cfg = ConsolidationConfig::default();
    for d in fx["decoys"].as_array().unwrap() {
        do_store(
            w.vector_store.as_mut().unwrap(),
            w.embedder.as_mut().unwrap(),
            &cfg,
            &FixedClock(0),
            &agent,
            d.as_str().unwrap(),
        )
        .unwrap();
    }
}

#[given(regex = r"^a loaded embedder$")]
async fn a_loaded_embedder(w: &mut StoreWorld) {
    let (m, t) = model();
    w.embedder = Some(Embedder::load(&m, &t).unwrap());
}

#[given(regex = r"^an open vector store$")]
async fn an_open_vector_store(w: &mut StoreWorld) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("m.db");
    w.vector_store = Some(VectorStore::open(path.to_str().unwrap()).unwrap());
    w.db_path = Some(path);
    w.db_dir = Some(dir);
}

// ─── When ────────────────────────────────────────────────────────────────────

#[when(regex = r#"^agent "([^"]*)" recalls with the calibrated paraphrase of the source text$"#)]
async fn agent_recalls_with_the_calibrated_paraphrase(w: &mut StoreWorld, agent: String) {
    let fx = ac3();
    let json = do_recall(
        w.vector_store.as_mut().unwrap(),
        w.embedder.as_mut().unwrap(),
        &FixedClock(0),
        &agent,
        fx["paraphrase"].as_str().unwrap(),
        5,
    )
    .unwrap();
    w.last_recall = Some(serde_json::from_str(&json).unwrap());
}

#[when(regex = r#"^the embedder embeds "([^"]*)" twice in the same process$"#)]
async fn the_embedder_embeds_twice(w: &mut StoreWorld, text: String) {
    let e = w.embedder.as_mut().unwrap();
    w.vectors.push(e.embed(&text).unwrap());
    w.vectors.push(e.embed(&text).unwrap());
}

#[when(regex = r"^a (\d+) element vector is passed to VectorStore insert and VectorStore knn$")]
async fn a_wrong_length_vector_is_passed(w: &mut StoreWorld, len: usize) {
    let store = w.vector_store.as_mut().unwrap();
    let v = vec![0.0f32; len];
    if store.insert("alpha", "x", &v, 1.0, 0).is_err() {
        w.errors.push("insert".into());
    }
    if store.knn("alpha", &v, 5).is_err() {
        w.errors.push("knn".into());
    }
}

// ─── Then ────────────────────────────────────────────────────────────────────

#[then(regex = r"^the recall result contains an entry whose text is the calibrated source text$")]
async fn recall_contains_the_calibrated_source_text(w: &mut StoreWorld) {
    let src = ac3()["source"].as_str().unwrap().to_string();
    let items = w.last_recall.as_ref().unwrap().as_array().unwrap();
    assert!(items.iter().any(|it| it["text"] == src));
}

#[then(regex = r"^that entry is first in the recall result$")]
async fn that_entry_is_first(w: &mut StoreWorld) {
    let src = ac3()["source"].as_str().unwrap().to_string();
    let items = w.last_recall.as_ref().unwrap().as_array().unwrap();
    assert_eq!(items[0]["text"], src);
}

#[then(regex = r"^the two vectors have cosine similarity of at least ([0-9.]+)$")]
async fn the_two_vectors_have_cosine_at_least(w: &mut StoreWorld, cosine: f64) {
    let (a, b) = (&w.vectors[0], &w.vectors[1]);
    let dot: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let na: f64 = a
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();
    let nb: f64 = b
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();
    assert!(dot / (na * nb) >= cosine);
}

#[then(regex = r"^both calls return an error$")]
async fn both_calls_return_an_error(w: &mut StoreWorld) {
    assert!(w.errors.contains(&"insert".to_string()) && w.errors.contains(&"knn".to_string()));
}

#[then(regex = r"^the test process has not panicked$")]
async fn the_test_process_has_not_panicked(_w: &mut StoreWorld) {
    // Reaching this step at all proves no panic occurred above.
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
