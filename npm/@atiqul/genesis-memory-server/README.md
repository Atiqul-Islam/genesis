# @atiqul/genesis-memory-server

Thin, cross-platform launcher for the **Genesis MCP memory server** — per-agent semantic
memory over SQLite + local ONNX embeddings.

Installing this package pulls in exactly one prebuilt native binary (via
`optionalDependencies`, gated on `os`/`cpu`/`libc`) plus the shared embedding model
(`@atiqul/genesis-memory-model`). At run time the `genesis-memory` bin resolves the binary
for the current platform, points it at the bundled model, and spawns it with the MCP stdio
stream inherited untouched.

The only prerequisite is **Node.js** (`>=18`). No Python, no Rust build, no run-time network.

## Use as an MCP server

```json
{
  "mcpServers": {
    "genesis-memory": {
      "command": "npx",
      "args": ["-y", "@atiqul/genesis-memory-server"]
    }
  }
}
```

## Environment overrides

| Variable | Effect |
|---|---|
| `GENESIS_MEMORY_BIN` | Run this locally-built server binary directly (dev/CI). Skips platform resolution. |
| `GENESIS_MODEL_DIR` | Use this directory as-is for the model (must contain `onnx/model.onnx` + `tokenizer.json`). Otherwise the bundled model package is used. |

Build a local server for `GENESIS_MEMORY_BIN`:

```sh
cd server && cargo build --release
# -> server/target/release/genesis-memory-server[.exe]
```

## How it resolves the binary

`bin/genesis-memory.js` follows esbuild's `generateBinPath` pattern: it builds the platform
key `${platform}-${arch}(-${libc})`, then `require.resolve(...)` locates
`@atiqul/genesis-memory-server-<key>/genesis-memory-server[.exe]`. On Linux, glibc-vs-musl is
detected via `process.report` with an `ldd --version` backstop (the biome pattern).

Published platform keys: `darwin-arm64`, `darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`,
`linux-x64-musl`, `linux-arm64-musl`, `win32-x64`, `win32-arm64`.

## License

MIT OR Apache-2.0. See the [Genesis repository](https://github.com/Atiqul-Islam/genesis).
