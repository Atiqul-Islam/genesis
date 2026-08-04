# Contributing to Genesis

Thanks for your interest in improving Genesis — the Claude Code agent-builder plugin.
This guide covers how the repository is laid out, how to build and test each part, the
code style we hold to, and the standard we expect every contribution to meet.

Genesis is MIT-licensed (see [`LICENSE`](./LICENSE)). By contributing, you agree that
your contributions are licensed under the same terms.

---

## The four hard rules (the contribution standard)

Every change to Genesis — code, hooks, docs, or agents — is held to the same four rules
Genesis itself enforces on the agents it builds. A pull request that violates them will
be sent back regardless of how small it is:

1. **No shortcuts.** Build the complete thing. Don't defer, minimize, sample, scope-down,
   or ship "good enough" without explicit sign-off. If a change is too big for one PR,
   split it into complete slices — don't cut the work.
2. **No speculation.** Don't guess or infer behavior. Verify against a primary source
   (the actual code, the real output, the upstream docs). If something is genuinely
   unclear, stop and ask in an issue rather than assuming.
3. **Use the docs / all relevant expertise.** The `docs/` and `expertise/` directories
   are the project's ground truth. Read the relevant material before changing behavior it
   describes.
4. **Production-ready and tested.** Accurate, complete, verified. New behavior ships with
   tests; a bug fix ships with a regression test that fails before the fix and passes
   after.

---

## Repository layout

```
.claude-plugin/   Plugin manifest (added during publish)
agents/           Plugin-shipped agents: sensei, method
team/             Source for the agents (persona.md, behavior.md, skills/)
skills/           Genesis skills (spec-forge workflow, expertise skills, ...)
hooks/            Plugin hook config (hooks.json — invokes genesis-hook via the launcher's
                  --run-hook) + the plugin/scaffold tests
server/           Rust MCP memory server (genesis-memory-server)
hook/             Rust enforcement-hook binary (genesis-hook): inject/gate/enforce-research/validate
cli/              Rust installer/orchestrator (genesis-cli): assemble/bootstrap/promote/install/
                  build-plugin-agents + the session-copy pipeline (capture/store/embed/build-session-agent)
bin/              Node launcher (genesis-memory.js) — downloads/runs the release binaries; also
                  --stage-hook/--stage-cli (stage a binary) and --run-hook/--run-cli (exec one)
scripts/          Helper scripts (e.g. fetch-model.mjs)
expertise/        Verified expertise reports the agents read
docs/             Architecture, workflow, and the publish plan
test/             Shared test assets: BDD features + the CRAP-gate tooling (Node .mjs)
```

There are two toolchains: **Node.js** — the launcher, the installer/assembler, the plugin's
hook resolver, and the session-copy pipeline, which is the only runtime a user's machine needs
— and **Rust**, the `server/` and `hook/` crates, which maintainers/CI build into the
memory-server + genesis-hook binaries that end users receive prebuilt as GitHub Release assets.
There is no build step for the agents/skills/docs themselves — they are Markdown and JSON.

---

## Prerequisites

**To use Genesis** (end users): **Claude Code** and **Node.js**. That's it — Genesis has no
Python runtime and requires no Python. The memory server + hooks ship as prebuilt binaries
delivered as GitHub Release assets, so end users don't need Rust either.

**To develop the memory server** (maintainers / CI):

- **Rust** — `rustc` / `cargo` (pinned by `rust-toolchain.toml`) for `server/` and `hook/`.
  Only needed to *build* them; end users get the prebuilt binaries from the GitHub Release.
- **Node.js** — for the hooks, the installer/assembler, the session-copy pipeline, and their
  tests. These have **no third-party runtime dependencies**; tests run against Node's built-ins.
- **Windows is a first-class target.** Genesis is expected to work on Windows, macOS, and
  Linux. Don't introduce POSIX-only assumptions (hard-coded `/` paths, bash-only scripts) without
  a cross-platform equivalent. Prefer `node:path` (`path.join`) over string-concatenated paths.

Optional, only for the server's full quality gate:
`cargo install cargo-llvm-cov rust-code-analysis-cli`.

---

## Building

### Rust memory server

```
cd server
cargo build            # debug
cargo build --release  # optimized (what the plugin ships)
```

The model weights are **not** committed. To run the server against the real model, fetch
it first (pinned Hugging Face revision):

```
node scripts/fetch-model.mjs
```

This downloads `onnx/model.onnx` and `tokenizer.json` for
`sentence-transformers/all-MiniLM-L6-v2` into `server/models/`. See
[`NOTICE.md`](./NOTICE.md) for the model's license (Apache-2.0).

### How the memory server reaches end users

Maintainers/CI compile the `server/` and `hook/` crates per platform and publish the binaries
as **GitHub Release assets** for the version tag — `genesis-memory-server-<key>`,
`genesis-hook-<key>`, the model (`model.onnx` + `tokenizer.json`), and a `SHA256SUMS` manifest —
using only the auto-provided `GITHUB_TOKEN` (no registry account or token). The plugin's
`.mcp.json` launches the Node launcher `bin/genesis-memory.js`, which downloads the matching
assets for the consumer's platform on first use, SHA256-verifies them, caches them per-user, and
execs the server. End users never build Rust and never touch npm.

