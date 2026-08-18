//! BDD step definitions for `test/features/consolidate.feature`.
//!
//! Source: `test/specs/genesis-memory-server.md` — acceptance criteria 8 and 13.
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
use genesis_memory::consolidate::{consolidate, cosine_from_l2, ConsolidationConfig, FixedClock};
use genesis_memory::embed::Embedder;
use genesis_memory::store::VectorStore;
use genesis_memory::{do_recall, do_store};
use tempfile::TempDir;

/// Scenario-scoped state for `consolidate.feature`.
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

/// The shared subject and the two near-identical memory texts (cosine >= `tau_merge`).
const SUBJECT: &str = "the release goes out on friday";
const NEAR_A: &str = "the release goes out on friday";
const NEAR_B: &str = "the release goes out on friday.";

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
fn items(w: &ConsolidateWorld) -> &Vec<serde_json::Value> {
    w.last_recall.as_ref().unwrap().as_array().unwrap()
}

/// Stores the near-duplicate pair and asserts the fixture really is at/above `tau_merge`.
async fn store_pair(w: &mut ConsolidateWorld, agent: &str) {
    let cfg = w.config.clone().unwrap();
    let store = w.vector_store.as_mut().unwrap();
    let emb = w.embedder.as_mut().unwrap();
    let id_a = do_store(store, emb, &cfg, &FixedClock(0), agent, NEAR_A).unwrap();
    let id_b = do_store(store, emb, &cfg, &FixedClock(0), agent, NEAR_B).unwrap();
    // Confirm the fixture really is >= tau_merge (derive cosine from L2 via a self-knn).
    let ea = store.embedding_of(id_a).unwrap();
    let hit = store
        .knn(agent, &ea, 2)
        .unwrap()
        .into_iter()
        .find(|(id, _)| *id == id_b)
        .unwrap();
    assert!(
        cosine_from_l2(hit.1) >= cfg.tau_merge,
        "fixture below tau_merge: {}",
        cosine_from_l2(hit.1)
    );
    w.pair_ids = vec![id_a, id_b];
}

// ─── Given ───────────────────────────────────────────────────────────────────

#[given(regex = r"^a memory server with an empty database$")]
async fn a_memory_server_with_an_empty_database(w: &mut ConsolidateWorld) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("m.db");
    w.vector_store = Some(VectorStore::open(path.to_str().unwrap()).unwrap());
    let (m, t) = model();
    w.embedder = Some(Embedder::load(&m, &t).unwrap());
    w.config = Some(ConsolidationConfig::default());
    w.shared_subject = Some(SUBJECT.to_string());
    w.db_path = Some(path);
    w.db_dir = Some(dir);
}

#[given(
    regex = r#"^agent "([^"]*)" has stored two memories whose cosine similarity is at or above tau_merge$"#
)]
async fn agent_has_stored_a_near_duplicate_pair(w: &mut ConsolidateWorld, agent: String) {
    store_pair(w, &agent).await;
}

#[given(regex = r#"^agent "([^"]*)" has two memories that a consolidate call has merged$"#)]
async fn agent_has_two_merged_memories(w: &mut ConsolidateWorld, agent: String) {
    store_pair(w, &agent).await;
    let cfg = w.config.clone().unwrap();
    consolidate(
        w.vector_store.as_mut().unwrap(),
        &agent,
        &cfg,
        &FixedClock(0),
    )
    .unwrap();
    w.superseded_id = w
        .vector_store
        .as_ref()
        .unwrap()
        .superseded_ids(&agent)
        .unwrap()
        .first()
        .copied();
}

// ─── When ────────────────────────────────────────────────────────────────────

#[when(
    regex = r#"^agent "([^"]*)" recalls the shared subject of those two memories with k of (\d+)$"#
)]
async fn agent_recalls_the_shared_subject(w: &mut ConsolidateWorld, agent: String, k: u32) {
    let subject = w.shared_subject.clone().unwrap();
    let json = do_recall(
        w.vector_store.as_mut().unwrap(),
        w.embedder.as_mut().unwrap(),
        &FixedClock(0),
        &agent,
        &subject,
        k as usize,
    )
    .unwrap();
    w.last_recall = Some(serde_json::from_str(&json).unwrap());
}

#[when(regex = r#"^agent "([^"]*)" consolidates$"#)]
async fn agent_consolidates(w: &mut ConsolidateWorld, agent: String) {
    let cfg = w.config.clone().unwrap();
    consolidate(
        w.vector_store.as_mut().unwrap(),
        &agent,
        &cfg,
        &FixedClock(0),
    )
    .unwrap();
    w.superseded_id = w
        .vector_store
        .as_ref()
        .unwrap()
        .superseded_ids(&agent)
        .unwrap()
        .first()
        .copied();
}

// ─── Then ────────────────────────────────────────────────────────────────────

#[then(regex = r"^the recall result contains exactly (\d+) entr(?:y|ies)$")]
async fn the_recall_result_contains_exactly_n_entries(w: &mut ConsolidateWorld, n: usize) {
    assert_eq!(items(w).len(), n);
}

#[then(regex = r"^the recall result does not contain the superseded memory$")]
async fn the_recall_result_omits_the_superseded_memory(w: &mut ConsolidateWorld) {
    let sid = w.superseded_id.expect("a row was superseded");
    assert!(items(w).iter().all(|it| it["id"].as_i64() != Some(sid)));
}

#[then(regex = r"^no recall entry has the id of the row whose superseded_by is non-null$")]
async fn no_recall_entry_has_the_superseded_id(w: &mut ConsolidateWorld) {
    let sid = w.superseded_id.expect("a row was superseded");
    assert!(items(w).iter().all(|it| it["id"].as_i64() != Some(sid)));
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
