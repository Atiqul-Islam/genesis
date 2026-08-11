//! BDD step definitions for `test/features/recall.feature`.
//!
//! Source: `test/specs/genesis-memory-server.md` — acceptance criteria 4, 5, 6, 7, 16.
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

/// Scenario-scoped state for `recall.feature`.
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

/// Resolves the real model + tokenizer paths, FAILING (never skipping) if the model is absent.
fn model() -> (String, String) {
    let (m, t) = genesis_memory::embed::model_paths();
    assert!(
        m.exists(),
        "model missing: run `node scripts/fetch-model.mjs`"
    );
    (
        m.to_string_lossy().into_owned(),
        t.to_string_lossy().into_owned(),
    )
}

/// The parsed entries of the most recent recall result.
fn items(w: &RecallWorld) -> &Vec<serde_json::Value> {
    w.last_recall.as_ref().unwrap().as_array().unwrap()
}

// ─── Given ───────────────────────────────────────────────────────────────────

#[given(regex = r"^a memory server with an empty database$")]
async fn a_memory_server_with_an_empty_database(w: &mut RecallWorld) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("m.db");
    w.vector_store = Some(VectorStore::open(path.to_str().unwrap()).unwrap());
    let (m, t) = model();
    w.embedder = Some(Embedder::load(&m, &t).unwrap());
    w.db_path = Some(path);
    w.db_dir = Some(dir);
}

#[given(regex = r#"^agent "([^"]*)" has stored (\d+) distinct memories$"#)]
async fn agent_has_stored_n_distinct_memories(w: &mut RecallWorld, agent: String, n: usize) {
    let cfg = ConsolidationConfig::default();
    let subjects = [
        "morning routine",
        "deploy schedule",
        "grocery list",
        "meeting notes",
        "vacation plan",
        "book summary",
        "workout plan",
        "recipe idea",
        "budget review",
        "travel log",
    ];
    for i in 0..n {
        let text = format!("{} number {i}", subjects[i % subjects.len()]);
        do_store(
            w.vector_store.as_mut().unwrap(),
            w.embedder.as_mut().unwrap(),
            &cfg,
            &FixedClock(0),
            &agent,
            &text,
        )
        .unwrap();
        w.stored_texts.push(text);
    }
}

#[given(regex = r#"^agent "([^"]*)" has stored the memory "([^"]*)"$"#)]
async fn agent_has_stored_the_memory(w: &mut RecallWorld, agent: String, text: String) {
    let cfg = ConsolidationConfig::default();
    do_store(
        w.vector_store.as_mut().unwrap(),
        w.embedder.as_mut().unwrap(),
        &cfg,
        &FixedClock(0),
        &agent,
        &text,
    )
    .unwrap();
    w.stored_texts.push(text);
}

// ─── When ────────────────────────────────────────────────────────────────────

#[when(regex = r#"^agent "([^"]*)" recalls "([^"]*)" with k of (\d+)$"#)]
async fn agent_recalls_with_k(w: &mut RecallWorld, agent: String, query: String, k: u32) {
    let json = do_recall(
        w.vector_store.as_mut().unwrap(),
        w.embedder.as_mut().unwrap(),
        &FixedClock(0),
        &agent,
        &query,
        k as usize,
    )
    .unwrap();
    w.last_recall = Some(serde_json::from_str(&json).unwrap());
    w.last_recall_text = Some(json);
}

#[when(regex = r#"^agent "([^"]*)" recalls "([^"]*)" without k$"#)]
async fn agent_recalls_without_k(w: &mut RecallWorld, agent: String, query: String) {
    let k = genesis_memory::DEFAULT_K; // omitted k ⇒ 5
    let json = do_recall(
        w.vector_store.as_mut().unwrap(),
        w.embedder.as_mut().unwrap(),
        &FixedClock(0),
        &agent,
        &query,
        k,
    )
    .unwrap();
    w.last_recall = Some(serde_json::from_str(&json).unwrap());
    w.last_recall_text = Some(json);
}

// ─── Then ────────────────────────────────────────────────────────────────────

#[then(regex = r"^the recall result contains exactly (\d+) entr(?:y|ies)$")]
async fn the_recall_result_contains_exactly_n_entries(w: &mut RecallWorld, n: usize) {
    assert_eq!(items(w).len(), n);
}

#[then(regex = r"^the distance values in the recall result are non-decreasing$")]
async fn distances_are_non_decreasing(w: &mut RecallWorld) {
    let ds: Vec<f64> = items(w)
        .iter()
        .map(|it| it["distance"].as_f64().unwrap())
        .collect();
    for win in ds.windows(2) {
        assert!(win[0] <= win[1]);
    }
}

#[then(regex = r#"^the first recall entry has text "([^"]*)"$"#)]
async fn the_first_recall_entry_has_text(w: &mut RecallWorld, text: String) {
    assert_eq!(items(w)[0]["text"], text);
}

#[then(regex = r"^the first recall entry has distance exactly ([0-9.]+)$")]
async fn the_first_recall_entry_has_distance(w: &mut RecallWorld, distance: f64) {
    approx::assert_abs_diff_eq!(
        items(w)[0]["distance"].as_f64().unwrap(),
        distance,
        epsilon = 1e-6
    );
}

#[then(regex = r#"^no recall entry has text "([^"]*)"$"#)]
async fn no_recall_entry_has_text(w: &mut RecallWorld, text: String) {
    assert!(items(w).iter().all(|it| it["text"] != text));
}

#[then(regex = r"^the text content block of the recall result parses as a JSON array$")]
async fn the_content_block_parses_as_a_json_array(w: &mut RecallWorld) {
    let v: serde_json::Value = serde_json::from_str(w.last_recall_text.as_ref().unwrap()).unwrap();
    assert!(v.is_array());
}

#[then(regex = r"^every element of that array has exactly the keys id and text and distance$")]
async fn every_element_has_exactly_the_three_keys(w: &mut RecallWorld) {
    for it in items(w) {
        let o = it.as_object().unwrap();
        assert_eq!(o.len(), 3);
        assert!(o.contains_key("id") && o.contains_key("text") && o.contains_key("distance"));
    }
}

#[then(regex = r"^in every element id is an integer, text is a string and distance is a number$")]
async fn every_element_has_the_right_types(w: &mut RecallWorld) {
    for it in items(w) {
        assert!(it["id"].is_i64() && it["text"].is_string() && it["distance"].is_number());
    }
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
