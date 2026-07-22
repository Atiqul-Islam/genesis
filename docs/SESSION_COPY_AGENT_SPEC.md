# Genesis feature spec — Session-Copy Custom Agent

**Status: DESIGN, awaiting Atiqul's sign-off. Do NOT build until approved.**
Author-of-record: Atiqul (decisions); drafted 2026-07-22. Every decision below traces to a confirmed answer this
session — nothing is inferred. Verified technical facts are cited from the live Claude Code docs; open items are
flagged HONESTLY as "verify at build", never assumed.

---

## 1. Goal (one line)

When Genesis builds a **single** custom agent, offer to build it as a **copy of the user's current Claude Code
session** — carrying that session's full conversation history + all memory/context — into a **separate, portable,
specialized agent**.

## 2. What it is / isn't

- It **is** an *option* on Genesis's existing single-agent build: "fresh custom agent" (today) **or** "copy my
  current session into it" (new).
- The result **is** a **separate named agent** you invoke explicitly. It is **not** auto-mounted as the session
  agent (the swappable "session-agent slot" is a *different, out-of-scope* feature — see §9).
- It **is** fully **portable** — a `git clone` on another machine reproduces the agent with its history/memory,
  because everything is captured into repo-local, copyable storage (the user's key insight — §6).
- It does **not** rely on Claude Code's native session fork (whose transcript lives in `~/.claude`, machine-local
  — the exact non-portability we are avoiding).

## 3. Confirmed decisions (traceable)

| # | Decision | Source |
|---|---|---|
| D1 | Genesis builds single agents OR teams; this feature applies to the **single-agent** path only. | confirmed |
| D2 | "Copy current session into it" is **an option Sensei offers** per single-agent build (the fresh path stays). | confirmed |
| D3 | Capture the **full conversation history** (transcript + compaction summary). | confirmed |
| D4 | Store captured history/memory in the new agent's **portable SQLite** (repo-local, travels with the clone). | confirmed |
| D5 | The agent **uses** the copied history via **BOTH**: a running summary injected at session start **+** semantic recall of details on demand. | confirmed |
| D6 | "Everything" copied = **A** live conversation + **B** persistent memory stores + **C** user-level/global config snapshot (see §5). | confirmed |
| D7 | **User-level/global config is snapshotted too** (for a self-contained clone), with **credentials scrubbed**. | confirmed |
| D8 | The result is a **separate named agent** (not mounted as the session agent). | confirmed |
| D9 | **Credentials are never copied** anywhere (hard workspace rule). | standing rule |

## 4. Verified grounding (live Claude Code docs, fetched 2026-07-22)

- *"The context window holds everything Claude knows about your session: your instructions, the files it reads,
  its own responses, and content that never appears in your terminal."* (`/docs/en/context-window`)
- Loads before you type: **CLAUDE.md, auto memory, MCP tool names, skill descriptions**, optionally output style
  / `--append-system-prompt`. As Claude works: **each file read, path-scoped `.claude/rules/`, hooks**. At the end:
  **`/compact` replaces the conversation with a structured summary.** (`/docs/en/context-window`)
- Auto memory lives at `~/.claude/projects/<project>/memory/` (`MEMORY.md` + topic files); first 200 lines / 25 KB
  of `MEMORY.md` load each session. (`/docs/en/memory`)
- Session transcript lives at `~/.claude/projects/<project-path>/<session-uuid>.jsonl`. (bg-agents skill;
  hooks expose `session_id` + `transcript_path`.)
- A native **fork** "inherits the entire conversation so far" but is a `~/.claude` background session
  (machine-local) — **why we do NOT use it** for portability. (`/docs/en/sub-agents`)
- A normal subagent definition "creates a new instance with **fresh context**" and does not inherit the
  conversation — so history must be captured explicitly. (`/docs/en/sub-agents`)

## 5. Capture scope — the complete enumeration ("everything")

**A. Live conversation (per-session, machine-local → capture):**
1. Session transcript `<session-uuid>.jsonl` (messages, tool calls, reasoning).
2. The current compaction summary.

**B. Persistent memory (machine-local → capture):**
3. Native auto-memory: `~/.claude/projects/<project>/memory/MEMORY.md` + topic files.
4. Genesis MCP memory: the per-agent SQLite vector store (existing rows for the current agent).
5. This setup's plugin memory: **context-mode** knowledge base + **claude-mem** observations DB. *(verify DB
   locations + schemas at build — §8.)*

