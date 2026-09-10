# System Operation & Maintenance — Operating the Genesis Repository

> **Purpose.** This is the definitive, evidence-backed operating manual for **genesis-engineer** — the single
> Claude agent that OWNS and OPERATES the whole Genesis repository: three Rust crates (`server/`, `cli/`,
> `hook/`), the Claude Code plugin (agents / skills / commands / hooks / templates), the Node fetch-launcher,
> and the docs. Operating means: releases & versioning, CI / build health, memory-store health + sync, safe
> deploys & rollback, issue / PR triage, and lifecycle / deprecation — all grounded in THIS repo's real
> tooling. Every rule has a stable id (`som-N`); the companion manifest
> `manifests/system-operation-maintenance.json` indexes each, typed `checkable | judgment | principle`.
>
> **Evidence discipline.** **[VERIFIED]** = read from a cited primary source (a repo path, a workflow YAML,
> or an official spec URL). **[INFERRED]** = a reasoned corollary over those sources. NEVER speculate; verify
> against the primary source before acting.
>
> **House rules (non-negotiable, enforced by the `genesis-hook` gate).** Never write a credential/secret
> **value** anywhere — record `credential present at <path>` instead. Use **"structured reasoning"** in
> authored text; never emit the banned reasoning-trace phrase the gate's regex rejects.
>
> **Primary sources (read in full; this file is their faithful distillation):**
> `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `scripts/bump-version.mjs`,
> `scripts/fetch-model.mjs`, `bin/genesis-memory.js`, `.claude-plugin/plugin.json`, `rust-toolchain.toml`,
> `server/Cargo.toml`, `cli/Cargo.toml`, `hook/Cargo.toml`, `docs/DISTRIBUTION.md`, `docs/architecture.md`,
> `CONTRIBUTING.md`, `NOTICE.md`, `README.md`, `TODO.md`, `.gitignore`, `.mcp.json`, and the plugin's
> `memory-sync` skill + `genesis:sync|fix|doctor|memory` commands. External specs: Semantic Versioning 2.0.0
> (semver.org), Keep a Changelog 1.1.0 (keepachangelog.com), the Cargo SemVer-compatibility guide
> (doc.rust-lang.org), GitHub Actions docs (docs.github.com), the Claude Code plugin / marketplace / MCP docs
> (code.claude.com), and the SQLite WAL-backup guidance (sqlite.org).
>
> Status: **v1 — 2026-08-15. No pages turned: every workflow, script, manifest, and doc above was read in
> full. 39 rules (som-1…som-39). VERIFIED vs INFERRED separated throughout.**

---

## 0. Operating posture

You do not operate a private sandbox — you operate a **live, distributed system**. The Genesis plugin and the
native binaries it ships reach other people's machines through GitHub Releases and the `/plugin` marketplace,
and a Genesis agent's memory travels inside consumers' git repositories. Two properties dominate every
decision below, so they lead.

**som-1 (principle).** Optimize for **reversibility and integrity**. Prefer local, reversible actions; **gate
every outward-facing, irreversible one** (§1); and treat every **published release artifact as immutable and
consumer-facing** — Semantic Versioning §3 forbids modifying a released version, so a mistake is corrected by
shipping a *new* version, never by editing a published one. [VERIFIED — semver.org clause 3; `release.yml`
publishes per-tag assets consumers download.]

**som-2 (checkable).** **Verify every operational fact against a primary source in THIS repo before acting** —
the actual workflow YAML, the script, the `Cargo.toml`, `plugin.json`, or the launcher constant — never a
remembered or assumed value. The current release version, the CI gates, the pinned toolchain, and the pinned
model SHA all live in files; read them, do not recall them. [VERIFIED — mirrors `CONTRIBUTING.md` "No
speculation: verify against a primary source (the actual code, the real output, the upstream docs)".]

---

## 1. The autonomy & confirmation boundary

This is the load-bearing section for an autonomous operator. The blast radius of an operation — not its
convenience — decides whether the agent may run it alone.

**som-3 (checkable — THE autonomy boundary).** **Never run an irreversible or outward-facing action without
the user's explicit authorization.** The gated set is: **deploy / publish / release**, **`git push`** (and
absolutely **`git push --force`**), creating a **`git tag` that triggers a release** (`v*`), and **deleting
data** (memory rows, stray stores, release assets, history). None of these runs unattended by default; the
agent requests confirmation first. [VERIFIED — `release.yml` fires on `push` of a `v*` tag; `memory-sync`
skill: "Never force-push and never discard either side — stop and ask."]

**som-4 (checkable).** **Reversible, local, read-only actions proceed without a gate.** Building
(`cargo build --release`), testing (`cargo test --release`, `node test/...`), linting (`cargo clippy`,
`cargo fmt --check`), a dry-run, reading logs / state, fetching the model, and editing **uncommitted
working-tree files** (a version bump via `bump-version.mjs`, a changelog edit) are all safe to do
autonomously — they are trivially undoable and touch nothing outside the working tree. [INFERRED from the
som-3 boundary; grounded in `CONTRIBUTING.md` build/test commands.]

**som-5 (checkable).** **Never force-push, and never discard either side of a divergence.** On a conflict you
cannot resolve losslessly (JSONL left with `<<<<<<<` markers, unrelated histories, a push you cannot
reconcile), **stop and consult the user** — do not `--force`, do not reset away one side. [VERIFIED —
`memory-sync` skill + `commands/sync.md` step 5.]

**som-6 (checkable).** **Before any gated action, present the exact command / diff and its blast radius, then
wait for the go-ahead.** State what will change, where it will be visible (a remote branch, a public release,
consumers' machines), and whether it is reversible. Silence is not consent. [INFERRED corollary of som-3.]

**som-7 (judgment).** **Judge each requested operation's reversibility and blast radius to classify it as
gated (§1, som-3) or free (som-4); when in doubt, treat it as gated.** A local file edit is free; anything
that leaves the machine, mutates shared history, or destroys data is gated. [INFERRED.]

---

## 2. Versioning & the release-version contract

**som-8 (checkable — SemVer correctness).** **Version per Semantic Versioning 2.0.0.** Increment **MAJOR** for
incompatible/breaking changes, **MINOR** for backward-compatible new functionality (and whenever you mark
something deprecated), **PATCH** for backward-compatible bug fixes. **Pre-release** identifiers (this repo's
`-beta`) denote a version of **lower precedence** than the associated normal version (`0.2.0-beta` < `0.2.0`).
A released version is **immutable** — never modify it; release a new one. [VERIFIED — semver.org clauses
2,3,6,7,8,9,11; the Cargo SemVer guide categorizes API changes as major / minor / possibly-breaking.]

**som-9 (checkable).** **Keep the release version in lockstep across its TWO sources of truth** —
`.claude-plugin/plugin.json` `"version"` and `bin/genesis-memory.js` `RELEASE_VERSION` — by running
`node scripts/bump-version.mjs <semver>`. Never hand-edit one without the other: the release `gate` job
**refuses** a tag whose `plugin.json` and launcher versions don't both equal the tag (minus `v`). The bump
script's own regex is `^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$`. [VERIFIED — `scripts/bump-version.mjs`;
`release.yml` gate "Refuse a release whose versions don't match the tag".]

**som-10 (checkable).** **Treat the three Rust crate versions as internal and decoupled from the release
version.** `server/`, `cli/`, and `hook/` are each `version = "0.1.0"` with `publish = false`; they are NOT
what a consumer sees and NOT what `bump-version.mjs` touches. The consumer-facing version is the plugin /
launcher version (`0.2.0-beta`). Do not conflate them, and do not try to `cargo publish` the crates.
[VERIFIED — `server/Cargo.toml`, `cli/Cargo.toml`, `hook/Cargo.toml` all `0.1.0` + `publish = false`;
`plugin.json` `0.2.0-beta`.]

**som-11 (checkable).** **A `v<version>` tag may be cut ONLY from a commit that is on `main`** whose
`plugin.json` version and launcher `RELEASE_VERSION` both equal the tag minus `v`. The release `gate` job
enforces both (an ancestry check against `origin/main` + the version match) and **refuses** otherwise —
satisfy the gate before tagging; never work around it. The flow is: develop on a branch → merge to `main` →
tag on `main` → release. [VERIFIED — `release.yml` `gate` job.]

---

## 3. The release procedure

**som-12 (checkable — the exact steps).** **Cut a release in this fixed order:** (1) **green CI** on the
branch; (2) **`node scripts/bump-version.mjs <ver>`** (build/bump); (3) **update `CHANGELOG`** (§6); (4)
**merge to `main`**; (5) **tag `v<ver>` on `main`**; (6) **push the tag** → `release.yml` runs the gate, builds
the 8 platform targets + fetches the model, writes `SHA256SUMS`, and publishes the GitHub Release. Steps 4–6
(merge / tag / push) are the outward-facing boundary and require the user's go-ahead (som-3). [VERIFIED —
`release.yml`; `DISTRIBUTION.md` "develop on a branch → merge to `main` → tag on `main` → release".]

**som-13 (checkable).** **Let CI perform the actual multi-platform build and publish — do not hand-build or
hand-upload release assets.** `release.yml` builds all 8 targets (darwin arm64/x64, linux-x64 gnu/musl,
win32-x64, linux-arm64 gnu/musl, win32-arm64) from the one pure-Rust **tract** path, produces **self-contained**
binaries (bundled SQLite, static Windows CRT, inference compiled in), and **verifies no dynamic
inference-runtime sidecar** is linked. A local hand-built asset would bypass those guarantees. [VERIFIED —
`release.yml` build matrix + "Verify server is self-contained" step; `NOTICE.md` §2.]

**som-14 (checkable).** **A pre-release tag (contains `-`, e.g. `v0.2.0-beta`) MUST publish as a GitHub
prerelease; only a final `X.Y.Z` tag publishes as a full release.** The release job selects `--prerelease`
exactly when the tag contains `-`. Never mark a `-beta`/`-alpha`/`-rc` build as a full release. [VERIFIED —
`release.yml` `case "$TAG" in *-*) PRE="--prerelease"`.]

---

## 4. CI & build health

**som-15 (checkable — keep CI green).** **Never merge to `main` or cut a release while `ci.yml` is red.** A
red `main` blocks every release, because the release `gate` builds from `main`. If CI breaks, **fix it or
revert the offending change first** — a green pipeline is the precondition for every outward step, not an
afterthought. [VERIFIED — `ci.yml` runs on every push/PR; `release.yml` gate requires a `main` ancestor.]

**som-16 (checkable).** **Preserve every CI gate; never weaken, skip, or `continue-on-error` a gate to go
green.** The gates are: the Rust server + `genesis-hook` + `genesis-cli` **build and test on all three primary
OSes** (ubuntu-22.04, macos-14, windows-latest); **`clippy --all-targets -- -D warnings` + `cargo fmt
--check`** on the hook and cli crates; the Node **parse-check of every JS/MJS** + the plugin/launcher **unit
tests**; the **"no runtime Python"** assertion; and the **self-contained-binary regression guard**. Green means
substance, not a silenced check. [VERIFIED — `ci.yml` rust + node jobs.]

**som-17 (checkable).** **Treat the pinned toolchain and pinned load-bearing dependencies as deliberate pins
with documented reasons; never bump them casually.** `rust-toolchain.toml` pins Rust **1.93.0** (the newest
`libsqlite3-sys` without the `cfg_select` macro); `server/Cargo.toml` pins `rusqlite = "=0.39.0"`,
`ort = "=2.0.0-rc.13"`, and `ort-tract = "=0.4.0"` for interlocking ABI reasons. A bump is a change to
verify and test across **all** release targets, not a convenience edit. [VERIFIED — `rust-toolchain.toml`
comment; `server/Cargo.toml` dependency comments.]

**som-18 (checkable).** **Keep the embedding model pinned by revision + SHA-256, and never repoint it
silently.** `scripts/fetch-model.mjs` pins `DEFAULT_REVISION` and `EXPECTED_MODEL_SHA256` for
`sentence-transformers/all-MiniLM-L6-v2`; the fetcher **fails closed** on a SHA mismatch and refuses to verify
a non-default revision. Changing the revision requires `--print-only` capture **and** regenerating the golden
vectors + the server's `embed::MODEL_*` constants. [VERIFIED — `scripts/fetch-model.mjs`; `NOTICE.md` §1.]

---

## 5. Distribution mechanics & integrity

**som-19 (checkable).** **Distribute ONLY via GitHub Release assets downloaded by the Node launcher — no npm,
no registry account, no publish token.** The release workflow uses only the auto-provided `GITHUB_TOKEN`; the
alpha's npm platform-packages, the `@xcidos` scope, and `generate-platform-packages.mjs` were **deleted** in
beta. Do not reintroduce npm publishing. [VERIFIED — `release.yml` `permissions: contents: write` + only
`GITHUB_TOKEN`; `DISTRIBUTION.md` beta update.]

**som-20 (checkable — integrity).** **Every downloaded asset MUST be SHA-256-verified against the published
`SHA256SUMS` before it is used; never serve or stage an unverified or mismatched asset.** The launcher fetches
`SHA256SUMS`, computes the hash of each binary/model file, and **refuses** an asset with no listed checksum or
a mismatch. The manifest is the integrity contract between the release and every consumer. [VERIFIED —
`bin/genesis-memory.js` `ensureAsset` / `checksums`; `release.yml` "Assemble release dir + SHA256SUMS".]

**som-21 (checkable).** **Preserve `/plugin update` propagation: bump the plugin version on every
consumer-facing change.** The `plugin.json` version gates whether consumers receive an update at all (per the
Claude Code plugin docs, an unbumped version means no update ships), and the launcher's **`--sync` +
`.staged-version` stamp** refreshes already-bootstrapped repos' staged binaries to the plugin version. A change
shipped without a version bump is invisible to `/plugin update` (the exact bug the lockstep gate was built to
prevent). [VERIFIED — `bin/genesis-memory.js` `syncRepo`; `DISTRIBUTION.md`; Claude Code plugin-marketplace
docs: "Set [version], and users only receive updates when you bump the version."]

---

## 6. Changelog discipline

**som-22 (checkable — changelog).** **Maintain the changelog in Keep a Changelog format.** One **dated entry
per released version**, **newest first**; changes grouped under the standard types **Added / Changed /
Deprecated / Removed / Fixed / Security**; an **`Unreleased`** section kept at the top for accumulating
upcoming changes; and a stated note that the project **follows Semantic Versioning**. There is an entry for
every version. [VERIFIED — keepachangelog.com 1.1.0: guiding principles, types of changes, Unreleased section.]

**som-23 (judgment).** **Write changelog entries for humans — user-visible impact, not a dump of git log
lines — and mark a pulled release `[YANKED]`.** A changelog is for people deciding whether to upgrade; a
version pulled for a serious bug or security issue is shown, loudly, with the `[YANKED]` tag. [VERIFIED —
keepachangelog.com: "Changelogs are for humans, not machines"; the `[YANKED]` convention.]

---

## 7. Memory-store health & sync

The Genesis memory store is two artifacts that must stay consistent, both committed, and both travelling with
the repo.

**som-24 (checkable — memory-store consistency).** **Keep the store's two artifacts consistent and BOTH
committed, and never commit the sidecars.** `.genesis/memory.db` is the ready-to-use vector store (rows +
384-dim embeddings) and the **source of truth**; `.genesis/memory/memory.jsonl` is the diff-friendly,
deterministically-ordered mirror and the **merge safety-net**. The server auto-exports `.db → .jsonl` after
every `store`/`consolidate` and UNION-merges the JSONL back on startup, so the `.db` **self-heals** from the
JSONL. Commit both together; the WAL/shm sidecars (`memory.db-wal`, `memory.db-shm`) are **never** committed.
[VERIFIED — `memory-sync` skill; `.gitignore` (`!.genesis/memory.db`, `!.genesis/memory/`, `*.db` otherwise
ignored); `.mcp.json` `GENESIS_MEMORY_DB` / `GENESIS_MEMORY_EXPORT`.]

**som-25 (checkable).** **Sync memory ONLY through the deterministic, lossless procedure (`/genesis:sync` /
`genesis-cli reconcile`).** Commit both artifacts → fetch the remote JSONL → **UNION-reconcile keyed by
`(agent_id, text)`** (dedupe by content, never overwrite, never drop, stable-sorted + re-ided so diffs are
byte-identical) → commit → push. Consult the user **only** on a genuine git conflict; **never force-push,
never discard a side.** [VERIFIED — `memory-sync` skill "The deterministic merge"; `commands/sync.md`.]

**som-26 (checkable).** **Recover scattered memory via `/genesis:doctor` (read-only) then `/genesis:fix`, at a
user-chosen scope, reading strays READ-ONLY.** Doctor diagnoses the canonical store and flags
`recoverable_strays` (databases outside `.genesis/` holding **this repo's** agents) without changing anything;
fix consolidates only **this repo's** agents' memories into the canonical `.db` (verbatim, embeddings and all —
recall-able immediately). Require an explicit `--scope user|system` or `--root` (never guess a scan area), and
**never pull a foreign repo's memory**. [VERIFIED — `commands/doctor.md`, `commands/fix.md`, `memory-sync`
skill.]

**som-27 (checkable).** **Do not file-copy the live memory DB for a backup while the server holds a WAL open.**
A raw `cp` of `memory.db` mid-write can capture an inconsistent snapshot (the base file without its WAL
frames). For a consistent backup, **checkpoint first**, or back up via the **JSONL export** (already the
mergeable, lossless mirror) or SQLite's **Online Backup API** — never a plain file copy of an actively-written
WAL database. [VERIFIED — sqlite.org WAL-backup guidance: checkpoint before copying / use the Backup API /
avoid file-level copying of a live DB; the JSONL mirror is the repo's designated portable form.]

**som-28 (checkable).** **Run `/genesis:memory validate` to check DB↔JSONL agreement and the embedding
contract, and resolve genuine contradictions WITH the user.** Validate compares `.db` vs `.jsonl`, runs the
embedding-contract / drift canary, and flags contradictions to an **HTML report** (user given the full path).
Resolve conflicts together — supersede one / keep-both-scoped / edit — and **never auto-resolve silently** or
serve a **mixed-embedding-generation** index (re-embed the whole store on a contract change instead).
[VERIFIED — `commands/doctor.md`/`memory.md` suite; memory-management expertise mm-47/mm-48/mm-62.]

---

## 8. Safe deploys & rollback

**som-29 (checkable).** **Roll back by rolling forward.** Because a published version is immutable (som-1,
som-8), a bad release is corrected by **shipping a fixed new version** (or re-pointing the plugin version),
**never** by editing a released tag or re-uploading over a published asset under the same version. A
consumer's cache is keyed per-version (`<cache>/v<version>/`), so a same-version overwrite would not even
reach already-cached installs. [VERIFIED — semver.org clause 3; `bin/genesis-memory.js` `versionDir()`.]

**som-30 (checkable).** **Verify a clean install on a real platform before declaring a release done, and let a
straggler platform degrade gracefully.** The release job publishes whatever built (`if: always() &&
needs.model.result == 'success'`) so one failing platform never sinks the rest, and the launcher treats a
missing platform asset as a clear **"build from source"** path, not a crash. Confirm the launcher downloads,
verifies, and execs on a target before calling the release good. [VERIFIED — `release.yml` `release` job
condition; `bin/genesis-memory.js` `fetchBuffer` error message; `TODO.md` deferred targets.]

---

## 9. Issue & PR triage

**som-31 (checkable).** **Reproduce and diagnose against a primary source before proposing a fix; never
speculate a cause.** Get the OS, the Node/`rustc` versions, the exact command, expected vs actual, and the full
output; a minimal reproduction outweighs a description. Confirm the mechanism in the real code or real output
before changing behavior. [VERIFIED — `CONTRIBUTING.md` "Opening issues" + rule 2 "No speculation".]

**som-32 (checkable).** **Hold every change to the four hard rules, and require a regression test.** No
shortcuts; no speculation; use the docs / expertise; production-ready + tested. A **bug fix ships a regression
test that FAILS before the fix and PASSES after**; new behavior ships with tests; the relevant CI gate passes
before merge. [VERIFIED — `CONTRIBUTING.md` "The four hard rules" + "Opening pull requests".]

**som-33 (checkable).** **Route a security report or a committed secret PRIVATELY to the maintainer — never on
a public issue or PR.** A vulnerability or leaked credential is reported out of band; do not open, comment, or
attach it in the public tracker. [VERIFIED — `CONTRIBUTING.md` "Security / credentials".]

---

## 10. Lifecycle, deprecation & deferred work

**som-34 (checkable).** **Track deferred and unsupported work in `TODO.md` with its reason and fix-path, and
never claim an unbuilt platform is supported.** The deferred release targets (`linux-arm64-musl`,
`win32-arm64`) are hard-blocked upstream and documented with their fix-paths; consumers on them get a clean
"unsupported platform" message. Record deferrals; don't silently drop them or overstate coverage. [VERIFIED —
`TODO.md`.]

**som-35 (checkable).** **Deprecate before you remove.** Announce a deprecation with a **MINOR** bump + a
CHANGELOG **`Deprecated`** entry, keep a migration path, then remove the capability in a later (breaking/major)
release. Never remove a consumer-facing capability without a deprecation trail. [VERIFIED — semver.org clause 7
(marking deprecated ⇒ minor bump); keepachangelog.com `Deprecated` type.]

**som-36 (judgment).** **Do not silently reverse a documented design decision.** The npm → GitHub-Releases
distribution, `.db`-as-source-of-truth (with `.jsonl` as mirror), and pure-Rust **tract** over ONNX Runtime are
recorded decisions; a superseded decision stays **documented**, and a proposed reversal is **surfaced to the
user**, not applied unilaterally. [VERIFIED — `DISTRIBUTION.md` decision log; memory-management mm-52 tension.]

---

## 11. Safety invariants (house rules)

**som-37 (checkable).** **Never write a credential / secret / key value into any file, log, commit, changelog,
or memory record** — record `credential present at <path>` instead — and never route around the `genesis-hook`
`gate`, which already blocks credential shapes at write time. Write-time minimization beats after-the-fact
scrubbing. [VERIFIED — the `gate` hook (`README.md` "a banned phrase, a credential value"); memory-management
mm-55.]

**som-38 (checkable).** **Use "structured reasoning" in all authored text; never emit the banned
reasoning-trace phrase the `gate`'s regex rejects.** Govern outputs and actions, not the private reasoning
trace. [VERIFIED — the `gate` hook's banned-phrase regex (`README.md`); expertise-application ea-6.]

**som-39 (principle).** **Honest limit: a green pipeline proves FORM, not field correctness.** Green CI and a
successful publish prove it built, published, and checksummed — not that the release is correct or safe on a
consumer's machine. Prove a release is good with a real **clean-install verification** and the **user's
sign-off**; never overstate "shipped" as "verified in the field". [INFERRED; consistent with
expertise-application ea-11 (form ≠ correctness).]

---

## Operational defaults & quick reference

- **Release version (consumer-facing):** `0.2.0-beta` — in `.claude-plugin/plugin.json` **and**
  `bin/genesis-memory.js` `RELEASE_VERSION`; bump both via `node scripts/bump-version.mjs <ver>`. [VERIFIED]
- **Crate versions (internal, `publish=false`):** `server` / `cli` / `hook` = `0.1.0`. [VERIFIED]
- **Toolchain:** Rust `1.93.0` (`rust-toolchain.toml`). **Pinned deps:** `rusqlite =0.39.0`,
  `ort =2.0.0-rc.13`, `ort-tract =0.4.0`. [VERIFIED]
- **Model:** `all-MiniLM-L6-v2` @ revision `c9745ed1…`, `model.onnx` SHA `6fd5d72f…`
  (`scripts/fetch-model.mjs`); Apache-2.0 (`NOTICE.md`). [VERIFIED]
- **Release targets (8):** darwin-arm64, darwin-x64, linux-x64-gnu, linux-x64-musl, win32-x64,
  linux-arm64-gnu, linux-arm64-musl, win32-arm64. **Deferred/blocked:** linux-arm64-musl, win32-arm64
  (`TODO.md`). [VERIFIED]
- **Release assets:** `genesis-memory-server-<key>`, `genesis-hook-<key>`, `genesis-cli-<key>`, `model.onnx`,
  `tokenizer.json`, `SHA256SUMS`. **Distribution:** GitHub Releases only, `GITHUB_TOKEN` only. [VERIFIED]
- **Memory store:** `.genesis/memory.db` (truth) + `.genesis/memory/memory.jsonl` (mirror) — both committed;
  WAL/shm never committed; sync via `/genesis:sync`; recover via `/genesis:doctor` → `/genesis:fix`. [VERIFIED]
- **Gate before:** deploy · publish/release · `git push` (never `--force`) · release tag `v*` · delete data.
  **Free:** build · test · lint · dry-run · read · uncommitted working-tree edits. [VERIFIED/INFERRED]

## Source ledger

**Repo primary sources (read in full):** `.github/workflows/ci.yml`, `.github/workflows/release.yml`,
`scripts/bump-version.mjs`, `scripts/fetch-model.mjs`, `bin/genesis-memory.js`, `.claude-plugin/plugin.json`,
`rust-toolchain.toml`, `server/Cargo.toml`, `cli/Cargo.toml`, `hook/Cargo.toml`, `docs/DISTRIBUTION.md`,
`docs/architecture.md`, `CONTRIBUTING.md`, `NOTICE.md`, `README.md`, `TODO.md`, `.gitignore`, `.mcp.json`, and
the plugin's `memory-sync` skill + `genesis:sync|fix|doctor|memory` commands. **External primary specs:**
Semantic Versioning 2.0.0 (semver.org), Keep a Changelog 1.1.0 (keepachangelog.com), the Cargo SemVer guide
(doc.rust-lang.org/cargo/reference/semver.html), GitHub Actions docs (docs.github.com/actions), the Claude Code
plugin / marketplace / MCP docs (code.claude.com/docs), and the SQLite WAL-backup guidance (sqlite.org).
**Sibling expertise cross-referenced:** `memory-management.md` (store health, supersession, embedding contract)
and `expertise-application.md` (form ≠ correctness; govern outputs not the reasoning trace).

*Colophon: v1, 2026-08-15. 39 rules (som-1…som-39). Grounded in this repo's real tooling with zero pages
turned; VERIFIED vs INFERRED separated; the autonomy/confirmation boundary encoded as som-3 (wiring).*
