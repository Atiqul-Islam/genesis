# Golden embedding fixture — provenance

`all_minilm_l6_v2_golden.json` is the frozen embedding of the input
`"genesis remembers what matters"` produced by `all-MiniLM-L6-v2`
(384-dim, mean-pooled, L2-normalized).

## Backend: tract (pure Rust), NOT ONNX Runtime

These vectors are **tract-backend-generated**. The server's inference engine was swapped from
ONNX Runtime (the `ort` `download-binaries` prebuilt) to the pure-Rust **tract** backend
(`ort` `alternative-backend` + `ort-tract`) so the binary builds for every target — including
`x86_64-apple-darwin` and the musl targets, for which pyke ships no ONNX Runtime prebuilt.

tract's floating-point kernels differ slightly from ONNX Runtime's, so the ONNX-Runtime-era
golden vector no longer matched at the `1e-4` tolerance. The fixture was therefore regenerated
with the tract backend via the `capture_golden_vector` harness in `src/embed.rs`:

```
cargo test --release capture_golden_vector -- --ignored
```

## Why this is safe (semantic sanity proven FIRST, not a blind overwrite)

Before regenerating, semantic correctness on the tract backend was proven by
`tract_embeddings_are_semantically_correct` (`src/embed.rs`), which is independent of any frozen
fixture and asserts:

- the output is a 384-dim, unit-norm (L2 ≈ 1.0) vector; and
- a known-**similar** sentence pair scores a strictly higher cosine similarity than a
  known-**dissimilar** pair (meaning is preserved).

Only after that gate passed was the golden vector re-captured. tract is deterministic and
single-threaded, so tract-vs-tract output is bit-identical; the `1e-4` / cosine ≥ 0.9999
assertion in `golden_and_determinism_comparisons_use_tolerance_1e_minus_4` now pins mean-pool
correctness, `output[0] = last_hidden_state`, the required `token_type_ids` input, and
reproducibility on the tract engine.

The model package itself (`onnx/model.onnx` + `tokenizer.json`, pinned by `MODEL_REVISION` /
`MODEL_SHA256`) is **unchanged** — only the engine that runs it changed.
