# Third-Party Notices

Genesis itself is licensed under the **MIT License** — see [`LICENSE`](./LICENSE),
Copyright (c) 2026 Atiqul Islam.

This document attributes the third-party software and machine-learning model that
Genesis bundles, downloads, or links against. Nothing here modifies the terms under
which those components are licensed by their respective owners; it records them so
that redistributors of Genesis carry the required notices.

All licenses below were verified against a primary source (the component's official
model card, repository `LICENSE` file, or its published crate metadata on crates.io)
at the URLs cited. Where a component is offered under a dual/multi license
(e.g. `MIT OR Apache-2.0`), the "OR" is the upstream author's, meaning a downstream
user may choose either license.

---

## 1. Embedding model — `sentence-transformers/all-MiniLM-L6-v2`

Genesis's memory server (`genesis-memory-server`) computes sentence embeddings using
the pre-trained model **`sentence-transformers/all-MiniLM-L6-v2`**. The server ships
with / fetches this model's ONNX weights (`onnx/model.onnx`) and tokenizer
(`tokenizer.json`) from a pinned Hugging Face revision.

- **Model:** `sentence-transformers/all-MiniLM-L6-v2`
- **Source:** https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2
- **Pinned revision:** `c9745ed1d9f207416be6d2e6f8de32d1f16199bf`
  (see `scripts/fetch-model.mjs`)
- **License:** **Apache-2.0** — verified from the model card metadata header
  ("License: apache-2.0") at the source URL above on 2026-07-22.
  Apache License 2.0 full text: https://www.apache.org/licenses/LICENSE-2.0

The tokenizer file (`tokenizer.json`) is a data artifact of the same model repository
and is covered by the same Apache-2.0 license.

---

## 2. tract (native inference engine, pure Rust)

The memory server runs the model above through **tract**, a pure-Rust ONNX inference
engine. The `ort` crate provides the API surface, but its backend is swapped from ONNX
Runtime to tract via `ort`'s `alternative-backend` feature together with the `ort-tract`
crate. No ONNX Runtime C++ library is downloaded, built, or shipped — so the binary is a
single self-contained file and builds for every target, including `x86_64-apple-darwin`
(Intel macOS) and the musl targets, for which no ONNX Runtime prebuilt exists.

- **tract** — https://github.com/sonos/tract — **MIT OR Apache-2.0**, Copyright (c) Sonos, Inc.
- **ort-tract** (the tract backend for `ort`) — https://crates.io/crates/ort-tract — **MIT OR Apache-2.0**.

---

## 3. SQLite (bundled)

The memory server stores vectors and metadata in **SQLite**, compiled directly into
the binary via `rusqlite`'s `bundled` feature (through `libsqlite3-sys`).

- **Project:** SQLite — https://www.sqlite.org
- **License:** **Public Domain** (SQLite is dedicated to the public domain by its
  authors — https://www.sqlite.org/copyright.html).

---

## 4. Key Rust dependencies (memory server)

The `genesis-memory-server` crate links the following notable dependencies. Licenses
below were verified from each crate's published metadata on crates.io (the version is
the one resolved in `server/Cargo.lock`). Each crates.io page is reachable at
`https://crates.io/crates/<name>` and the machine-readable license at
`https://crates.io/api/v1/crates/<name>/<version>`.

| Crate | Version | License (SPDX, as published) | Role |
|---|---|---|---|
| `rmcp` | 2.2.0 | Apache-2.0 | Official Rust MCP SDK (stdio server) |
| `rusqlite` | 0.39.0 | MIT | SQLite bindings (bundled build) |
| `libsqlite3-sys` | 0.37.0 | MIT | SQLite native bindings (transitive, bundled) |
| `sqlite-vec` | 0.1.9 | MIT OR Apache-2.0 | SQLite vector / KNN extension |
| `ort` | 2.0.0-rc.13 | MIT OR Apache-2.0 | Inference API (tract backend, linking disabled) |
| `ort-sys` | 2.0.0-rc.13 | MIT OR Apache-2.0 | `ort` sys crate (native linking disabled) |
| `ort-tract` | 0.4.0+0.23 | MIT OR Apache-2.0 | Pure-Rust tract backend for `ort` |
| `tract-onnx` | 0.23.4 | MIT OR Apache-2.0 | Pure-Rust ONNX inference (engine) |
| `tract-core` | 0.23.4 | MIT OR Apache-2.0 | tract inference core |
| `tract-linalg` | 0.23.4 | MIT OR Apache-2.0 | tract linear-algebra kernels |
| `tokenizers` | 0.23.1 | Apache-2.0 | Hugging Face tokenizer |
| `ndarray` | 0.17.2 | MIT OR Apache-2.0 | N-dimensional arrays |
| `bytemuck` | 1.25.1 | Zlib OR Apache-2.0 OR MIT | Byte-level casting for embeddings |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | Serialization framework |
| `serde_json` | 1.0.150 | MIT OR Apache-2.0 | JSON serialization |
| `schemars` | 1.2.1 | MIT | JSON Schema generation |
| `tokio` | 1.53.0 | MIT | Async runtime |
| `anyhow` | 1.0.104 | MIT OR Apache-2.0 | Error handling |
| `thiserror` | 2.0.19 | MIT OR Apache-2.0 | Error `derive` |
| `tracing` | 0.1.44 | MIT | Structured logging |
| `tracing-subscriber` | 0.3.23 | MIT | Logging subscriber |

