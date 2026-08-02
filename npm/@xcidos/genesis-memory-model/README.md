# @xcidos/genesis-memory-model

Platform-independent embedding model for the **Genesis MCP memory server**. Shipped as a
separate package so all eight platform binaries share one copy of the weights instead of
bundling them eight times.

Ships two files at the package root:

```
onnx/model.onnx     # ONNX export of the embedder
tokenizer.json      # matching fast tokenizer
```

The launcher (`@xcidos/genesis-memory-server`) resolves this package's directory and passes
it to the native server as `GENESIS_MODEL_DIR`.

## Model provenance

- **Model:** `sentence-transformers/all-MiniLM-L6-v2` (384-dim, mean pooling)
- **Pinned revision:** `c9745ed1d9f207416be6d2e6f8de32d1f16199bf`

The revision is pinned load-bearingly: ONNX exports of the same model differ in output shape
and tokenizer behavior, so the pinned commit is what keeps the golden embedding vectors
reproducible. Do not change it without regenerating the server's golden vectors and constants.

## License

The bundled model and tokenizer are redistributed from `all-MiniLM-L6-v2` under
**Apache-2.0**. This package's metadata is part of the Genesis project
([repository](https://github.com/Atiqul-Islam/genesis)).
