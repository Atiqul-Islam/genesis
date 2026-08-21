## Feature: mechanical guard — a no-grep agent cannot grep a file (grep-to-skip-a-read)

`genesis-engineer` has **no Grep tool by design** and must read files in full; grepping a file to skip
reading it is a hard-rule violation that discipline alone failed to prevent (it happened, user-flagged,
2026-08-21). This adds a deterministic `PreToolUse(Bash)` guard in the `enforce-research` hook: for an
agent on the **no-grep list**, a Bash command that runs a grep-family tool **against a file** is denied
with a message to Read the file instead. Agents that legitimately hold the Grep tool (sensei/method/mneme)
are untouched, and piped-output greps (reading stdin) are allowed for everyone.

### Expected Behavior

- For a no-grep agent (`genesis-engineer`), a Bash command that greps a FILE (`grep PATTERN file`,
  `grep -r …`, `rg …`) is BLOCKED, with a reason telling it to Read the file.
- A piped-output grep (`… | grep PATTERN`) is ALLOWED for every agent — it reads stdin, not a file.
- An agent NOT on the no-grep list (sensei/method/mneme — they hold the Grep tool) is never blocked by
  this guard.
- The guard is dormant for non-genesis sessions and fail-open, like the other hooks.
- The guard fires for a promoted MAIN agent — `main_thread_hooks` wires a `Bash → enforce-research
  --main-agent <name>` entry (a promoted main currently has no Bash hook at all).

### Acceptance Criteria

- **AC1** — active `genesis-engineer` + `grep foo src/x.rs` → **deny** (reason mentions reading the file).
- **AC2** — active `genesis-engineer` + `rg foo` → **deny** (rg searches files by default).
- **AC3** — active `genesis-engineer` + `cargo test | grep result` → **allow** (piped stdin).
- **AC4** — active `genesis-engineer` + `grep foo` (no file, reads stdin) → **allow**.
- **AC5** — active `method` + `grep foo src/x.rs` → **allow** (method holds the Grep tool; not no-grep).
- **AC6** — no genesis agent active → **allow** (dormant).
- **AC7** — `main_thread_hooks(name, …)` contains a `PreToolUse` `Bash` hook whose command is
  `enforce-research --main-agent <name>`.

### Implementation Requirements

- `enforce_research.rs`: `NO_GREP_AGENTS = ["genesis-engineer"]` *(ratified choice — the agent(s) whose
  discipline forbids grep because they carry no Grep tool; extend the const if more are added)*. A
  grep-family command (`grep|egrep|fgrep|rg|ag`) at a **command position** (index 0, or immediately after a
  shell operator `| || && ; (` — but NOT immediately after a single `|`, which is piped stdin) that **reads
  a file** → deny. "Reads a file": `rg`/`ag` always (they search files by default); `grep` family when a
  recursive flag (`-r`/`-R`/`--recursive`/a combined `-…r…`) is present OR there are ≥2 non-flag arguments
  (pattern + file). Reuse the existing `shlex_split`; the detector is purely structural (no filesystem
  lookup), so it is deterministic.
- `render.rs::main_thread_hooks`: add a `PreToolUse` entry `{"matcher":"Bash", … "enforce-research
  --main-agent <name>"}` alongside the existing `Write|Edit → gate`, so promoted mains run the Bash guard.
- Fail-open, dormant-by-default, deterministic. The guard denies BEFORE the existing assembler-research
  check and does not alter it.
