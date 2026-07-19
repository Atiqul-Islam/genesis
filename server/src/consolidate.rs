//! Memory consolidation — decay/recency scoring and dedup/merge (v1 scope).
//!
//! v1 implements only the two sourced halves of §2.4:
//! - **Decay/recency:** `effective = base_score * exp(-lambda*age_days) * (1 + beta*ln(1+use_count))`,
//!   with `age_days` measured from `created_at` (spec D2).
//! - **Dedup/merge:** KNN top-1 within `agent_id`; cosine `>= tau_merge` merges into the
//!   higher-`effective` row (sum `use_count`, set the loser's `superseded_by`, no new vector row).
//!
//! **Deliberate deviation (spec D8):** §2.4 titles the merge rule "Dedup/compress *on insert*";
//! v1 runs it only inside an explicit `consolidate` call, never as part of `store`.
//!
//! **Out of scope (v2):** summarize/evict — its thresholds (`THETA_EVICT`, `MIN_AGE`, `TAU_SIM`,
//! `K`, `E%`) are unsourced (§6.2 #8). `cap` is retained in config as the future eviction trigger,
//! unused in v1.
//!
//! Every threshold is config so tests can pin it; `now` is injected via a [`Clock`] for determinism.

use anyhow::Result;

use crate::store::{MemRow, VectorStore};

/// Tunable consolidation thresholds. All values are placeholders to calibrate via tests.
#[derive(Debug, Clone)]
pub struct ConsolidationConfig {
    /// Decay constant λ (default `ln2/30` ⇒ 30-day half-life).
    pub lambda: f64,
    /// Use-count weight β.
    pub beta: f64,
    /// Cosine similarity at/above which two memories merge.
    pub tau_merge: f64,
    /// Score written into every new `memories` row at store time (chosen normalization = 1.0).
    pub base_score: f64,
    /// Row-count cap that triggers eviction (v2; unused in v1).
    pub cap: usize,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            lambda: core::f64::consts::LN_2 / 30.0,
            beta: 0.15,
            tau_merge: 0.95,
            base_score: 1.0,
            cap: 10_000,
        }
    }
}

/// Injectable wall clock (Unix seconds) — logic never reads the system clock directly.
pub trait Clock: Send + Sync {
    /// The current time as Unix seconds.
    fn now_unix(&self) -> i64;
}

/// Production clock backed by the system time.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
    }
}

/// Deterministic clock for tests — always returns the wrapped instant.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.0
    }
}

/// `effective = base_score * exp(-lambda * age_days) * (1 + beta * ln(1 + use_count))`,
/// with `age_days` measured from `created_at` (spec D2: recency already enters via the
/// `use_count` term, so age is true age, not last-used age).
#[must_use]
// Timestamps and use-counts are well under 2^53, so the i64→f64 conversions are exact here.
#[allow(clippy::cast_precision_loss)]
pub fn effective(
    cfg: &ConsolidationConfig,
    base_score: f64,
    created_at_unix: i64,
    now_unix: i64,
    use_count: i64,
) -> f64 {
    let age_days = (now_unix - created_at_unix) as f64 / 86_400.0;
    base_score * (-cfg.lambda * age_days).exp() * (1.0 + cfg.beta * (1.0 + use_count as f64).ln())
}

/// Cosine similarity from a `vec0` L2 (Euclidean) distance, valid for L2-normalized vectors.
#[must_use]
pub fn cosine_from_l2(l2_distance: f64) -> f64 {
    1.0 - l2_distance * l2_distance / 2.0
}

/// A mergeable pair discovered during a consolidation pass.
struct Merge {
    survivor: i64,
    loser: i64,
    loser_use_count: i64,
}

/// Builds a [`Merge`] if `mem` and its neighbour `nid` (at L2 distance `dist`) are a
/// mergeable pair — distinct rows whose cosine similarity is at or above `tau_merge` — with
/// the higher-`effective` row kept as survivor. Returns `None` otherwise.
fn merge_from_candidate(
    mems: &[MemRow],
    mem: &MemRow,
    nid: i64,
    dist: f64,
    cfg: &ConsolidationConfig,
    now: i64,
) -> Option<Merge> {
    if nid == mem.id || cosine_from_l2(dist) < cfg.tau_merge {
        return None;
    }
    let cand = mems.iter().find(|m| m.id == nid)?;
    let score_mem = effective(cfg, mem.base_score, mem.created_at, now, mem.use_count);
    let score_cand = effective(cfg, cand.base_score, cand.created_at, now, cand.use_count);
    let (survivor, loser) = if score_mem >= score_cand {
        (mem, cand)
    } else {
        (cand, mem)
    };
    Some(Merge {
        survivor: survivor.id,
        loser: loser.id,
        loser_use_count: loser.use_count,
    })
}

