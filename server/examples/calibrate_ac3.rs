//! Calibration harness for AC3 (spec Bootstrap item 4).
//!
//! Run: `cargo run --release --example calibrate_ac3`
//!
//! It stores a source plus deliberately-dissimilar decoys, recalls with a paraphrase of the
//! source, and confirms the source ranks first among the candidates. On success it writes
//! `tests/fixtures/ac3_calibration.json` — commit that file (bootstrap item 4). If the source
//! does not rank first it errors, directing the developer to adjust the literals below.
//!
//! No mocks: the real ONNX model + real `sqlite-vec` store drive the ranking.

use anyhow::{anyhow, ensure, Result};
use genesis_memory::consolidate::{ConsolidationConfig, FixedClock};
use genesis_memory::embed::{model_paths, Embedder};
use genesis_memory::store::VectorStore;
use genesis_memory::{do_recall, do_store};

fn main() -> Result<()> {
    let (model, tokenizer) = model_paths();
    ensure!(
        model.exists(),
        "model missing at {}: run node scripts/fetch-model.mjs",
        model.display()
    );
    let mut embedder = Embedder::load(&model.to_string_lossy(), &tokenizer.to_string_lossy())?;
    let cfg = ConsolidationConfig::default();
    let clock = FixedClock(0);

    let source = "The database migration must run before the service restarts.";
    let paraphrase = "Apply the schema migration prior to bouncing the service.";
    let decoys = [
        "The cat slept on the warm windowsill all afternoon.",
        "Quarterly revenue exceeded the forecast by twelve percent.",
        "Remember to water the office plants on Fridays.",
    ];

    let dir = tempfile::tempdir()?;
    let db = dir.path().join("m.db");
    let db_path = db.to_str().ok_or_else(|| anyhow!("non-utf8 db path"))?;
    let mut store = VectorStore::open(db_path)?;

    do_store(&mut store, &mut embedder, &cfg, &clock, "cal", source)?;
    for decoy in &decoys {
        do_store(&mut store, &mut embedder, &cfg, &clock, "cal", decoy)?;
    }

    let json = do_recall(&mut store, &mut embedder, &clock, "cal", paraphrase, 5)?;
    let items: serde_json::Value = serde_json::from_str(&json)?;
    let first = items
        .get(0)
        .and_then(|it| it.get("text"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("recall returned no first entry"))?;
    ensure!(
        first == source,
        "source did not rank first (got: {first}); adjust the fixtures"
    );

    let out = serde_json::json!({
        "source": source,
        "paraphrase": paraphrase,
        "decoys": decoys,
    });
    std::fs::create_dir_all("tests/fixtures")?;
    std::fs::write(
        "tests/fixtures/ac3_calibration.json",
        serde_json::to_string_pretty(&out)?,
    )?;
    println!("wrote tests/fixtures/ac3_calibration.json");
    Ok(())
}