Development-only dependencies (test harnesses, not distributed in the release binary)
include `cucumber`, `assert_cmd`, `approx`, `insta`, `tempfile`, `sha2`, and `hex`;
these are standard permissively licensed (MIT / MIT-OR-Apache-2.0) crates and are not
linked into the shipped binary.

Each dependency in turn pulls transitive crates. The full, authoritative dependency
graph is captured in `server/Cargo.lock`; per-crate license text can be regenerated
from a checkout with a tool such as `cargo-about` or `cargo-license`.

---

## 5. Node components

Genesis's plugin hook resolver (`hooks/run.js`), installer (`install/`), session-copy
pipeline (`session_copy/`), and repo test tooling (`test/tools/*.mjs`) are
implemented in **Node.js** and run on Node's built-in standard library alone —
including `node:sqlite` for local storage. They declare and require **no third-party
runtime dependencies** and are covered by Genesis's own MIT license. Running these
components requires Node.js (alongside Claude Code); they do **not** require Python.

The **enforcement hooks themselves** are a native Rust binary (`genesis-hook`, crate at
`hook/`; deps: `serde_json` + `regex`), shipped prebuilt as GitHub Release assets —
no runtime is required to invoke it.

---

## 6. Vendored skills — `superpowers` plugin

Genesis's `/spec-forge` workflow (skills `spec-forge`, `forge-dev-agent`,
`forge-review-agent`) invokes a set of discipline skills that originate in the
**superpowers** plugin. So that Genesis ships self-contained (a fresh install has no
dependency on superpowers being installed), the complete transitive closure of those
skills is vendored verbatim into `skills/`, with their inter-skill references rewired
to Genesis's own bare-name convention.

- **Component:** superpowers — Core skills library for Claude Code
- **Version vendored:** `6.1.1`
- **Copyright:** © 2025 Jesse Vincent
- **License:** **MIT License** — verified from the plugin's `LICENSE` file and
  `.claude-plugin/plugin.json` (`"license": "MIT"`) on 2026-07-22. The full license
  text is preserved at [`skills/VENDORED-superpowers-LICENSE`](./skills/VENDORED-superpowers-LICENSE).
- **Source:** https://github.com/obra/superpowers
  (per the plugin's `.claude-plugin/plugin.json` `homepage`/`repository` fields).

The 11 vendored skills (each a directory under `skills/`):

| Skill | Skill | Skill |
|---|---|---|
| `brainstorming` | `writing-plans` | `using-git-worktrees` |
| `test-driven-development` | `systematic-debugging` | `verification-before-completion` |
| `requesting-code-review` | `receiving-code-review` | `finishing-a-development-branch` |
| `subagent-driven-development` | `executing-plans` | |

Support files bundled with these skills (e.g. `requesting-code-review/code-reviewer.md`,
`systematic-debugging/*.md` + `find-polluter.sh`, `brainstorming/scripts/*`,
`subagent-driven-development/scripts/*` + prompt templates, `writing-plans/plan-document-reviewer-prompt.md`,
`test-driven-development/testing-anti-patterns.md`) are copied verbatim under the same
MIT license. Minor edits were limited to (a) rewiring `superpowers:<skill>` skill
references to Genesis's bare-name form, and (b) removing one now-dangling relative-path
pointer in `executing-plans` to the non-vendored `using-superpowers` skill. No
functional logic of the vendored skills was changed.

---

_Licenses verified 2026-07-22 against the primary sources cited above. If you
redistribute Genesis, retain this notice together with the `LICENSE` file._
