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
hooks/            Enforcement hooks — Node (.js): inject / gate / validate /
                  review / adherence / enforce_research / session_pointer / agent_ident
install/          Installer + assembler (Node): install.js, bootstrap.js, assemble.js
session_copy/     Session-copy pipeline (Node): capture.js / embed.js / store.js /
                  build_session_agent.js
server/           Rust MCP memory server (genesis-memory-server)
npm/              npm packaging for the memory server — the launcher package
                  (@xcidos/genesis-memory-server) plus per-OS/arch binary packages and
                  the model package (@xcidos/genesis-memory-model)
scripts/          Helper scripts (e.g. fetch-model.mjs)
expertise/        Verified expertise reports the agents read
docs/             Architecture, workflow, and the publish plan
test/             Shared test assets: BDD features + the CRAP-gate tooling (Node .mjs)
```

There are two toolchains: **Node.js** — the hooks, the installer/assembler, and the
session-copy pipeline, which is the only runtime a user's machine needs — and **Rust**, the
`server/` crate, which maintainers/CI build into the memory-server binary that end users
receive prebuilt via npm. There is no build step for the agents/skills/docs themselves — they
are Markdown and JSON.

---

## Prerequisites

**To use Genesis** (end users): **Claude Code** and **Node.js**. That's it — Genesis has no
Python runtime and requires no Python. The memory server ships as a prebuilt binary delivered
through npm, so end users don't need Rust either.

**To develop the memory server** (maintainers / CI):

- **Rust** — `rustc` / `cargo` 1.97 or newer (for `server/`). Only needed to *build* the
  server; end users get the prebuilt binary via npm.
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

Maintainers/CI compile the `server/` crate per platform and publish the binaries as npm
packages: a launcher package (`@xcidos/genesis-memory-server`) with per-OS/arch binary
packages as `optionalDependencies`, plus a model package (`@xcidos/genesis-memory-model`).
The plugin's `.mcp.json` launches the server with `npx -y @xcidos/genesis-memory-server`, so
end users never build Rust — npm resolves the right prebuilt binary for their platform.

> **Status: pre-release / beta.** These npm packages are **not published to the registry
> yet**, so `npx @xcidos/genesis-memory-server` will not resolve today. The npm/npx flow above
> describes the intended distribution mechanism, not a currently-installable path. Build the
> server locally from `server/` (above) while the packages are in beta.

### Node components (hooks, installer, session-copy)

No build. The hooks (`hooks/*.js`), the installer/assembler (`install/*.js`), and the
session-copy pipeline (`session_copy/*.js`) all run directly under `node`.

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

### Node (hooks, installer, session-copy)

The hooks, installer/assembler, and session-copy pipeline are **Node** (`hooks/*.js`,
`install/*.js`, `session_copy/*.js`), and their tests are Node too.

Each test file is a self-contained script that prints `N passed, M failed` and exits
non-zero on failure. Run them directly:

```
node hooks/test_gate.js
node hooks/test_inject.js
node hooks/test_validate.js
node hooks/test_review.js
node hooks/test_adherence.js
node hooks/test_enforce_research.js
node hooks/test_session_pointer.js

node install/test_bootstrap.js
node install/test_portability.js
node "npm/@xcidos/genesis-memory-server/test/launcher.test.js"

node session_copy/test_capture.js
node session_copy/test_embed.js
node session_copy/test_store.js
node session_copy/test_build_session_agent.js
node session_copy/test_pipeline.js

node team/echo/test_echo.mjs
```

To run them all in one go from the repo root:

```
# Node tests (hooks, installer/assembler, session-copy, team samples, memory-server launcher)
find hooks install session_copy team npm \
  \( -name 'test_*.js' -o -name 'test_*.mjs' -o -name '*.test.js' \) -print0 \
  | xargs -0 -n1 node
```

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