/// Finds the first mergeable pair among `agent_id`'s active memories, or `None`.
fn find_merge(
    store: &VectorStore,
    agent_id: &str,
    cfg: &ConsolidationConfig,
    now: i64,
) -> Result<Option<Merge>> {
    let mems = store.active_memories(agent_id)?;
    for mem in &mems {
        let emb = store.embedding_of(mem.id)?;
        for (nid, dist) in store.knn(agent_id, &emb, 2)? {
            if let Some(m) = merge_from_candidate(&mems, mem, nid, dist, cfg, now) {
                return Ok(Some(m));
            }
        }
    }
    Ok(None)
}

/// Runs one consolidation pass for `agent_id`: decay-scored dedup/merge only (v1).
///
/// Repeatedly merges the first near-duplicate pair (cosine `>= tau_merge`) into the
/// higher-`effective` survivor until none remain. Summarize/evict is deferred to v2.
///
/// # Errors
///
/// Returns an error if any underlying store operation fails.
pub fn consolidate(
    store: &mut VectorStore,
    agent_id: &str,
    cfg: &ConsolidationConfig,
    clock: &dyn Clock,
) -> Result<()> {
    let now = clock.now_unix();
    while let Some(m) = find_merge(store, agent_id, cfg, now)? {
        store.add_use_count(m.survivor, m.loser_use_count)?; // sum loser's use_count in
        store.set_superseded(m.loser, m.survivor)?; // retire loser (no new vector row)
    }
    Ok(())
}

// Source: test/specs/genesis-memory-server.md — Implementation Requirements
// (group "Consolidation (decay + dedup/merge only)"). Store-backed tests use a real temp
// SQLite + sqlite-vec database per test (no mocks, §5 #6).
#[cfg(test)]
mod tests {
    use super::{
        consolidate, cosine_from_l2, effective, Clock, ConsolidationConfig, FixedClock, SystemClock,
    };
    use crate::embed::EMBED_DIM;
    use crate::store::VectorStore;

    const DAY: i64 = 86_400;

