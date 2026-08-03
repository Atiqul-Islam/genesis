# Genesis distribution spec (beta)

Status: **implemented; alpha published to npm (`0.1.0-alpha.1`).** Beta (branch `beta`) rewrites the
enforcement hooks Node → native Rust AND moves binary distribution from npm → **GitHub Release assets**
(no registry, no token) — see "Beta update" below. Owner: Atiqul. Author: agent.

## Decision (alpha — partly superseded, see Beta update)

- ~~**Distribute the Rust memory server as npm platform-specific packages** (`optionalDependencies`,
  `os`/`cpu`/`libc`-gated), launched via **`npx`**.~~ **SUPERSEDED in beta** → binaries + model ship as
  **GitHub Release assets** downloaded by a Node launcher; npm is dropped entirely (no registry account/token).
- **Node.js is the one required prerequisite** (the accepted baseline for the Claude Code + MCP ecosystem).
- **Hooks:** Python → Node (alpha) → **native Rust** (beta — see below).
- Kills the hand-rolled Python launcher, the `python3` hardcoding, and the local-Rust-build requirement.

### Beta update (branch `beta`) — native-Rust hooks + GitHub-Releases distribution

Two changes supersede the alpha's npm/Node model:

**1. Enforcement hooks → native Rust.** The alpha's Node hooks are replaced by one binary `genesis-hook`
(crate `hook/`):

- Deterministic hooks — `inject`, `gate`, `enforce-research`, `validate`, `session-pointer` — are
  busybox-style subcommands. Cold-spawn ~2–10 ms vs the Node hooks' ~65 ms; `validate` no longer walks
  `node_modules`/`target` (measured **62.5 s → 0.55 s** on a 21k-file tree). Byte-identical decisions to the
  retired Node hooks (17/17 parity before deletion; deps `serde_json` + `regex`).
- `review` moved off `claude -p` (×2 per expertise) onto a Claude Code built-in **`agent` hook** (Haiku,
  tool-capable — reads the artifacts + manifests itself).
- Built agents call the binary directly (`assemble.js` bakes `${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook`);
  the plugin's static `hooks.json` resolves it via the cross-platform `hooks/run.js` shim.

**2. Distribution: npm → GitHub Release assets (no registry, no token).**

- `release.yml` builds both binaries per platform + fetches the model, then publishes them as **GitHub
  Release assets** for the tag (`genesis-memory-server-<key>`, `genesis-hook-<key>`, `model.onnx`,
  `tokenizer.json`, `SHA256SUMS`) using only the auto-provided `GITHUB_TOKEN`. The npm publish job, the
  `@xcidos` platform packages, and `generate-platform-packages.mjs` are **deleted**.
- The launcher `bin/genesis-memory.js` (stdlib-only) downloads the matching assets for the consumer's
  platform on first use, **SHA256-verifies** them, caches them per-user (`~/.cache/genesis/v<ver>/`), and
  execs the server. `--stage-hook <dir>` stages the hook binary. The release version is a constant in the
  launcher, bumped per release. `.mcp.json` (plugin + generated) launches `node <launcher>` instead of `npx`.
- `bootstrap.js` stages the hook binary via the launcher's `--stage-hook` (download, or `GENESIS_HOOK_BIN`
  for dev). Consumers need only Node + network on first run; no npm account anywhere.
- Tests: hook crate 28 unit + 13 CLI; Node `test/launcher.test.js`, `test_bootstrap`, `test_plugin`,
  `test_portability` updated. `ci.yml` builds/tests both Rust crates + the Node surface.

## Target layout (alpha npm model — superseded; see Beta update)

```
@xcidos/genesis-memory-server            # thin launcher (bin/genesis-memory.js) — resolves + spawns
  optionalDependencies (exact versions):
    @xcidos/genesis-memory-server-darwin-arm64      os:darwin  cpu:arm64
    @xcidos/genesis-memory-server-darwin-x64        os:darwin  cpu:x64
    @xcidos/genesis-memory-server-linux-x64-gnu     os:linux   cpu:x64   libc:glibc
    @xcidos/genesis-memory-server-linux-arm64-gnu   os:linux   cpu:arm64 libc:glibc
    @xcidos/genesis-memory-server-linux-x64-musl    os:linux   cpu:x64   libc:musl
    @xcidos/genesis-memory-server-linux-arm64-musl  os:linux   cpu:arm64 libc:musl
    @xcidos/genesis-memory-server-win32-x64         os:win32   cpu:x64
    @xcidos/genesis-memory-server-win32-arm64       os:win32   cpu:arm64
  dependencies:
    @xcidos/genesis-memory-model                    # 83 MB ONNX weights + tokenizer, platform-independent, shared once
```

`bin/genesis-memory.js` = esbuild's `generateBinPath` pattern: build `${platform}-${arch}(-${libc})`,
`require.resolve('@xcidos/genesis-memory-server-<key>/<exe>')`, `spawn(..., {stdio:'inherit'})`.
Backstop: `ldd --version` musl check (biome pattern); `GENESIS_MEMORY_BIN` dev override.

`.mcp.json` (plugin AND generated repo `.genesis`):
```json
{ "mcpServers": { "genesis-memory": { "command": "npx", "args": ["-y", "@xcidos/genesis-memory-server"] } } }
```

## Self-contained binary (per platform, one file)

