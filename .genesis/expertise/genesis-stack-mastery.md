# Genesis Stack Mastery — every tool the repo uses, at its EXACT pinned version

**A primary-source, version-locked reference for `genesis-engineer`** — the single agent that owns and
develops the whole Genesis repo (three Rust crates `server/`, `cli/`, `hook/`; the Node launcher
`bin/genesis-memory.js`; and the Claude Code plugin). The rule this file exists to enforce: **you are an
expert on the code the repo actually builds — the pinned graph — not on "latest".** Every API decision is
made against `docs.rs/<crate>/<the version in Cargo.lock>` and that version's CHANGELOG.

> **Labels.** [VERIFIED] = read from a cited primary source (docs.rs at the pinned version, crates.io, the
> crate repo/CHANGELOG) **or** proven by the repo's own source compiling against the pinned graph (the lock +
> the `use`/call sites — the strongest possible evidence an API exists at that version). [INFERRED] = a
> reasoned corollary not directly read. Every pinned version number below is [VERIFIED] straight from the
> relevant `Cargo.lock`.
>
> **House rules honored here.** No credential VALUE is ever written (the launcher fetches PUBLIC release
> assets and needs none — verified from `bin/genesis-memory.js`). The banned research phrase is avoided:
> models do "structured reasoning", never the hyphenated trace term.

---

## 0. How to use this expertise (the operating rule)

1. **Read the version first.** Before touching any dependency, open the crate's `Cargo.toml` (the version
   *requirement*) **and** its `Cargo.lock` (the *resolved exact* version). The lock wins — that is what
   compiles. This file records both.
2. **Verify at the pinned version.** Open `https://docs.rs/<crate>/<exact-version>/` and that version's
   CHANGELOG. Never reason from "latest" — several crates here are majors ahead of the widely-blogged line
   (`thiserror` 2.x, `schemars` 1.x, `ndarray` 0.17, `rmcp` 2.x, `ort` 2.0.0-rc.x).
3. **Cite what you relied on.** When an edit depends on an API, name the `crate@version` you checked.
4. **No workspace ⇒ per-crate truth.** Genesis has **no** Cargo workspace; each crate resolves
   independently and carries its own `Cargo.lock`. Versions legitimately differ between crates (§7). Never
   assume one crate's version holds in another.

---

## 1. The pinned toolchain — Rust `1.93.0`

- `rust-toolchain.toml` → `[toolchain] channel = "1.93.0"`, `components = ["rustfmt", "clippy"]`. [VERIFIED
  `rust-toolchain.toml`]
- **Why this exact pin (load-bearing).** 1.93.0 is verified to build the pinned graph. It is the *newest*
  stable that still builds `rusqlite =0.39.0 → libsqlite3-sys 0.37.0`: `libsqlite3-sys ≥ 0.38`'s `build.rs`
  uses the `cfg_select!` macro that was stabilized only *after* 1.93, so bumping SQLite would break the build
  on this toolchain. [VERIFIED `rust-toolchain.toml` comment + `server/Cargo.toml` comment]
- **Edition.** All three crates are `edition = "2021"` (not 2024). [VERIFIED each `Cargo.toml`]
- **What the repo relies on from the toolchain:** stable `async fn`, the 2021 closure/capture rules, and a
  stable clippy that understands the crate-level `[lints]` table (used by all three crates). The pin also
  guarantees CI and every dev machine use an *identical* compiler.
- **Rule:** build and test with `1.93.0`; do not silently retune the channel. A toolchain bump must be a
  deliberate change that re-verifies the `cfg_select` boundary above. [gsm-1]

---

## 2. Repository shape & the no-workspace invariant

| Unit | Kind | Key file(s) | Ships as |
|---|---|---|---|
| `server/` | Rust lib+bin `genesis-memory-server` | `src/{main,lib,store,embed,persist,consolidate}.rs` | native binary (GitHub Release) |
| `cli/` | Rust lib+bin `genesis-cli` | `src/*.rs` (assemble/bootstrap/promote/doctor/fix/memfix/…) | native binary (GitHub Release) |
| `hook/` | Rust lib+bin `genesis-hook` | `src/{main,gate,validate,inject,enforce_research,…}.rs` | native binary (GitHub Release) |
| `bin/genesis-memory.js` | Node launcher | one file, zero deps | downloads+execs the binaries |
| plugin | Claude Code plugin | `.claude-plugin/plugin.json`, `.mcp.json`, `agents/skills/commands/hooks/templates` | the marketplace unit |

- **No `[workspace]`.** Each crate is standalone with its own lockfile; the comment "genesis has no
  workspace" recurs verbatim in every `Cargo.toml`. Consequence: three independent resolutions (§7). [VERIFIED]
- **Release profile (identical in all three crates):** `lto = "fat"`, `codegen-units = 1`, `panic =
  "unwind"`, `strip = "debuginfo"`. [VERIFIED each `Cargo.toml`]
- **House lint gate (identical in all three):** `unsafe_code = "deny"` (one scoped exception in `server`),
  `missing_docs`/`unreachable_pub` warn; clippy `pedantic` warn with `unwrap_used`/`expect_used`/`panic`/`todo`
  **denied**. New code must survive `clippy -D warnings`. [VERIFIED each `Cargo.toml`] [gsm-13]

---

## 3. `server/` — the MCP memory server stack

`genesis-memory-server`: per-agent semantic memory over SQLite + local ONNX embeddings, exposed as an MCP
server over stdio. It is the heaviest crate (312 resolved packages). Direct dependencies below; every version
is the **resolved** one from `server/Cargo.lock`. [VERIFIED `server/Cargo.lock`]

### 3.1 Direct dependencies (server) — pinned table

