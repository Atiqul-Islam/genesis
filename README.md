# Genesis

**An agent-builder for Claude Code** — a team of three agents (Sensei, Method, and Mneme) that builds, tests, installs, and remembers specialized Claude Code agents with enforced expertise and per-agent semantic memory.

**Genesis is self-hosting: its own agents built it, under the same rules it enforces on everything else.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
![Claude Code plugin](https://img.shields.io/badge/Claude%20Code-plugin-informational)
[![Latest release](https://img.shields.io/github/v/release/Atiqul-Islam/genesis?include_prereleases&sort=semver&label=release)](https://github.com/Atiqul-Islam/genesis/releases)
![Status: beta](https://img.shields.io/badge/status-beta-yellow)

You describe the agent you want. Genesis interviews you, writes it test-first, installs it into your project, and wires it with enforced rules and its own memory — so the agent you get is one you (and your teammates) can reproduce.

```text
# In Claude Code, from your project directory:

/plugin marketplace add Atiqul-Islam/genesis
/plugin install genesis@genesis

# Then talk to Sensei:
> Use sensei to build an agent that runs my test suite and summarizes the failures.
```

Sensei interviews you, delegates the authoring to Method (who writes the agent's tests **first**), assembles and installs it into `.claude/agents/`, and hands back a working, tested agent.

---

## What it is

Genesis is a **Claude Code plugin**. Installing it adds three agents to your project:

- **Sensei** — the coordinator you talk to. It gathers and verifies every requirement, decides the plan with you (single agent or a team — your call, not its), and orchestrates the build. It never authors prompts itself and never guesses.
- **Method** — the test-first craftsman. It writes each agent's persona, behavior, and skills, writes the acceptance tests before the agent, and ships nothing until they pass.
- **Mneme** — the memory specialist. It structures each memory the moment it's written and keeps every agent's store contradiction-free through deterministic bi-temporal supersession, so recall stays trustworthy across sessions. Memory is its own discipline, so it's its own agent.

The agents Genesis produces are ordinary Claude Code agents — plus two things Genesis wires in for you: **expertise the agent is forced to apply**, and **memory that persists across sessions**.

## Why

Hand-rolling a Claude Code agent means copy-pasting a persona from a blog post, hoping the prompt holds, and having no way to prove the next person gets the same behavior. There's no test, no enforced boundary, and the agent forgets everything between sessions.

Genesis's answer is to treat an agent like software you build, not a snowflake you paste:

- its behavior is **tested before it ships**,
- its rules are **enforced by hooks**, not by hoping the model remembers them, and
- its knowledge and history live in a **per-agent store** it can recall from later.

## Install

From inside Claude Code, in the repository you want the agents in:

```text
/plugin marketplace add Atiqul-Islam/genesis
/plugin install genesis@genesis
/reload-plugins
```

The first command registers this repository as a plugin marketplace; the second installs the `genesis` plugin from it. See [Requirements](#requirements) below — the builder itself needs only Claude Code and Node.js; the memory server has a couple of extra notes.

## Build your first agent

After install, start Claude Code in your project and ask Sensei to build something. A first run looks like this:

```text
> Use sensei to build an agent that reviews my pull-request diffs for security issues.
```

1. **Sensei interviews you.** It asks for the goal, the done-criteria, the tools the agent needs, and when it should escalate — then restates every requirement back and proceeds only on what you confirm.
2. **You choose the expertise.** Sensei runs its `research-expertise` step and proposes what the agent must know; you add, drop, or change it, and decide whether to deep-research it.
3. **Method authors it test-first.** Sensei hands Method a task-spec. Method writes the acceptance tests first, then the smallest persona/behavior/skills that make them pass, and returns only when they do.
4. **Sensei assembles and installs it.** The finished agent lands in `.claude/agents/`, with its enforcement hooks wired and — if you asked for it — its memory.
5. **You use it.** The new agent is now available in Claude Code, held to its rules by the same hooks, with its own memory.

Nothing is built on an unconfirmed assumption, and nothing untested is delivered — those are properties of the two agents, not promises in this README (see `agents/sensei.md`, `agents/method.md`).

## What you get

Each item below is backed by a file in this repo, named so you can check it.

- **A two-agent team, not a prompt template.** Coordination (Sensei) and authoring (Method) are separate roles with separate tools; Method is test-first and ships nothing untested.
  *Proof:* `agents/sensei.md`, `agents/method.md`, `skills/build-agent/SKILL.md`.

- **Enforced expertise (fail-closed).** Every agent is required to load named expertise and, before it can end a turn, declare which rules it applied. The declaration is checked against a rule manifest, and the cited evidence is spot-checked against the files the agent produced — a fabricated rule id or made-up evidence blocks the turn.
  *Proof:* the `genesis-hook` binary (`validate`), `expertise/required.json`, `expertise/manifests/`.

- **Deterministic house rules.** A pre-write gate blocks any edit that contains a banned phrase, a credential value, or that exceeds the persona/behavior/`CLAUDE.md` line budget — enforced by regex at the moment of writing, not by trusting the model to remember.
  *Proof:* the `genesis-hook` binary (`gate`).

- **Per-agent semantic memory.** Genesis ships a Rust MCP memory server — SQLite + `sqlite-vec` KNN over local ONNX embeddings, fully offline. Every agent gets its own `store` / `recall` / `consolidate`, scoped by agent id, separate from the transient session context.
  *Proof:* `server/`, `.mcp.json`, `server/README.md`.

- **Research is required before a build.** Before Sensei can assemble a new agent, a hook confirms the `research-expertise` step actually ran this session — you cannot skip choosing (and confirming) what the agent should know.
  *Proof:* the `genesis-hook` binary (`enforce-research`), `skills/research-expertise/SKILL.md`.

- **A spec-driven build workflow, included.** A supervisor-led, test-first (RED → GREEN, one commit per task) multi-agent workflow — the same workflow Genesis used to build its own memory server.
  *Proof:* `skills/spec-forge/SKILL.md`, `skills/spec-build/SKILL.md`, `server/README.md`.

## How it works

Genesis is wired at the plugin level, because a plugin-shipped agent can't carry its own enforcement hooks. So the hooks live in `hooks/hooks.json` and derive the active agent from each event. The deterministic hooks are a single native Rust binary — **`genesis-hook`** (busybox-style subcommands) — shipped in the same GitHub Release as the memory server, so they run as a ~2–10 ms native spawn instead of a Node process per tool call:

- **SubagentStart → `genesis-hook inject`** delivers the checkable house rules and pointers to the expertise store into the agent's context (the full expertise reports stay on disk and are read on demand).
- **PreToolUse (Write/Edit) → `genesis-hook gate`** blocks a write that violates a checkable rule, and re-surfaces the governing rules right before the write.
- **PreToolUse (Bash) → `genesis-hook enforce-research`** blocks assembling a non-built-in agent unless the expertise-research step ran.
- **SubagentStop → `genesis-hook validate` + a built-in `agent` review hook** refuse to let an agent finish while a checkable rule is still violated or its expertise declaration isn't credible, and add an independent LLM review pass (a fast Haiku model that reads the produced artifacts) for the judgment rules a regex can't check.

Built agents invoke the binary directly (baked into their frontmatter by the assembler); the plugin's own team resolves it through a tiny cross-platform shim. The launcher + installer that fetch and stage the binaries are Node.

The **native binaries + the embedding model are distributed as GitHub Release assets** — not committed to the repo, and not on any package registry. The plugin's `.mcp.json` runs a small Node launcher (`bin/genesis-memory.js`) that, on first use, downloads the `genesis-memory-server` (and `genesis-hook`) for your OS/arch plus the pinned model from the matching release, **SHA256-verifies each against a published `SHA256SUMS`**, caches them per-user, and execs the server — the MCP stream passing through untouched. The assets are public: **no npm, no registry account, no token.** A build-from-source path in [Requirements](#requirements) remains as a fallback (or for an as-yet-unpackaged platform).

## Requirements

- **Claude Code with plugin support** (the `/plugin` marketplace system).
- **Node.js on your PATH.** The memory-server launcher (downloads + runs the binaries), the installer, and the plugin's hook resolver run on **Node 18+**; the optional **session-copy** feature reads SQLite via the built-in `node:sqlite` and needs **Node 24+**. The enforcement hooks themselves are a native Rust binary (no per-call runtime); built agents invoke it directly. No Python runtime is required.
- **For the memory server:** prebuilt native binaries + the model are published as **GitHub Release assets** (macOS and Linux on x64/arm64, Windows on x64; more targets rolling out); the launcher downloads and SHA256-verifies the right ones for your platform. As a fallback — or on an as-yet-unpackaged platform — build from source with **Rust (cargo 1.97+)**: `cd server && cargo build --release` (and `cd hook && cargo build --release`), fetch the model with `node scripts/fetch-model.mjs`, then point the launcher at them via `GENESIS_MEMORY_BIN`, `GENESIS_HOOK_BIN`, and `GENESIS_MODEL_DIR` (see [`CONTRIBUTING.md`](./CONTRIBUTING.md)). The agent builder works without the memory server; memory is wired into a built agent only when you ask for it.

## Status

Genesis is in **beta** — the latest release is **`v0.2.0-beta.2`** (of 17 published releases). It is honest about what that means:

- The **three-agent builder** (Sensei, Method, Mneme), the enforcement hooks, and the spec-driven workflow are implemented and have their own test suites (the Rust `genesis-hook` suite, the Rust server suite, and the Node installer/session-copy/plugin tests).
- The **memory server (v1)** — `store` / `recall` / `consolidate` — is built and tested: 86 unit tests and 17 BDD scenarios pass in release against the real ONNX model, real SQLite, and the real spawned stdio server. Its `consolidate` summarize/evict pass is deferred to v2 (see `server/README.md`).
- The **binary distribution is live:** prebuilt `genesis-memory-server`, `genesis-hook`, and `genesis-cli` plus the pinned model ship as SHA256-verified GitHub Release assets, downloaded by the launcher on first use. Build-from-source remains a fallback.

No usage, star, or production-deployment claims are made here because none would be verifiable yet.

## Docs

- [`docs/architecture.md`](./docs/architecture.md) — how the plugin, agents, hooks, and memory server fit together.
- [`docs/multi-agent-workflow.md`](./docs/multi-agent-workflow.md) — the Sensei → Method build flow in detail.
- [`docs/spec-driven-development.md`](./docs/spec-driven-development.md) — the spec-first build workflow.
- [`server/README.md`](./server/README.md) — the memory server: design, build, and test gates.

## Contributing

Contributions are welcome — Genesis holds every change to the same four rules it enforces on the agents it builds (no shortcuts, no speculation, use the docs, and nothing ships untested or unfinished). See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for the repository layout, build/test instructions for both the Rust and Node toolchains, and the code style.

## License

Genesis is licensed under the **MIT License** — see [`LICENSE`](./LICENSE). Third-party components it bundles or downloads (the embedding model, ONNX Runtime, SQLite, and key Rust crates) are attributed in [`NOTICE.md`](./NOTICE.md); if you redistribute Genesis, keep both files with it.
