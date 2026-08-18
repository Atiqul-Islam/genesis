//! Memory eval harness (P5) — deterministic, store-level metrics for the 0.2.0 structured memory system.
//!
//! Runs against the REAL ONNX embedder (like the recall unit tests) and does two things at once: it ASSERTS
//! the load-bearing invariants (so a regression fails CI) AND prints a metrics report you can read with
//! `cargo test --test eval -- --nocapture`. It is deterministic — Mneme's structuring is simulated by
//! calling `structure_memory` directly, so there is no LLM in the loop.
//!
//! Metrics:
//! * **superseded-serve-rate** — how often recall returns a fact that has been superseded. MUST be 0 (the
//!   bi-temporal active-filter guarantees it; this proves it end-to-end through the hybrid pipeline).
//! * **active-contradiction-rate** — after supersession, does every `(subject, relation)` key resolve to
//!   exactly one current value (the newest)? MUST be 0 contradictions.
//! * **recall@1 / recall@3** — does the hybrid pipeline surface the CURRENT fact for a topical query? A
//!   quality metric (reported), gated loosely so it catches a broken pipeline without flaking on embeddings.
//! * **no-false-supersession-from-similarity** — two facts with near-identical embeddings but DIFFERENT
//!   subjects never supersede each other (deterministic keying, not similarity — MemStrata).

// Integration tests legitimately abort on failure; the strict restriction lints don't apply to test code.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use genesis_memory::embed::Embedder;
use genesis_memory::store::VectorStore;

/// A fact that changes over time: `subject relation` went from `old` to `new`.
struct Fact {
    subject: &'static str,
    relation: &'static str,
    old: &'static str,
    new: &'static str,
}

const FACTS: &[Fact] = &[
    Fact {
        subject: "ball",
        relation: "color",
        old: "red",
        new: "green",
    },
    Fact {
        subject: "sky",
        relation: "color",
        old: "gray",
        new: "blue",
    },
    Fact {
        subject: "car",
        relation: "brand",
        old: "toyota",
        new: "honda",
    },
    Fact {
        subject: "dog",
        relation: "name",
        old: "rex",
        new: "fido",
    },
    Fact {
        subject: "language",
        relation: "favorite",
        old: "python",
        new: "rust",
    },
    Fact {
        subject: "office",
        relation: "city",
        old: "paris",
        new: "london",
    },
];

const AGENT: &str = "eval";

/// `n / d` as `f64` without a lossy `as` cast (counts are tiny, so the clamp never triggers).
fn frac(n: usize, d: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(d.max(1)).unwrap_or(1))
}

/// Open a temp store + the real embedder. FAILS (never skips) if the model is absent — an eval that silently
/// skips is worse than none.
fn setup() -> (tempfile::TempDir, VectorStore, Embedder) {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::open(dir.path().join("m.db").to_str().unwrap()).unwrap();
    let (m, t) = genesis_memory::embed::model_paths();
    assert!(
        m.exists(),
        "model missing: run `node scripts/fetch-model.mjs`"
    );
    let embedder = Embedder::load(m.to_str().unwrap(), t.to_str().unwrap()).unwrap();
    (dir, store, embedder)
}

/// Insert every fact's OLD then NEW value and structure both, so each NEW supersedes its OLD (deterministic
/// keyed supersession). Returns `(new_ids, old_ids)` aligned to `FACTS`.
fn build_store(store: &mut VectorStore, emb: &mut Embedder) -> (Vec<i64>, Vec<i64>) {
    let mut new_ids = Vec::new();
    let mut old_ids = Vec::new();
    for (i, f) in FACTS.iter().enumerate() {
        let t = i64::try_from(i).unwrap_or(0);
        let old_text = format!("the {} {} is {}", f.subject, f.relation, f.old);
        let old_id = store
            .insert(
                AGENT,
                &old_text,
                &emb.embed(&old_text).unwrap(),
                1.0,
                100 + t,
            )
            .unwrap();
        store
            .structure_memory(
                AGENT,
                old_id,
                "semantic",
                Some(f.subject),
                Some(f.relation),
                Some(f.old),
            )
            .unwrap();

        let new_text = format!("the {} {} is {}", f.subject, f.relation, f.new);
        let new_id = store
            .insert(
                AGENT,
                &new_text,
                &emb.embed(&new_text).unwrap(),
                1.0,
                200 + t,
            )
            .unwrap();
        let retired = store
            .structure_memory(
                AGENT,
                new_id,
                "semantic",
                Some(f.subject),
                Some(f.relation),
                Some(f.new),
            )
            .unwrap();
        assert_eq!(
            retired, 1,
            "the new '{}' fact supersedes exactly its old value",
            f.subject
        );

        new_ids.push(new_id);
        old_ids.push(old_id);
    }
    (new_ids, old_ids)
}