| crate | exact version | req in Cargo.toml | role | key APIs used here | depth |
|---|---|---|---|---|---|
| `rmcp` | **2.2.0** | `2.2.0` | official MCP Rust SDK (server) | `#[tool_router]`, `#[tool_handler]`, `#[tool]`, `ServerHandler`, `ServiceExt::serve`, `transport::stdio`, `model::{CallToolResult,ContentBlock,Implementation,ProtocolVersion,ServerCapabilities,ServerInfo}`, `handler::server::wrapper::Parameters`, `ErrorData as McpError`, re-exported `schemars` | deep |
| `schemars` | **1.2.1** | `1` | JSON-Schema for tool inputs | `#[derive(JsonSchema)]` on tool arg structs (via `rmcp::schemars`) | deep |
| `serde` | **1.0.229** | `1` (+`derive`) | (de)serialization | `#[derive(Serialize,Deserialize)]`, `#[serde(default, skip_serializing_if, rename)]` | deep |
| `serde_json` | **1.0.150** | `1` | JSON values / tool payloads | `serde_json::{json,Value,Map}`, to/from string | deep |
| `tokio` | **1.53.0** | `1` (+`macros,rt,rt-multi-thread,io-std,signal`) | async runtime | `#[tokio::main]`, `#[tokio::test]`, `tokio::sync::Mutex`, stdio + signal handling | deep |
| `anyhow` | **1.0.104** | `1` | app error type | `anyhow::{Result,Context}`, `?` propagation | deep |
| `thiserror` | **2.0.19** | `2` | library error enums | `#[derive(thiserror::Error)]` on `embed`/store errors | deep |
| `rusqlite` | **0.39.0** | `=0.39.0` (+`bundled`) | SQLite driver (bundled) | `Connection::{open}`, `params!`, `OptionalExtension`, `OpenFlags`, `ffi::sqlite3_auto_extension`, `Row` | deep |
| `sqlite-vec` | **0.1.9** | `0.1.9` | vec0 KNN extension (FFI) | `sqlite3_vec_init` registered via auto-extension; `vec0(embedding float[384])` | deep |
| `bytemuck` | **1.25.1** | `1` | zero-copy casts | `cast_slice::<f32,u8>` to store/read embedding blobs | deep |
| `sha2` | **0.10.9** | `0.10` | content-addressed ids | `Sha256`, `Digest` → `sha256(normalized_text+type+agent_id)` | deep |
| `hex` | **0.4.3** | `0.4` | hex encoding | encode the sha256 digest to the memory id | deep |
| `ort` | **2.0.0-rc.13** | `=2.0.0-rc.13` (`default-features=false` +`alternative-backend,ndarray,std,api-17`) | ONNX Runtime API surface (engine swapped to tract) | `set_api`, `session::Session`, `session::builder::GraphOptimizationLevel`, `value::{TensorRef,DynValue}`, `inputs!`, `Error`/`Result` | deep |
| `ort-tract` | **0.4.0** (`0.4.0+0.23`) | `=0.4.0` | pure-Rust backend for `ort` | `ort_tract::api()` → `OrtApi` installed by `ort::set_api` | deep |
| `ndarray` | **0.17.2** | `0.17` | tensors | `Array`, `Array3<f32>`, indexing for mean-pooling hidden states | deep |
| `tokenizers` | **0.23.1** | `0.23.1` | HF tokenizer | `Tokenizer::from_file`, encode with special tokens → input_ids/mask/type_ids | deep |
| `tracing` | **0.1.44** | `0.1` | structured logs | span/event macros to stderr | deep |
| `tracing-subscriber` | **0.3.23** | `0.3` (+`env-filter,fmt`) | log subscriber | `fmt` layer + `EnvFilter` (stderr only; stdout is the MCP channel) | deep |

**dev-dependencies (server):** `cucumber` **0.23.0** (`0.23` +`macros`) — BDD; `assert_cmd` **2.2.2** — spawn
the bin in tests; `approx` **0.5.1** — float asserts on embeddings; `insta` **1.48.0** — snapshot tests;
`tempfile` **3.27.0** — throwaway DB dirs. [VERIFIED `server/Cargo.lock`]

### 3.2 MCP layer — `rmcp` 2.2.0 [VERIFIED docs.rs/rmcp/2.2.0]

- **Identity.** `rmcp` 2.2.0 = "Rust SDK for Model Context Protocol", published 2026-07-08, repo
  `github.com/modelcontextprotocol/rust-sdk`, Apache-2.0. [VERIFIED docs.rs/rmcp/2.2.0]
- **Feature flags (docs.rs table).** `server` (server functionality + the tool system, **default**),
  `macros` (`#[tool]`/`#[prompt]` macros, re-exports `rmcp-macros`, **default**), `client`, `schemars`
  (JSON-Schema generation for tool definitions, **not** default), `auth`, `elicitation`. The repo declares
  `features = ["server","macros","transport-io"]`; `transport-io` provides the stdio transport used here.
  [VERIFIED docs.rs/rmcp/2.2.0 feature table + `server/Cargo.toml`]
- **schemars is re-exported.** `rmcp` re-exports `schemars` (docs.rs points the re-export at
  `schemars 1.2.1`), which is why the server writes `use rmcp::{schemars, …}` and the tool-arg structs derive
  `JsonSchema` through it. Treat `rmcp::schemars` as the single schema generator; do **not** pull a second,
  differently-versioned `schemars`. [VERIFIED docs.rs/rmcp/2.2.0 re-exports] [gsm-14]
- **Attribute macros used** (all require `macros`+`server`): `#[tool_router]` (generates
  `Self::tool_router()`), `#[tool_handler]` (impls the request dispatch on the `ServerHandler`), `#[tool(description=…)]`
  (registers one tool). The router is generated, not stored on the struct. [VERIFIED docs.rs/rmcp/2.2.0
  attribute-macro list + `server/src/lib.rs`]