> **Status: pre-release / beta.** The beta GitHub Release is **not published yet**, so the
> launcher's download will 404 today. Build `server/` and `hook/` locally (above) and point the
> launcher at them via `GENESIS_MEMORY_BIN` / `GENESIS_HOOK_BIN` / `GENESIS_MODEL_DIR` while in beta.

### Rust installer/orchestrator (`cli/` → genesis-cli)

```
cd cli
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release      # unit + integration (assemble/promote/bootstrap/build-plugin-agents/drift)
```

The integration tests stage the native binaries via the launcher; set `GENESIS_HOOK_BIN` and
`GENESIS_CLI_BIN` to your built binaries first (see below) to exercise the real staging path.

### Node component (the launcher only)

No build. The launcher (`bin/genesis-memory.js`) runs directly under `node` — it is the one irreducible
Node file (registry-free bootstrap: something must download the first binary). Everything else — the
enforcement hooks, the installer/orchestrator, AND the session-copy pipeline — is the Rust `hook/` and
`cli/` binaries; the launcher execs them via `--run-hook` / `--run-cli`.

---

## Running the tests

**Everything must pass before you open a PR.** New/changed behavior must be covered.

### Rust (the GREEN gate)

```
cd server
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release
```

`cargo test --release` runs the unit tests plus the cucumber BDD suites in
`server/tests/bdd/` (which read the feature files in `test/features/`). The release
profile is required because the suites exercise the real ONNX model and the real spawned
stdio server.

Optional quality gate (CRAP score), as documented in `server/README.md`:

```
cd server
mkdir -p test-results/rca
rust-code-analysis-cli -m -p src/ -O json -o test-results/rca/
cargo llvm-cov --json --release --output-path test-results/llvm-cov.json
node ../test/tools/rust_crap_adapter.mjs   # exits non-zero iff CRAP > 8
```

### Node (plugin scaffold + launcher)

The enforcement hooks, the installer/orchestrator, AND the session-copy pipeline are all Rust
(`hook/`, `cli/`). What remains in **Node** is the fetch-launcher (`bin/genesis-memory.js`) and the
plugin/scaffold checks (`hooks/*.js`); their tests are Node too.

Each test file is a self-contained script that prints `N passed, M failed` and exits
non-zero on failure. Run them directly:

```
# Rust: the memory server + the enforcement-hook binary + the installer/orchestrator
( cd server && cargo test --release )
( cd hook   && cargo test --release )   # 28 unit + 15 CLI end-to-end
( cd cli    && cargo test --release )   # unit + integration (assemble/promote/bootstrap/drift +
                                        #   session-copy: capture/store/embed/pipeline)

# Node: plugin scaffold + launcher
node hooks/test_plugin.js
node hooks/test_vendored_skills.js
node test/launcher.test.js

node team/echo/test_echo.mjs
```

To run them all in one go from the repo root:

```
# Node tests (plugin scaffold, team samples, launcher)
find hooks team test bin \
  \( -name 'test_*.js' -o -name 'test_*.mjs' -o -name '*.test.js' \) -print0 \
  | xargs -0 -n1 node
```

The session-copy pipeline's full round-trip (capture→store→embed) is a `cli` integration test that
auto-runs when the server binary + model are built (`server/target/release` + `server/models`).

---

## Code style

**Rust**
- Formatting is enforced by `cargo fmt` — run it before committing.
- `cargo clippy --all-targets -- -D warnings` must be clean. The crate denies
  `unsafe_code` (one sanctioned, documented exception for registering the `sqlite-vec`
  extension), `unwrap_used`, `expect_used`, `panic`, and `todo`. Return `Result`s and
  handle errors; do not add new `unwrap`/`expect`/`panic` in library code.
- Keep the flat `src/` layout (one file per concern) and document public items
  (`missing_docs` is warned).

**Node**
- Node built-ins only for runtime code. Don't add third-party dependencies to the hooks,
  the installer, or session-copy without discussion.
- Cross-platform paths (`node:path` — `path.join`), UTF-8 explicit on file I/O, no reliance on
  a POSIX-only shell.
- Match the existing style of the file you're editing (these modules favor compact,
  dependency-free scripts).

**Docs / agents / skills**
- Markdown and JSON. Keep the four hard rules intact wherever they appear — they are load
  bearing, not boilerplate.

---

## Opening issues

- **Bugs:** include the OS, `node` version (and `rustc`/`cargo` if the memory server is
  involved), the exact command you
  ran, what you expected, and the full output. A minimal reproduction is worth more than a
  description.
- **Features / changes in behavior:** open an issue describing the problem and the intended
  approach *before* writing a large change, so the design can be agreed on first.
- **Security / credentials:** if you find a committed secret or a vulnerability, report it
  privately to the maintainer rather than opening a public issue.

## Opening pull requests

1. Branch from the default branch; keep each PR focused on one change.
2. Make sure the relevant test gate above passes locally — the Node suites *and* the Rust
   server tests for anything you touched. State in the PR description what you ran and that it
   passed.
3. Include tests for new behavior and a regression test for any bug fix.
4. Keep commits clean and messages descriptive (what changed and why). If a change is
   agent-assisted, that's fine and welcome — attribute it honestly (e.g. a
   `Co-Authored-By:` trailer).
5. Update the relevant docs in `docs/` when you change behavior they describe.
6. Confirm your PR meets the four hard rules at the top of this guide.

Thank you for contributing to Genesis.