#[test]
fn eval_superseded_serve_rate_and_recall_quality() {
    let (_d, mut store, mut emb) = setup();
    let (new_ids, old_ids) = build_store(&mut store, &mut emb);
    let superseded: std::collections::HashSet<i64> = old_ids.iter().copied().collect();
    let now = 10_000;

    let mut superseded_hits = 0usize;
    let mut recall_at_1 = 0usize;
    let mut recall_at_3 = 0usize;
    for (i, f) in FACTS.iter().enumerate() {
        let query = format!("{} {}", f.subject, f.relation);
        let qe = emb.embed(&query).unwrap();
        let hits = store.hybrid_recall(AGENT, &query, &qe, 3, now).unwrap();
        let ids: Vec<i64> = hits.iter().map(|(id, _)| *id).collect();

        // INVARIANT: a superseded fact must NEVER be served.
        if ids.iter().any(|id| superseded.contains(id)) {
            superseded_hits += 1;
        }
        // recall of the CURRENT fact for its topical query.
        if ids.first() == Some(&new_ids[i]) {
            recall_at_1 += 1;
        }
        if ids.contains(&new_ids[i]) {
            recall_at_3 += 1;
        }
    }

    let k = FACTS.len();
    let superseded_serve_rate = frac(superseded_hits, k);
    let r1 = frac(recall_at_1, k);
    let r3 = frac(recall_at_3, k);
    println!("\n=== Genesis memory eval ({k} evolving facts) ===");
    println!("superseded-serve-rate : {superseded_serve_rate:.3}  (target 0.000)");
    println!("recall@1              : {r1:.3}");
    println!("recall@3              : {r3:.3}");

    assert_eq!(
        superseded_hits, 0,
        "a superseded fact was served (rate {superseded_serve_rate:.3}) — the active-filter regressed"
    );
    assert!(
        r3 >= 5.0 / 6.0,
        "recall@3 too low ({r3:.3}) — the hybrid pipeline is not surfacing current facts"
    );
}

#[test]
fn eval_active_contradiction_rate_is_zero() {
    let (_d, mut store, mut emb) = setup();
    build_store(&mut store, &mut emb);

    // After supersession every (subject, relation) key must resolve to exactly one CURRENT value — the newest.
    let mut contradictions = 0usize;
    for f in FACTS {
        match store.active_object(AGENT, f.subject, f.relation).unwrap() {
            Some(obj) if obj == f.new => {}
            _ => contradictions += 1,
        }
    }
    println!(
        "\nactive-contradiction-rate: {contradictions}/{} (target 0)",
        FACTS.len()
    );
    assert_eq!(
        contradictions, 0,
        "a key resolved to a stale/ambiguous value after supersession"
    );
}

#[test]
fn eval_no_false_supersession_from_similarity() {
    let (_d, mut store, mut emb) = setup();
    // Two facts with near-identical embeddings AND the same object, but DIFFERENT subjects.
    let ball = store
        .insert(
            AGENT,
            "the ball is blue",
            &emb.embed("the ball is blue").unwrap(),
            1.0,
            100,
        )
        .unwrap();
    store
        .structure_memory(
            AGENT,
            ball,
            "semantic",
            Some("ball"),
            Some("color"),
            Some("blue"),
        )
        .unwrap();
    let sky = store
        .insert(
            AGENT,
            "the sky is blue",
            &emb.embed("the sky is blue").unwrap(),
            1.0,
            200,
        )
        .unwrap();
    let retired = store
        .structure_memory(
            AGENT,
            sky,
            "semantic",
            Some("sky"),
            Some("color"),
            Some("blue"),
        )
        .unwrap();

    assert_eq!(
        retired, 0,
        "a different-subject fact must NOT be superseded despite similar embeddings"
    );
    assert_eq!(
        store
            .active_object(AGENT, "ball", "color")
            .unwrap()
            .as_deref(),
        Some("blue")
    );
    assert_eq!(
        store
            .active_object(AGENT, "sky", "color")
            .unwrap()
            .as_deref(),
        Some("blue")
    );
    println!("\nno-false-supersession-from-similarity: PASS (both facts remain active)");
}