    fn open_temp() -> (tempfile::TempDir, VectorStore) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.db");
        let store = VectorStore::open(path.to_str().unwrap()).unwrap();
        (dir, store)
    }

    /// A 384-dim L2-normalized vector in the (slot0, slot1) plane.
    fn unit_vec(a: f32, b: f32) -> Vec<f32> {
        let mut v = vec![0.0f32; EMBED_DIM];
        let n = (a * a + b * b).sqrt();
        v[0] = a / n;
        v[1] = b / n;
        v
    }

    // ─── Decay / recency scoring ──────────────────────────────────────────────

    #[test]
    fn effective_score_uses_the_specified_decay_and_usage_formula() {
        let c = ConsolidationConfig::default();
        let expected = 2.0 * (-c.lambda * 10.0).exp() * (1.0 + c.beta * (1.0f64 + 4.0).ln());
        approx::assert_abs_diff_eq!(effective(&c, 2.0, 0, 10 * DAY, 4), expected, epsilon = 1e-6);
    }

    #[test]
    fn effective_is_one_at_age_zero_and_half_at_thirty_days() {
        let c = ConsolidationConfig::default();
        approx::assert_abs_diff_eq!(effective(&c, 1.0, 0, 0, 0), 1.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(effective(&c, 1.0, 0, 30 * DAY, 0), 0.5, epsilon = 1e-6);
    }

    #[test]
    fn lambda_defaults_to_ln2_over_30() {
        approx::assert_abs_diff_eq!(
            ConsolidationConfig::default().lambda,
            core::f64::consts::LN_2 / 30.0,
            epsilon = 1e-6
        );
    }

    #[test]
    fn beta_defaults_to_zero_point_one_five() {
        approx::assert_abs_diff_eq!(ConsolidationConfig::default().beta, 0.15, epsilon = 1e-6);
    }

    #[test]
    fn tau_merge_defaults_to_zero_point_nine_five() {
        approx::assert_abs_diff_eq!(
            ConsolidationConfig::default().tau_merge,
            0.95,
            epsilon = 1e-6
        );
    }

    #[test]
    fn base_score_defaults_to_one_as_a_config_field() {
        // A field (overridable per test), not a magic literal buried in an insert statement.
        approx::assert_abs_diff_eq!(
            ConsolidationConfig::default().base_score,
            1.0,
            epsilon = 1e-6
        );
    }

    #[test]
    fn store_writes_the_configured_base_score_into_every_new_row() {
        let (_d, mut store) = open_temp();
        // override the normalization per test (struct-update, not field reassignment)
        let cfg = ConsolidationConfig {
            base_score: 0.42,
            ..ConsolidationConfig::default()
        };
        let id = store
            .insert("a", "x", &unit_vec(1.0, 0.0), cfg.base_score, 0)
            .unwrap();
        let row = store.active_memories("a").unwrap();
        assert_eq!(row[0].id, id);
        approx::assert_abs_diff_eq!(row[0].base_score, 0.42, epsilon = 1e-9);
    }

    #[test]
    fn age_days_is_measured_from_created_at() {
        // effective has no last_used_at parameter: age is created-based by construction.
        // 30 days after created_at => exactly half, independent of any recall/last-used time.
        let c = ConsolidationConfig::default();
        approx::assert_abs_diff_eq!(
            effective(&c, 1.0, 100, 100 + 30 * DAY, 0),
            0.5,
            epsilon = 1e-6
        );
    }

    #[test]
    fn cap_stays_in_config_at_ten_thousand_and_unused_in_v1() {
        assert_eq!(ConsolidationConfig::default().cap, 10_000);
        // v1 never evicts: a small store below cap is untouched by consolidate (aside from merges).
        let (_d, mut store) = open_temp();
        let cfg = ConsolidationConfig::default();
        for i in 0..3 {
            // three mutually dissimilar vectors — no merges
            let mut v = vec![0.0f32; EMBED_DIM];
            v[i] = 1.0;
            store.insert("a", "x", &v, cfg.base_score, 0).unwrap();
        }
        consolidate(&mut store, "a", &cfg, &FixedClock(0)).unwrap();
        assert_eq!(
            store.active_memories("a").unwrap().len(),
            3,
            "nothing evicted in v1"
        );
    }

    // ─── Dedup / merge ────────────────────────────────────────────────────────

    #[test]
    fn cosine_is_derived_from_l2_distance() {
        approx::assert_abs_diff_eq!(cosine_from_l2(0.0), 1.0, epsilon = 1e-6); // identical
        approx::assert_abs_diff_eq!(cosine_from_l2(2.0f64.sqrt()), 0.0, epsilon = 1e-6);
        // orthogonal
    }

    #[test]
    fn the_dedup_candidate_is_the_knn_top1_within_the_same_agent() {
        // A near-duplicate under agent B must NOT be merged when consolidating agent A.
        let (_d, mut store) = open_temp();
        let cfg = ConsolidationConfig::default();
        store
            .insert("A", "a1", &unit_vec(1.0, 0.001), cfg.base_score, 0)
            .unwrap();
        store
            .insert("A", "a2", &unit_vec(1.0, 0.002), cfg.base_score, 0)
            .unwrap();
        store
            .insert("B", "b1", &unit_vec(1.0, 0.001), cfg.base_score, 0)
            .unwrap();
        consolidate(&mut store, "A", &cfg, &FixedClock(0)).unwrap();
        assert_eq!(store.active_memories("A").unwrap().len(), 1, "A merged");
        assert_eq!(store.active_memories("B").unwrap().len(), 1, "B untouched");
    }

    #[test]
    fn memories_merge_at_or_above_tau_merge() {
        let cfg = ConsolidationConfig::default();
        // near-identical => merge
        let (_d1, mut s1) = open_temp();
        s1.insert("a", "x", &unit_vec(1.0, 0.001), cfg.base_score, 0)
            .unwrap();
        s1.insert("a", "y", &unit_vec(1.0, 0.002), cfg.base_score, 0)
            .unwrap();
        consolidate(&mut s1, "a", &cfg, &FixedClock(0)).unwrap();
        assert_eq!(s1.active_memories("a").unwrap().len(), 1);
        // orthogonal (cosine 0 < tau) => no merge
        let (_d2, mut s2) = open_temp();
        s2.insert("a", "x", &unit_vec(1.0, 0.0), cfg.base_score, 0)
            .unwrap();
        s2.insert("a", "y", &unit_vec(0.0, 1.0), cfg.base_score, 0)
            .unwrap();
        consolidate(&mut s2, "a", &cfg, &FixedClock(0)).unwrap();
        assert_eq!(s2.active_memories("a").unwrap().len(), 2);
    }

    #[test]
    fn dedup_merge_runs_only_inside_consolidate() {
        // D8: storing a near-duplicate does NOT retire anything; only consolidate does.
        let (_d, mut store) = open_temp();
        let cfg = ConsolidationConfig::default();
        store
            .insert("a", "x", &unit_vec(1.0, 0.001), cfg.base_score, 0)
            .unwrap();
        store
            .insert("a", "y", &unit_vec(1.0, 0.002), cfg.base_score, 0)
            .unwrap();
        assert_eq!(
            store.active_memories("a").unwrap().len(),
            2,
            "store does not merge"
        );
        consolidate(&mut store, "a", &cfg, &FixedClock(0)).unwrap();
        assert_eq!(
            store.active_memories("a").unwrap().len(),
            1,
            "consolidate merges"
        );
    }

    #[test]
    fn merge_keeps_the_higher_scored_row_as_the_survivor() {
        let (_d, mut store) = open_temp();
        let cfg = ConsolidationConfig::default();
        let older = store
            .insert("a", "dup a", &unit_vec(1.0, 0.001), cfg.base_score, 0)
            .unwrap();
        let newer = store
            .insert("a", "dup b", &unit_vec(1.0, 0.002), cfg.base_score, 0)
            .unwrap();
        store.add_use_count(newer, 5).unwrap(); // newer scores higher
        consolidate(&mut store, "a", &cfg, &FixedClock(0)).unwrap();
        let active = store.active_memories("a").unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, newer);
        assert_eq!(store.superseded_ids("a").unwrap(), vec![older]);
    }

    #[test]
    fn merge_sums_use_count_into_the_survivor() {
        let (_d, mut store) = open_temp();
        let cfg = ConsolidationConfig::default();
        let older = store
            .insert("a", "dup a", &unit_vec(1.0, 0.001), cfg.base_score, 0)
            .unwrap();
        let newer = store
            .insert("a", "dup b", &unit_vec(1.0, 0.002), cfg.base_score, 0)
            .unwrap();
        store.add_use_count(older, 2).unwrap();
        store.add_use_count(newer, 5).unwrap();
        consolidate(&mut store, "a", &cfg, &FixedClock(0)).unwrap();
        let active = store.active_memories("a").unwrap();
        assert_eq!(active[0].id, newer);
        assert_eq!(active[0].use_count, 7, "survivor use_count = 5 + loser's 2");
    }

    #[test]
    fn merge_sets_the_losers_superseded_by_to_the_survivor_id() {
        let (_d, mut store) = open_temp();
        let cfg = ConsolidationConfig::default();
        let older = store
            .insert("a", "dup a", &unit_vec(1.0, 0.001), cfg.base_score, 0)
            .unwrap();
        let newer = store
            .insert("a", "dup b", &unit_vec(1.0, 0.002), cfg.base_score, 0)
            .unwrap();
        store.add_use_count(newer, 5).unwrap();
        consolidate(&mut store, "a", &cfg, &FixedClock(0)).unwrap();
        assert_eq!(store.superseded_ids("a").unwrap(), vec![older]);
        assert_eq!(store.active_memories("a").unwrap()[0].id, newer);
    }

    #[test]
    fn merge_writes_no_new_vector_row() {
        let (_d, mut store) = open_temp();
        let cfg = ConsolidationConfig::default();
        store
            .insert("a", "dup a", &unit_vec(1.0, 0.001), cfg.base_score, 0)
            .unwrap();
        store
            .insert("a", "dup b", &unit_vec(1.0, 0.002), cfg.base_score, 0)
            .unwrap();
        consolidate(&mut store, "a", &cfg, &FixedClock(0)).unwrap();
        // one active + one superseded == two rows total: no third (summary) row was added.
        let total =
            store.active_memories("a").unwrap().len() + store.superseded_ids("a").unwrap().len();
        assert_eq!(total, 2, "merge adds no new row");
    }

    // ─── Recall side effects (via VectorStore::touch, exercised by do_recall) ──

    #[test]
    fn recall_increments_use_count_for_each_returned_memory() {
        let (_d, mut store) = open_temp();
        let id = store.insert("a", "x", &unit_vec(1.0, 0.0), 1.0, 0).unwrap();
        store.touch(id, 50).unwrap();
        store.touch(id, 60).unwrap();
        assert_eq!(store.active_memories("a").unwrap()[0].use_count, 2);
    }

    #[test]
    fn recall_sets_last_used_at_for_each_returned_memory() {
        let (_d, mut store) = open_temp();
        let id = store
            .insert("a", "x", &unit_vec(1.0, 0.0), 1.0, 10)
            .unwrap();
        store.touch(id, 777).unwrap();
        assert_eq!(store.active_memories("a").unwrap()[0].last_used_at, 777);
    }

    // ─── Determinism knobs ────────────────────────────────────────────────────

    #[test]
    fn now_is_injected_through_a_clock_abstraction() {
        assert_eq!(FixedClock(1234).now_unix(), 1234);
        // SystemClock implements the same trait (production path); just exercise it.
        let via_trait: &dyn Clock = &SystemClock;
        assert!(via_trait.now_unix() > 0);
    }

    #[test]
    fn every_threshold_is_config_exposed_and_assertable_within_1e_minus_6() {
        let c = ConsolidationConfig::default();
        approx::assert_abs_diff_eq!(c.lambda, core::f64::consts::LN_2 / 30.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(c.beta, 0.15, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(c.tau_merge, 0.95, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(c.base_score, 1.0, epsilon = 1e-6);
        assert_eq!(c.cap, 10_000);
    }
}