- **Serve loop.** `ServerHandler` + `ServiceExt` are re-exported at the crate root; the server calls
  `server.serve(stdio()).await?` (`ServiceExt::serve`) and awaits the returned service to run until the client
  disconnects. `ErrorData` is re-exported and aliased `McpError`. [VERIFIED docs.rs/rmcp/2.2.0 re-exports +
  `server/src/lib.rs`]
- **Tool results.** `CallToolResult::success(vec![ContentBlock::text(..)])` /
  `CallToolResult::error(vec![ContentBlock::text(..)])`; args arrive via
  `Parameters<T: JsonSchema+Deserialize>`. [VERIFIED `server/src/lib.rs`]
- **Protocol version.** `get_info()` returns `ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
  .with_server_info(Implementation::from_build_env()).with_protocol_version(ProtocolVersion::V_2024_11_05)` —
  i.e. MCP protocol **2024-11-05**. [VERIFIED `server/src/lib.rs`, compiles against rmcp 2.2.0]
- **Gotchas.** rmcp 2.x is a *different major* from any 0.x tutorial; the `#[tool]` surface and `model` types
  moved across 1.x→2.x. Verify any handler/model change against docs.rs/rmcp/2.2.0. TLS/reqwest features are
  HTTP-transport-only and irrelevant to this stdio server.

### 3.3 Embeddings layer — `ort` + `ort-tract` + `tract` + `tokenizers` + `ndarray`

This is the most unusual part of the stack: the crate keeps **`ort`'s API** but swaps the **engine** from
Microsoft's ONNX Runtime to the **pure-Rust `tract`** inference engine, so the binary builds for *every*
target (incl. Intel macOS and musl Linux) with **no** prebuilt ONNX Runtime, no cmake, no sidecar dylib.

- **`ort` 2.0.0-rc.13** — "a safe Rust wrapper for ONNX Runtime 1.28". Declared `default-features = false`
  with `alternative-backend` (= `ort-sys/disable-linking`: no linked ONNX Runtime), `ndarray`, `std`,
  `api-17`. [VERIFIED docs.rs/ort/2.0.0-rc.13 + `server/Cargo.toml`]
- **The backend swap.** `ort::set_api(api: OrtApi) -> bool` "sets the global `ort_sys::OrtApi` … to use
  alternative backends … **When using an alternative backend, this must be called before using any other
  `ort` API.** Returns `true` if successful; if an API was already set it will **not** be overridden
  (returns `false`)." Docs give the exact call `ort::set_api(ort_tract::api());` — identical to the repo.
  [VERIFIED docs.rs/ort/2.0.0-rc.13/fn.set_api.html] The server calls it once before building any `Session`.
  [VERIFIED `server/src/embed.rs`] [gsm-7]
