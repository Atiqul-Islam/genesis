# Genesis memory server — npm distribution

This directory holds the npm packages that ship the **Genesis MCP memory server** as prebuilt,
platform-specific native binaries — the esbuild / `@napi-rs` / biome / sharp model, and how
Claude Code itself distributes native code. The one run-time prerequisite is **Node.js** (`>=18`);
there is no Python, no local Rust build, and no run-time network access.

Install/run:

```json
{ "mcpServers": { "genesis-memory": { "command": "npx", "args": ["-y", "@atiqul/genesis-memory-server"] } } }
```

## Layout

```
npm/
├── README.md                                   ← this file
├── .gitignore                                  ← keeps CI-filled binaries/weights out of git
├── scripts/
│   └── generate-platform-packages.mjs          ← emits the 8 platform manifests + syncs versions
├── templates/
│   └── platform-package.example.json           ← reference shape of one platform manifest
└── @atiqul/
    ├── genesis-memory-server/                  ← LAUNCHER (published bin: `genesis-memory`)
    │   ├── package.json                         ← optionalDependencies (8, exact) + model dep (exact)
    │   ├── bin/genesis-memory.js                ← esbuild generateBinPath: resolve + spawn
    │   └── README.md
    ├── genesis-memory-model/                   ← MODEL (platform-independent weights)
    │   ├── package.json                         ← files: onnx/model.onnx + tokenizer.json
    │   └── README.md
    └── genesis-memory-server-<key>/            ← 8 PLATFORM packages (manifest committed, binary by CI)
        └── package.json                         ← os / cpu / libc + single `files` entry
```

The eight platform keys: `darwin-arm64`, `darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`,
`linux-x64-musl`, `linux-arm64-musl`, `win32-x64`, `win32-arm64`.

## How resolution works

`@atiqul/genesis-memory-server` declares each platform package as an **`optionalDependencies`**
entry gated on `os`/`cpu`/`libc`, so a normal `npm install` downloads only the one binary that
matches the machine. At run time `bin/genesis-memory.js`:

1. Builds the key `${platform}-${arch}(-${libc})` (Linux libc via `process.report`, backstopped
   by `ldd --version` — the biome pattern).
2. `require.resolve('@atiqul/genesis-memory-server-<key>/genesis-memory-server[.exe]')`.
3. Resolves `@atiqul/genesis-memory-model` and exports its dir as `GENESIS_MODEL_DIR` (the
   directory holding `onnx/model.onnx` + `tokenizer.json`, which the server reads).
4. `spawn(bin, argv, { stdio: 'inherit' })`, forwards termination signals, and exits with the
   child's status.

Overrides: `GENESIS_MEMORY_BIN` (run a locally-built binary directly) and `GENESIS_MODEL_DIR`
(use a model directory as-is). Both match the legacy launcher's behavior.

## Binaries and weights are NOT committed

Only the JSON/JS manifests and the launcher live here. CI produces the native binaries and
fetches the model, then stages them into the package directories immediately before publish
(see `.gitignore`). Do not commit `*.onnx`, `tokenizer.json`, or any `genesis-memory-server[.exe]`.

## Keeping versions in sync

Every launcher `optionalDependencies` / `dependencies` entry is an **exact** version and must
match the versions actually published, or `npm install` of the launcher fails. Do not hand-edit
the nine-plus version strings — run the generator, which is the single source of truth:

```sh
# Stamp a version everywhere (8 platform manifests + launcher + model), creating dirs as needed:
node npm/scripts/generate-platform-packages.mjs 0.1.0

# Reuse the launcher's current version:
node npm/scripts/generate-platform-packages.mjs

# CI gate — fail if anything drifted from the given version:
node npm/scripts/generate-platform-packages.mjs --check 0.1.0
```

## Publish order (load-bearing)

Because the launcher pins **exact** versions of its dependencies, every dependency must already
exist on the registry before the launcher is published. Publish in this order:

1. **All 8 platform packages** — `@atiqul/genesis-memory-server-<key>` (each with its binary staged in).
2. **The model package** — `@atiqul/genesis-memory-model` (with weights + tokenizer staged in).
3. **The launcher last** — `@atiqul/genesis-memory-server`.

Each with:

```sh
npm publish --provenance --access public
```

`--provenance` attaches a signed build-provenance attestation (requires publishing from CI via
OIDC); `--access public` publishes the scoped package publicly (scoped packages are private by
default — also set in each `publishConfig`). If the launcher is published before its dependencies
exist at the exact pinned versions, installs will 404; publishing dependencies first makes the
launcher resolvable the moment it lands.