- **SQLite:** `rusqlite` `bundled` (compiles amalgamation in). ✅ already pinned `=0.39.0` (avoids `cfg_select`).
- **Inference engine — RESOLVED: pure-Rust tract.** ONNX Runtime was rejected: pyke ships no CPU-static
  prebuilt for `x86_64-apple-darwin` or the musl targets (verified from `ort-sys` `dist.txt`, feature-set
  `none`), and its default Windows prebuilt links the DirectML GPU EP the server never uses. The engine is
  now the pure-Rust **tract** backend (`ort` `alternative-backend` + `ort-tract`): no C++ runtime, no
  prebuilt matrix, no cmake, no sidecar dylib — every target builds from one `cargo` path and inference is
  compiled into the single binary. tract loads the same `onnx/model.onnx`; the golden vectors were
  regenerated on tract with documented provenance (`server/tests/golden/PROVENANCE.md`).
- **Windows CRT:** `+crt-static` (via `.cargo/config.toml` for `*-pc-windows-msvc`) — removes the
  `MSVCP140`/`VCRUNTIME140` dependency (verified present today; breaks on a fresh Windows).
- **Linux:** ship **both** `-gnu` (built on an old-glibc floor: `ubuntu-22.04`/manylinux/`cargo-zigbuild`)
  **and** `-musl` (fully static).
- **macOS:** **codesign (Developer ID, hardened runtime) + notarize + staple** — unsigned arm64 is
  killed by the kernel. No GPU EPs.

## CI (`.github/workflows/release.yml` + `ci.yml`) — ✅ implemented

`release.yml` (on tag `v*`):
1. 8-row build matrix (all triples), all from the pure-Rust **tract** path: native `cargo` for host-arch
   targets, `cargo-zigbuild` for cross / musl / low-glibc-floor. No ONNX-Runtime prebuilt special-casing.
2. macOS rows: `codesign --options runtime` → `notarytool submit --wait` (ad-hoc sign if no Apple secrets).
3. Stage each binary into `npm/@xcidos/genesis-memory-server-<key>/`; `generate-platform-packages.mjs`
   syncs versions.
4. Publish job: `npm publish --provenance --access public` — platform packages first, then model, then the
   launcher last (its exact-version deps must resolve).

`ci.yml` (on push/PR): the server + `genesis-hook` crates `cargo build`/`cargo test` on 3 OSes (the hook crate
also `clippy -D warnings` + `fmt --check`), Node parse-check + installer/session-copy/plugin unit tests, and a
"no runtime Python" assertion. A regression guard fails if the server binary links a dynamic inference runtime
(`onnxruntime`/`DirectML`).

## Remaining source fixes (fold in)

- **Delete** `bin/genesis-memory` (Python launcher) and the `CHECKSUMS_SHA256`/`PINNED_VERSION` machinery.
- **Rewrite** the 5 hooks (`inject`, `gate`, `enforce_research`, `validate`, `review`) + `session_pointer`,
  `agent_ident` from Python → Node; `hooks.json` invokes `node ${CLAUDE_PLUGIN_ROOT}/hooks/<x>.js`.
- ✅ Ported **`assemble.py` → `assemble.js`**: emits `node ...` hook commands + braced `${CLAUDE_PROJECT_DIR}`;
  wraps `relpath` in try/catch for cross-drive Windows. (`build_plugin_agents.py` + `install.py` ported too.)
- ✅ Ported **`bootstrap.py` → `bootstrap.js`**: dropped the "build it first" exit and the binary/model copy;
  generated repo `.mcp.json` uses `npx -y @xcidos/genesis-memory-server` (no local build).
- **`server/src/embed.rs`:** gate `use std::os::unix::fs::PermissionsExt` + the mode assertion behind `#[cfg(unix)]`.
- **`rust-toolchain.toml`** (pin stable) + repo-root **`.gitattributes`** (`* text=auto`, `*.sh`/`*.js` `eol=lf`).
- ✅ Ported `test_portability.py` → `test_portability.js` (asserts the Node command, not `python3`) and
  `test_bootstrap.py` → `test_bootstrap.js` (asserts the npx `.mcp.json`); added a Node launcher test at
  `npm/@xcidos/genesis-memory-server/test/launcher.test.js`.
- Retired the Linux-only `scripts/fetch-model` bash **and** the `scripts/fetch-model.py` Python in
  favour of a single cross-platform Node `scripts/fetch-model.mjs` (Node stdlib only — no curl,
  sha256sum, shell, or Python), so the only runtime a user's machine needs is Node.

## Needs Atiqul (outward-facing / credentials)

- npm scope `@xcidos` (or chosen) + an npm publish token (CI secret).
- Apple Developer ID cert + notarization creds (for signed macOS binaries) — or ship macOS later.
- Decision to publish the packages publicly (this is the real beta publish).

## Migration order

1. Node interpreter scheme + hooks→Node + assemble/bootstrap wiring.
2. Self-contained build (ORT CPU-only/static + `+crt-static` + musl/glibc); embed.rs `#[cfg(unix)]`; `rust-toolchain.toml`.
3. npm package scaffold (launcher `bin.js` + platform-package template + model package).
4. Rewrite `release.yml` (matrix, sign, publish `--provenance`) + add `ci.yml`.
5. First publish → `.mcp.json`/bootstrap point at `npx`; delete the Python launcher.
6. Verify a clean install (`npx`) on Windows + Linux here; macOS via CI.