**C. Instructions/config (files):**
6. Repo-level (`CLAUDE.md`, `.claude/rules|skills|agents|commands`, `.mcp.json`, project settings) — **already
   travels via git**; not copied wholesale. The custom agent gets its **own specialized persona** (the "custom").
7. **User-level/global** (`~/.claude/CLAUDE.md`, user skills, user settings) — **snapshotted** into the agent
   (D7), **credentials/secrets scrubbed** (D9).

## 6. How it works (architecture)

```
CAPTURE (in the current session, which knows its own session_id + transcript_path)
  → serialize A+B+C(user-level) into the new agent's portable store under the repo:
      <repo>/.genesis/agents/<name>/
        history.sqlite      # transcript chunks + memory rows (semantic-recall-ready), credential-scrubbed
        summary.md          # running summary of the session (injected at start)
        snapshot/           # user-level CLAUDE.md/skills/settings snapshot (scrubbed)
  → embed transcript/memory chunks via the Genesis memory server (offline ONNX) for recall.

BUILD (Genesis normal single-agent flow, seeded)
  → Sensei interviews for the SPECIALIZATION (what this copied agent should now be good at).
  → Method authors persona/behavior/skills (test-first) — the "custom" layer on top of the copied knowledge.
  → assemble.py wires the agent + points its memory tools at history.sqlite; required-expertise enforced as usual.

USE (separate named agent)
  → on start: inject summary.md (running summary) as context.
  → on demand: the agent recalls specific history/memory slices via its memory tools (semantic recall).
  → clone on another machine: .genesis/agents/<name>/ travels with the repo → re-bootstrap → agent works. (§7)
```

## 7. Portability design

- All captured state lives under `<repo>/.genesis/agents/<name>/` (committed/copyable), **not** in `~/.claude`.
- A clone on machine B: `bootstrap.py` (already exists) makes the repo a self-contained Genesis workspace; the
  agent's `history.sqlite` + `summary.md` + `snapshot/` are already present → the agent recalls its history and
  loads its summary with **no `~/.claude` dependency**. This is exactly the user's SQLite-portability insight.
- **Credential scrubbing** runs on every captured store before it is written (transcript, memory, settings,
  env) — reusing the workspace's existing secret-scan patterns; a value that matches is replaced with
  `credential present at <path>`, never the value.

## 8. Honest open items — status

1. **Capture trigger/mechanism.** [OPEN → Phase 3] The capturing step must run where the *current* session's
   `session_id` / `transcript_path` are available. Verified: hooks receive both; the transcript is also findable
   by globbing `~/.claude/projects/*/<session_id>.jsonl` (encoding-agnostic, implemented). The user-facing
   trigger (a command/skill run in-session) is wired in Phase 3.
2. **Plugin memory formats.** [RESOLVED, Phase 1] Verified real schemas 2026-07-22: context-mode content =
   `chunks(title,content,session_id,timestamp)` (extract by session_id — implemented); claude-mem = observer
   `*.jsonl` with `content`+`sessionId`. **Finding:** claude-mem's observer `sessionId` is the *observer's*, not
   the main session's, and has no project field → a "keep all" fallback dumped the entire ~13k-record / ~85MB
   cross-project corpus. **Fixed:** the file extractor now keeps ONLY exact-session matches (no corpus dump);
   COMPLETE project-scoped claude-mem capture uses claude-mem's MCP search API from the LIVE in-session step
   (Phase 3).
3. **Summary "running/updated".** [OPEN → Phase 3] Generate at capture; refresh via a Stop/PreCompact hook.
4. **Recall fidelity vs context limits.** [Phase 2] chunking/embedding of `records.jsonl` into the memory server.
5. **Big binaries.** [MEASURED] A real 468-line session captured to 372 KB; the earlier 85 MB was the claude-mem
   bug, now fixed. Size is reasonable to commit.
6. **Credential scrubbing scope.** [RESOLVED, Phase 1 — honest] Pattern-scrub catches known SHAPES + LABELLED
   secrets + a caller-supplied exact-value denylist (guaranteed). It CANNOT auto-detect an arbitrary unlabelled
   unknown string (would false-redact hashes/UUIDs pervasive in transcripts). Defense-in-depth, not a proof;
   the live step passes any known secret values to the denylist. Tests make this limit explicit.