- **`ort-tract` 0.4.0 (`+0.23`).** Provides `ort_tract::api()` → an `OrtApi` implemented by tract. The
  `+0.23` build metadata marks its tract line (0.23.x). It pins `ort-sys =2.0.0-rc.13`; because `ort-sys` is a
  single `links` crate, `ort` and `ort-tract` **must** agree on that exact rc — the reason `ort` is pinned to
  `=2.0.0-rc.13` and `api-17` is chosen (the `OrtApi` version tract's stub implements). Mixing rc levels is a
  hard build error. [VERIFIED `server/Cargo.toml` comment + `server/Cargo.lock` (`ort-sys 2.0.0-rc.13`)] [gsm-8]
- **tract engine (key transitive, 0.23.4):** `tract-core`, `-data`, `-hir`, `-linalg`, `-nnef`, `-onnx`,
  `-onnx-opl`, `-pulse`, `-pulse-opl`, `-transformers`, `-extra`, all **0.23.4** — the actual graph loader +
  executor behind `ort-tract`. Deep-relevant because they define what subset of ONNX ops the model may use.
  [VERIFIED `server/Cargo.lock`]
- **tract-subset caveat (load-bearing behavior).** tract implements only a *subset* of `OrtApi`.
  `SetSessionGraphOptimizationLevel` **is** implemented, so the server may call
  `.with_optimization_level(GraphOptimizationLevel::Level3)`. The session is committed with
  `Session::builder()?.with_optimization_level(Level3)?.commit_from_file(model_path)`; the code fail-closes on
  a missing path **before** tract sees it, because tract's failed-`commit_from_file` path frees invalidly and
  aborts the process. [VERIFIED `server/src/embed.rs`]
- **Tensors.** Inputs are built with `ort::value::TensorRef::from_array_view(([1, seq], slice))` for
  `input_ids`/`attention_mask`/`token_type_ids` (all `i64`), fed via `ort::inputs![…]`; the model output is
  read as `ort::value::DynValue` → `ndarray::Array3<f32>` and mean-pooled to a 384-d embedding. [VERIFIED
  `server/src/embed.rs`] `ort::Error` is **not** `Send`/`Sync`, so the code converts it to `anyhow::Error`
  explicitly. [VERIFIED `server/src/embed.rs`]
- **`ndarray` 0.17.2** — 0.17 is a recent major; array/view APIs differ from the common 0.15 tutorials.
  Verify shape/axis calls against docs.rs/ndarray/0.17.2. [VERIFIED `server/Cargo.lock`]
- **`tokenizers` 0.23.1** — HuggingFace tokenizers; the server only uses `Tokenizer::from_file(path)` + encode
  with special tokens. Pinned exactly (`0.23.1`); it drags a large sub-tree (onig, esaxx-rs, spm_precompiled,
  monostate, minijinja, rayon — §3.8). [VERIFIED docs.rs/tokenizers/0.23.1 + `server/src/embed.rs`]
- **The model is unchanged** by the backend swap: tract loads the same `onnx/model.onnx` + `tokenizer.json`
  the Node launcher stages. 384-d output ⇒ `vec0(embedding float[384])`. [VERIFIED `server/src/*` + launcher]

### 3.4 Vector store — `rusqlite` + `libsqlite3-sys` + `sqlite-vec` + `bytemuck`

- **`rusqlite` 0.39.0** — ergonomic SQLite wrapper, published 2026-03-15, MIT. Pinned `=0.39.0` with
  `bundled` (compiles SQLite from source via `libsqlite3-sys`). Structs used: `Connection`, `OpenFlags`,
  `Row`, plus `OptionalExtension` (`.optional()`) and the `params!` macro. [VERIFIED docs.rs/rusqlite/0.39.0 +
  `server/src/store.rs`]
- **`libsqlite3-sys` 0.37.0** — the FFI + bundled SQLite amalgamation. **This is the pin the whole toolchain
  choice hinges on:** 0.37.0 is the newest `libsqlite3-sys` *without* the `cfg_select!` `build.rs` (§1); that
  is why `rusqlite` is held at `=0.39.0` (0.40.x pulls `libsqlite3-sys ≥ 0.38`). [VERIFIED
  `server/Cargo.toml` comment + `*/Cargo.lock`] [gsm-6]
- **`sqlite-vec` 0.1.9** — "FFI bindings to the sqlite-vec SQLite extension", published 2026-07-05. The crate
  exposes `sqlite3_vec_init`, registered as a SQLite *auto-extension* so every subsequent connection gets the
  `vec0` virtual table. [VERIFIED docs.rs/sqlite-vec/0.1.9 + `server/src/store.rs`]
- **The one sanctioned `unsafe` (spec §2.3b).** `unsafe { sqlite3_auto_extension(Some(std::mem::transmute(
  sqlite3_vec_init as *const ()))) }` on `VectorStore::open`, carrying a scoped `#[allow(unsafe_code)]`; the
  crate keeps `unsafe_code = "deny"` everywhere else (`deny`, not `forbid`, exactly so this single FFI
  registration is possible). Register it **before** opening the connection that creates
  `CREATE VIRTUAL TABLE IF NOT EXISTS vec_items USING vec0(embedding float[384])`. [VERIFIED
  `server/src/store.rs` + `server/Cargo.toml` lints comment] [gsm-9]
- **`bytemuck` 1.25.1** — `cast_slice::<f32,u8>` writes the 384×f32 embedding as a byte blob into `vec0` and
  reads it back; no per-element copy. [VERIFIED `server/src/store.rs`]
- **Gotchas.** `Connection::open` does **not** create the parent directory — the store `mkdir`s `.genesis/`
  first. KNN is `MATCH … ORDER BY distance LIMIT k` against `vec0` as an inner query. Content-addressed ids
  (`sha2`+`hex`) make store idempotent for dedup/sync. [VERIFIED `server/src/store.rs`]

### 3.5 Runtime, serialization & errors (server)

- **`tokio` 1.53.0** — features `macros`, `rt`, `rt-multi-thread`, `io-std`, `signal`. Used for
  `#[tokio::main]`, the stdio transport I/O, `tokio::sync::Mutex` guarding the shared store/embedder, and
  signal handling for clean shutdown (the launcher also forwards signals). 1.x is API-stable; still verify any
  new API against docs.rs/tokio/1.53.0. [VERIFIED `server/Cargo.toml` + `server/Cargo.lock` + `src/lib.rs`]
- **`serde` 1.0.229 / `serde_json` 1.0.150** — the lock resolves `serde` alongside `serde_core 1.0.229` (the
  core-traits split-out in the modern 1.0.2xx line) and `serde_derive 1.0.229`. Attributes actually used:
  `#[serde(default, skip_serializing_if = "Option::is_none")]` and `#[serde(rename = "type")]`. [VERIFIED
  `server/Cargo.lock` + macro scan] The `serde_core` split is [INFERRED] as the reason three serde packages
  appear; their presence is [VERIFIED] in the lock.
- **`schemars` 1.2.1** — published 2026-06-22; derive macros `JsonSchema` / `JsonSchema_repr`; default
  features `std`+`derive`. **1.x is a major redesign vs the widely-documented 0.8** (`Schema` is now a thin
  wrapper over a `serde_json` value; the `SchemaObject`/`RootSchema` types are gone). Any hand-written schema
  manipulation must target docs.rs/schemars/1.2.1, not 0.8 examples. [VERIFIED docs.rs/schemars/1.2.1]
- **`anyhow` 1.0.104** — application errors + `Context`. **`thiserror` 2.0.19** — library error enums; **v2 is
  a distinct major** from the ubiquitous 1.x (MSRV + format-arg changes). Verify enum/`#[error(...)]`
  attributes against docs.rs/thiserror/2.0.19 before edits; the specific v1→v2 deltas are [INFERRED] here,
  the version is [VERIFIED] from the lock. [VERIFIED `server/Cargo.lock`]
- **`sha2` 0.10.9 + `hex` 0.4.3** — content-addressed memory ids (idempotent dedup/sync); runtime, not just
  tests. [VERIFIED `server/Cargo.toml` + `store.rs`]

### 3.6 Logging (server)

- **`tracing` 0.1.44 + `tracing-subscriber` 0.3.23** (`env-filter`, `fmt`). All logs go to **stderr** —
  stdout is the MCP channel and must stay pristine (mirrored by the launcher's `log()` writing to stderr).
  `EnvFilter` reads `RUST_LOG`. [VERIFIED `server/Cargo.toml` + launcher comment]

### 3.7 Dev / test stack (server) — `cucumber` 0.23.0 & friends

- **`cucumber` 0.23.0** — "Cucumber testing framework for Rust, async, fully native, no external test
  runners." Feature `macros`. Pattern (matches the repo exactly): `#[derive(cucumber::World)]`,
  `#[given(expr=…)]`/`#[when(regex=…)]`/`#[then(…)]` async steps, `World::run("…features…").await` under
  `#[tokio::main]`, and **`[[test]] … harness = false`**. The four BDD runners (`bdd_store`, `bdd_recall`,
  `bdd_consolidate`, `bdd_server`) are `harness=false` bins that resolve `../test/features/<slug>.feature`
  from the crate root. [VERIFIED docs.rs/cucumber/0.23.0 + `server/Cargo.toml`]
- **`assert_cmd` 2.2.2** (spawn+assert the built bin), **`predicates` 3.1.4** (assert output),
  **`approx` 0.5.1** (float compare embeddings), **`insta` 1.48.0** (snapshots), **`tempfile` 3.27.0**
  (temp DB dirs). `gherkin` 0.16.0 parses the feature files. [VERIFIED `server/Cargo.lock`]

### 3.8 Key transitive dependencies (server) — deep because load-bearing

| crate | version | pulled by | role |
|---|---|---|---|
| `tract-core` … `tract-transformers` | **0.23.4** | `ort-tract` | pure-Rust ONNX graph load + execute (the real inference engine) |
| `prost` / `prost-derive` | **0.14.4** | `tract-onnx` | protobuf decode of the ONNX model file |
| `safetensors` | **0.8.0** | `tract-transformers` | tensor blob loading |
| `rayon` / `rayon-core` | **1.12.0** / **1.13.0** | tract, tokenizers | data-parallel ops |
| `rustfft` | **6.4.1** | tract-linalg | FFT kernels (with `transpose` 0.2.3, `strength_reduce` 0.2.4, `primal-check` 0.3.4) |
| `matrixmultiply` | **0.3.11** | ndarray | matmul backend |
| `ndarray` | **0.17.2** | direct + tract | tensor math |
| `half` | **2.7.1** | tract | f16 support |
| `num-traits`/`num-complex`/`num-integer` | 0.2.19 / 0.4.6 / 0.1.46 | tract, ndarray, rustfft | numeric traits |
| `memmap2` | **0.9.11** | tract | mmap the model file |
| `onig` / `onig_sys` | **6.5.3** / **69.9.3** | tokenizers | Oniguruma regex (pre-tokenizer) |
| `esaxx-rs` | **0.1.10** | tokenizers | suffix-automaton (unigram) |
| `spm_precompiled` | **0.1.4** | tokenizers | SentencePiece charsmap |
| `minijinja` | **2.21.0** | tokenizers | chat/template rendering |
| `async-trait` | **0.1.91** | rmcp | async trait methods on handlers |
| `tokio-util` | **0.7.18** | rmcp | framed stdio transport helpers |
| `bytes` | **1.12.1** | rmcp | byte buffers |
| `pin-project` | **1.1.13** | rmcp/futures | self-referential futures |
| `futures` (+ `-util`/`-core`/…) | **0.3.33** | rmcp/tokio | async combinators |
| `schemars_derive` | **1.2.1** | schemars | the `JsonSchema` derive |
| `regex` (+ `-automata`/`-syntax`) | **1.13.1** / 0.4.16 / 0.8.11 | tokenizers, subscriber | regex engine |
| `chrono` | **0.4.45** | (test/format paths) | time formatting |

[VERIFIED `server/Cargo.lock` for every version above.]

---

## 4. `cli/` — the installer/orchestrator stack

`genesis-cli` runs **once** at build/install time (assemble / bootstrap / promote / install /
build-plugin-agents + the session-copy pipeline: capture / store / embed / build-session-agent; plus
doctor / fix / memfix / merge / reconcile / render / resolve / validate). 59 resolved packages. [VERIFIED
`cli/Cargo.lock`]

### 4.1 Direct dependencies (cli) — pinned table

| crate | exact version | req | role | key APIs used here | depth |
|---|---|---|---|---|---|
| `serde_json` | **1.0.151** | `1` (+`preserve_order`) | emit settings.json / .mcp.json / required.json in insertion order (byte-parity with the Node installer) | `serde_json::{json,Value,Map}` | deep |
| `serde` | **1.0.229** | `1` (+`derive`) | portable memory record (doctor/fix) — JSONL byte-compatible with the server's `MemRecord` | `#[derive(Serialize,Deserialize)]` | deep |
| `regex` | **1.13.1** | `1` | credential scrubbing in the capture step (same ASCII regex semantics as the hooks) | `Regex`, `Captures` | deep |
| `rusqlite` | **0.39.0** | `=0.39.0` (+`bundled`) | read context-mode / genesis DBs, write `history.sqlite`; **same pin as the server** | `Connection`, `OpenFlags`, `params!`, `ffi::sqlite3_auto_extension` | deep |
| `sqlite-vec` | **0.1.9** | `0.1.9` | `fix` copies memories **and** embedding blobs straight from stray DBs into the canonical `.db` (no re-embed, no restart) | `sqlite3_vec_init` | deep |

**dev-dependencies (cli):** `tempfile` **3.27.0**. [VERIFIED `cli/Cargo.lock`]

- **`preserve_order` matters.** It pulls `indexmap 2.14.0` and makes generated JSON keep insertion order so
  the Rust output is byte-identical to the Node installer's `JSON.stringify`. Do not drop this feature.
  [VERIFIED `cli/Cargo.toml` + lock (`indexmap 2.14.0`)]
- **Same SQLite trio as the server** (`rusqlite =0.39.0`, `libsqlite3-sys 0.37.0`, `sqlite-vec 0.1.9`) so "one
  SQLite compiles across the workspace" even without a workspace. `fix` opens the vector store to migrate
  embeddings losslessly. [VERIFIED `cli/Cargo.toml` comments + lock] [gsm-5]
- **Note the drift (independent lock):** `serde_json` here is **1.0.151** (server has 1.0.150); `libc` is
  0.2.189 (server 0.2.186); `aho-corasick` 1.1.5. Same crate, different resolved patch — because there is no
  shared lock (§7).

---

## 5. `hook/` — the enforcement-hook stack (deliberately tiny)

`genesis-hook`: one self-contained, busybox-style binary dispatched by its first arg
(gate / validate / inject / enforce_research / promote_offer / …). Kept dependency-light so cold-spawn stays
~2–10 ms — the whole reason it replaces Node hooks. **33 resolved packages** total. [VERIFIED `hook/Cargo.lock`]

### 5.1 Direct dependencies (hook) — pinned table

| crate | exact version | req | role | key APIs used here | depth |
|---|---|---|---|---|---|
| `serde_json` | **1.0.151** | `1` (+`preserve_order`) | parse hook-event JSON on stdin, emit decision JSON **byte-identical** to the old Node hooks (same key order) | `serde_json::{json,Value}` | deep |
| `regex` | **1.13.1** | `1` | content checks: banned-phrase / credential-shape / `APPLIED-EXPERTISE:` parsing, with ASCII classes spelled out to match Node's regex semantics | `Regex` | deep |

**dev-dependencies (hook):** `tempfile` **3.27.0**. [VERIFIED `hook/Cargo.lock`]

- **Intentional minimalism.** No serde-derive in `Cargo.toml`; the crate hand-walks `serde_json::Value`.
  `preserve_order` (⇒ `indexmap 2.14.0`) guarantees byte-identical decision JSON so Rust↔Node parity is
  *exactly* verifiable, not merely semantic. [VERIFIED `hook/Cargo.toml` + lock]
- **The whole tree** is regex + serde_json + their shared plumbing (`aho-corasick 1.1.4`,
  `regex-automata 0.4.16`, `regex-syntax 0.8.11`, `memchr 2.8.3`, `indexmap 2.14.0`, `hashbrown 0.17.1`,
  `serde 1.0.229`/`serde_core`/`serde_derive`, plus `rustix`/`linux-raw-sys`/`windows-sys` for `tempfile` in
  tests). Adding anything heavier defeats the cold-spawn budget. [VERIFIED `hook/Cargo.lock`]

---

## 6. The Node launcher + the plugin / MCP / hooks surface

### 6.1 `bin/genesis-memory.js` — the fetch-launcher (zero npm deps)

- **Runtime contract:** Node **≥ 18** (for global `fetch`), and **only Node built-ins**: `fs`, `os`, `path`,
  `crypto`, `child_process`, plus `process.report`/`ldd` for musl detection. **No `package.json` exists
  anywhere in the repo** (verified by an exhaustive search) — so there are, by construction, zero third-party
  Node dependencies. [VERIFIED `bin/genesis-memory.js` + repo-wide `find`] [gsm-12]
- **What it does:** resolves a per-OS `platformKey()` (`darwin/linux/win32` × `x64/arm64`, `linux` split into
  `gnu`/`musl`), downloads the pinned-version binaries + ONNX model from the **public** GitHub Release,
  verifies each against `SHA256SUMS` (refuses any unlisted/mismatched asset), caches per-user, then execs one
  of: the stdio memory server (default), `--stage-hook`/`--stage-cli` (copy a binary into `.genesis/bin`),
  `--run-hook` (fail-**open** shim), `--run-cli` (fail-**loud**), `--sync` (refresh staged binaries to
  `RELEASE_VERSION`). Model-free one-shots (`structure`, `export`, `unstructured`) skip the model download.
  [VERIFIED `bin/genesis-memory.js`]
- **Single source of truth:** `const RELEASE_VERSION = "0.2.0-beta"` and `REPO = "Atiqul-Islam/genesis"`.
  Bump `RELEASE_VERSION` per release (same commit as the git tag). Diagnostics go to **stderr**; **stdout is
  the MCP channel**. No credential is ever required or written — assets are public HTTPS. [VERIFIED
  `bin/genesis-memory.js`]

### 6.2 `.mcp.json` — the MCP server registration

- One server, `genesis-memory`, launched as `node .genesis/bin/genesis-memory.js`, with env
  `GENESIS_MEMORY_DB` and `GENESIS_MEMORY_EXPORT` pointing at `.genesis/memory.db` and
  `.genesis/memory/memory.jsonl`. [VERIFIED `.mcp.json`]

### 6.3 `.claude-plugin/plugin.json` — the plugin manifest

- Fields present: `name` `genesis`, `description`, `version` **`0.2.0-beta`**, `author.name`, `license` MIT,
  `homepage`, `repository`. The plugin version and the launcher's `RELEASE_VERSION` must move together (both
  `0.2.0-beta` today). [VERIFIED `.claude-plugin/plugin.json` + launcher]

### 6.4 The MCP / hooks contract the project targets

- **MCP protocol version: `2024-11-05`** — the server advertises `ProtocolVersion::V_2024_11_05` in
  `get_info()`; keep client/server aligned to this revision. [VERIFIED `server/src/lib.rs`] [gsm-11]
- **Hook events the plugin wires** (native `genesis-hook` binary, via the launcher shim for the plugin and
  directly for assembled agents): `SessionStart` (inject house rules / pointers; promote-offer),
  `PreToolUse` (gate — surface rules before Write/Edit; enforce_research), `PostToolUse`
  (`structure` write-back), `SubagentStart`/`SubagentStop` and a `Stop` gate (the APPLIED-EXPERTISE
  declaration). [VERIFIED `hook/src/*` module names + launcher comments] The hooks are deterministic and
  fail-open at the launcher boundary so a missing binary can never break a session. [VERIFIED
  `bin/genesis-memory.js` `runHook`]

---

## 7. Cross-cutting version gotchas (read before any dependency edit)

1. **No workspace ⇒ three independent locks.** The same crate resolves to *different* patches per crate:
   `serde_json` = 1.0.150 (server) vs 1.0.151 (cli, hook); `libc` = 0.2.186 (server) vs 0.2.189 (cli, hook);
   `aho-corasick` = 1.1.4 (server, hook) vs 1.1.5 (cli); `fastrand` 2.4.1 vs 2.5.0; `cc` 1.3.0 vs 1.4.0.
   Always check the *specific* crate's lock. [VERIFIED the three locks] [gsm-10]
2. **The `cfg_select` boundary is the reason for two pins at once.** `rust-toolchain 1.93.0` **and**
   `rusqlite =0.39.0` (⇒ `libsqlite3-sys 0.37.0`) are a matched set. Bumping either can break the other:
   `libsqlite3-sys ≥ 0.38` needs a post-1.93 stable. Change them together, re-verify a clean build. [gsm-6]
3. **The `ort` ⇄ `ort-tract` ⇄ `ort-sys` rc-lock.** `ort =2.0.0-rc.13`, `ort-tract =0.4.0`, `ort-sys
   =2.0.0-rc.13` must agree exactly (`ort-sys` is a single `links` crate). `default-features=false` +
   `alternative-backend` + `api-17` is mandatory, and `ort::set_api(ort_tract::api())` must run before any
   `Session`. [gsm-7][gsm-8]
4. **The SQLite trio is intentionally shared** across server + cli (`rusqlite =0.39.0`,
   `libsqlite3-sys 0.37.0`, `sqlite-vec 0.1.9`) so one SQLite compiles everywhere; keep them in lockstep and
   keep the single sanctioned `unsafe` registration. [gsm-5][gsm-9]
5. **Majors ahead of the common tutorials:** `thiserror` 2.x, `schemars` 1.x, `ndarray` 0.17, `rmcp` 2.x,
   `ort` 2.0.0-rc.x, `derive_more` 2.x, `rand` 0.9/0.10, `nom` 8.x — never copy 0.8/1.x/0.15-era snippets;
   verify at the pinned version. [gsm-2]
6. **Byte-parity constraints:** `preserve_order` on `serde_json` in cli+hook is load-bearing (JSON key order
   must match the Node installer/hooks). Do not remove it. [VERIFIED cli/hook `Cargo.toml`]
7. **stdout is sacred** for the server and the launcher (MCP channel). All diagnostics → stderr. [VERIFIED]

---

## 8. Appendix — long-tail transitive catalogue (name · exact version · role)

Catalogued (name/version/role), **not** deep-read. All versions [VERIFIED] from the relevant `Cargo.lock`.
Grouped by cluster; unless noted, these are `server/`'s tree (the superset).

**Proc-macro & build plumbing:** `proc-macro2` 1.0.107 · `quote` 1.0.47 · `syn` 2.0.119 (+ `syn` 3.0.0 in
server / 3.0.3 in cli+hook) · `unicode-ident` 1.0.24 · `cfg-if` 1.0.4 · `autocfg` 1.5.1 · `version_check`
0.9.5 · `rustversion` 1.0.23 · `rustc_version` 0.4.1 · `cc` 1.3.0/1.4.0 · `pkg-config` 0.3.33 · `vcpkg`
0.2.15 · `find-msvc-tools` 0.1.9 · `shlex` 2.0.1 — codegen/build only.

**Derive/meta helpers:** `serde_derive` 1.0.229 · `serde_core` 1.0.229 · `thiserror-impl` 2.0.19 ·
`darling`(+core/macro) 0.20.11 & 0.23.0 · `derive_builder`(+core/macro) 0.20.2 · `derive_more`(+impl) 2.1.1 ·
`derive-new` 0.7.0 · `typed-builder`(+macro) 0.23.2 · `smart-default` 0.7.1 · `heck` 0.5.0 · `ident_case`
1.0.1 · `paste` 1.0.15 · `pastey` 0.2.3 · `sealed` 0.6.0 · `synthez`(+core/codegen) 0.4.0 ·
`macro_rules_attribute`(+proc_macro) 0.2.2 · `ref-cast`(+impl) 1.0.26 · `inventory` 0.3.24.

**Async / futures / rmcp support:** `async-trait` 0.1.91 · `futures`(+util/core/channel/executor/io/macro/
sink/task) 0.3.33 · `pin-project`(+internal) 1.1.13 · `pin-project-lite` 0.2.17 · `tokio-macros` 2.7.1 ·
`tokio-util` 0.7.18 · `bytes` 1.12.1 · `mio` 1.2.2 · `signal-hook-registry` 1.4.8 · `slab` 0.4.12 ·
`parking_lot`(+core) 0.12.5 · `lock_api` 0.4.14 · `scopeguard` 1.2.0.

**tract / ml numerics:** `tract-{core,data,hir,linalg,nnef,onnx,onnx-opl,pulse,pulse-opl,transformers,extra}`
0.23.4 · `prost`(+derive) 0.14.4 · `safetensors` 0.8.0 · `rustfft` 6.4.1 · `transpose` 0.2.3 ·
`strength_reduce` 0.2.4 · `primal-check` 0.3.4 · `matrixmultiply` 0.3.11 · `rawpointer` 0.2.1 · `half` 2.7.1 ·
`num-traits` 0.2.19 · `num-complex` 0.4.6 · `num-integer` 0.1.46 · `rand`(+chacha/core/distr) 0.9.x/0.10.x ·
`libm` 0.2.16 · `memmap2` 0.9.11 · `dyn-clone` 1.0.20 · `dyn-hash` 1.0.0 · `downcast-rs` 2.0.2 ·
`scan_fmt` 0.2.6 · `float-ord` 0.3.2 · `dary_heap` 0.3.9.

**tokenizers support:** `onig` 6.5.3 · `onig_sys` 69.9.3 · `esaxx-rs` 0.1.10 · `spm_precompiled` 0.1.4 ·
`unicode-normalization-alignments` 0.1.12 · `unicode-segmentation` 1.13.3 · `unicode_categories` 0.1.1 ·
`monostate`(+impl) 0.1.18 · `minijinja` 2.21.0 · `memo-map` 0.3.3 · `daachorse` 1.0.1 · `rayon`(+cond/core)
1.12.0/0.4.0/1.13.0 · `crossbeam-{deque,epoch,utils}` 0.8.x/0.9.20.

**Hashing / collections / regex / small utils:** `sha2` 0.10.9 · `digest` 0.10.7 · `block-buffer` 0.10.4 ·
`crypto-common` 0.1.7 · `generic-array` 0.14.7 · `cpufeatures` 0.2.17/0.3.0 · `hex` 0.4.3 ·
`aho-corasick` 1.1.4/1.1.5 · `regex`(+automata/syntax) 1.13.1/0.4.16/0.8.11 · `memchr` 2.8.3 ·
`hashbrown` 0.16.1/0.17.1 · `hashlink` 0.11.1 · `indexmap` 2.14.0 · `equivalent` 1.0.2 · `foldhash` 0.2.0 ·
`ahash` 0.8.12 · `smallvec` 1.15.2 · `itoa` 1.0.18 · `ryu` 1.0.23 · `bitflags` 2.13.1 · `bytemuck` 1.25.1 ·
`byteorder` 1.5.0 · `once_cell` 1.21.4 · `lazy_static` 1.5.0 · `either` 1.16.0 · `itertools` 0.14.0.

**Time / platform / fs:** `chrono` 0.4.45 · `iana-time-zone`(+haiku) 0.1.65/0.1.2 · `libc` 0.2.186/0.2.189 ·
`getrandom` 0.3.4/0.4.3 · `rustix` 1.1.4 · `linux-raw-sys` 0.12.1 · `errno` 0.3.14 · `fastrand` 2.4.1/2.5.0 ·
`filetime` 0.2.29 · `xattr` 1.6.1 · `tempfile` 3.27.0 · `walkdir` 2.5.0 · `same-file` 1.0.6 ·
`windows-{sys,core,link,result,strings}` 0.61.2/0.62.2/0.2.1/0.4.1/0.5.1 · `windows-{implement,interface}`
0.60.2/0.59.3 · `redox_syscall` 0.5.18 · `core-foundation-sys` 0.8.7.

**Compression / archive (release/model packaging paths):** `flate2` 1.1.9 · `miniz_oxide` 0.8.9 ·
`crc32fast` 1.5.0 · `adler2` 2.0.1 · `tar` 0.4.46 · `simd-adler32` 0.3.10.

**CLI/terminal & test-only (dev-deps' trees):** `clap`(+builder/derive/lex) 4.6.2/4.6.1/1.1.0 ·
`anstream`/`anstyle`(+parse/query/wincon) 1.0.x/3.0.11 · `colorchoice` 1.0.5 · `console` 0.16.4 ·
`indicatif` 0.18.6 · `unicode-width` 0.2.2 · `terminal_size` 0.4.4 · `textwrap` 0.16.2 · `cucumber`(+codegen)
0.23.0 · `cucumber-expressions` 0.5.0 · `gherkin` 0.16.0 · `globwalk` 0.9.1 · `globset` 0.4.19 · `ignore`
0.4.30 · `assert_cmd` 2.2.2 · `predicates`(+core/tree) 3.1.4/1.0.10/1.0.13 · `bstr` 1.13.0 · `wait-timeout`
0.2.1 · `difflib` 0.4.0 · `insta` 1.48.0 · `similar` 2.7.0 · `linked-hash-map` 0.5.6 · `approx` 0.5.1 ·
`peg`(+macros/runtime) 0.6.3 · `maplit` 1.0.2 · `humantime` 2.4.0.

**Tracing internals:** `tracing`(+core/attributes) 0.1.44/0.1.36/0.1.31 · `tracing-subscriber` 0.3.23 ·
`tracing-log` 0.2.0 · `nu-ansi-term` 0.50.3 · `sharded-slab` 0.1.7 · `thread_local` 1.1.10 · `matchers`
2.0.0 · `valuable` 0.1.1 · `log` 0.4.33.

**WASM/edge targets of transitive crates (present in the graph, not exercised by the native build):**
`wasm-bindgen`(+macro/support/shared) 0.2.126 · `js-sys` 0.3.103 · `web-time` 1.1.0 · `sqlite-wasm-rs`
0.5.5 · `rsqlite-vfs` 0.1.1 · `wasi` 0.11.1 · `wasip2` 1.0.4 · `wit-bindgen` 0.57.1 · `zerocopy`(+derive)
0.8.54.

*(Remaining micro-utilities — `anymap3` 1.1.0, `bit-set`/`bit-vec` 0.10.0/0.9.1, `castaway` 0.2.4,
`compact_str` 0.9.1, `convert_case` 0.10.0, `erased-serde` 0.4.10, `dyn-eq` 0.1.3, `encode_unicode` 1.0.0,
`monostate`, `portable-atomic`(+util) 1.14.0/0.2.7, `ppv-lite86` 0.2.21, `string-interner` 0.20.0, `typeid`
1.0.3, `typenum` 1.20.1, `unit-prefix` 0.5.2, `nom`(+language/locate) 8.0.0/0.1.0/5.0.0 & `nom` 7.1.3,
`static_assertions` 1.1.0, `zmij` 1.0.23, `stringprep`-style unicode helpers — all pure mechanical plumbing.)*
All versions [VERIFIED] from `server/Cargo.lock` / `cli/Cargo.lock` / `hook/Cargo.lock`.