### Phase 1 — DONE (2026-07-22)
`session_copy/capture.py` + `test_capture.py` (24/24 green). Extracts all stores → one scrubbed, normalized
`records.jsonl` + manifest. Verified on synthetic data (real schemas) AND a real 468-line session (152 records,
372 KB, 0 raw-secret-shape leaks).

### Phase 2 — DONE (2026-07-22)
- `store.py` + `test_store.py` (12/12): `records.jsonl` → portable `history.sqlite` + deterministic `summary.md`
  (carried-over sources, the prior MEMORY.md, recent turns; injected at start per D5) + manifest.
- `embed.py` + `test_embed.py` (4/4, REAL server, no mocks): records embed under the agent's `agent_id` and are
  semantically recallable, per-agent isolated.
- `test_pipeline.py` (7/7, REAL server): capture→store→embed end-to-end, then **★ recall from a COPIED bundle on
  a simulated "machine B"** — proving full-history + clone-portability together, no `~/.claude` dependency.
- **Design note (simpler wiring):** embeddings go into the repo's shared genesis-memory DB under `agent_id=<name>`
  (Genesis already isolates by agent_id) — so a session-copy agent is a normal Genesis agent whose id is
  pre-loaded with its history; no separate memory server. The DB lives in `.genesis/` and travels.
- **session_copy tests total: 47 green** (24 + 12 + 4 + 7).

### Phase 3 — DONE (2026-07-22)
- `hooks/inject.py` extended + `hooks/test_inject.py` (7/7): surfaces a session-copy agent's `summary.md` at
  SessionStart (found at `<genesis_home>/agents/<name>/` or `<cwd>/.genesis/agents/<name>/`), within the 10k cap.
- `hooks/session_pointer.py` + `test_session_pointer.py` (7/7): records the live session id to
  `<repo>/.genesis/current-session.json` so `--session current` works (the capture trigger).
- `session_copy/build_session_agent.py` + `test_build_session_agent.py` (4/4, REAL server): the orchestrator —
  one call captures→stores→embeds into the repo's shared memory under `agent_id=<name>`, lays the bundle where
  inject.py finds it, and proves the agent recalls its history + gets its summary at start.
- `team/sensei/skills/build-agent/SKILL.md` Step 2b: Sensei offers "fresh vs copy my current session" (D2);
  copy → run the orchestrator, then the normal specialize+assemble flow.

**Manual live gate (the one thing not self-verifiable here):** building a session-copy agent from a REAL live
Claude Code session and talking to it — needs a live session (the pointer hook wired, a real transcript). All
components are built, wired, and unit/integration-tested against the real memory server; the live end-to-end is
the single remaining verification (analogous to commune's one manual run).

## 11. Status — COMPLETE (build), pending the live gate
Full genesis suite: **14 suites, 147 assertions, 0 failures** (session_copy adds 51: capture 24, store 12,
embed 4, pipeline 7, orchestrator 4; plus inject 7, session_pointer 7). Phases 1–3 built + tested. Feature is
usable: Sensei offers it, the orchestrator builds a portable session-copy agent, it recalls its history and
loads its summary. Only the live end-to-end gate remains.

## 9. Out of scope (explicitly, to avoid scope creep)

- The **swappable "session-agent slot" / mount** feature (mount a Genesis agent as what bare `claude` gives you).
  Discussed earlier; **separate feature**, not built here (D8 = separate named agent).
- Team (multi-agent) session-copy — this spec is single-agent only (D1).
- Using Claude Code's native fork (rejected for non-portability, §2).

## 10. Build plan (phased, after sign-off)

1. **P1 — Capture library:** locate + serialize A+B+C(user) with credential scrubbing; unit-tested against a
   real session's stores in a temp dir.
2. **P2 — Store + embed:** write `history.sqlite` + `summary.md` + `snapshot/`; embed chunks via the memory
   server for recall; portability test (clone-copy → recall works).
3. **P3 — Sensei flow:** add the D2 option to the build-agent skill; seed Method's build with the captured store;
   wire the agent's memory tools + start-time summary injection.
4. **P4 — End-to-end + portability test:** build a session-copy agent, verify it recalls prior history + loads
   the summary, then verify the same after a simulated clone-copy on a fresh workspace.
- No shortcuts: each phase test-first, real stores (no mocks), credentials scrubbed, cross-platform.
