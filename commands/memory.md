---
description: Run a Genesis memory operation via Mneme — validate, serialize, deserialize, merge, or migrate this repo's memory store. Mneme owns memory; conflicts are surfaced to you in an HTML report you resolve together.
argument-hint: <validate|serialize|deserialize|merge|migrate> [path-to-other-store]
---

You are running **`/genesis:memory`**. The memory specialist agent **Mneme** owns this repo's memory — the
canonical `.genesis/memory.db` (source of truth) and its line-diffable `.genesis/memory/memory.jsonl` mirror.
Your job here is only to route the request to Mneme and relay the conversation; Mneme does the work under its
own expertise. Do NOT run the memory CLIs yourself.

**Parse `$ARGUMENTS`:** the first word is the subcommand (`validate` | `serialize` | `deserialize` | `merge` |
`migrate`); anything after it is an optional path (used by `merge`). If the subcommand is missing or not one of
those five, ask the user which of them they want — do not guess.

**Do this now:** invoke the **`genesis:mneme`** agent (via the Agent tool), telling it the subcommand, the repo
(`${CLAUDE_PROJECT_DIR}`), and any path from `$ARGUMENTS`. Mneme carries out exactly the requested operation:

- **validate** — Mneme runs `node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli validate --repo
  "${CLAUDE_PROJECT_DIR}"` and reports whether the `.db` and `.jsonl` agree and whether any **semantic
  contradiction** exists (the same subject+relation asserting different objects). Read-only — nothing changes.

- **deserialize** — Mneme mirrors the `.db` out to its `.jsonl` via `node
  "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" export`. The `.db` stays the source of truth; the `.jsonl` is
  refreshed from it. Nothing is destroyed.

- **serialize** — rebuild the `.db` FROM the `.jsonl`. Mneme first **warns you** the `.db` is the base truth and
  asks you to confirm, then **timestamp-renames the current `.db`** (a recoverable backup — never overwrites in
  place), then runs `node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" import` to rebuild it (re-embedding
  each memory). If the confirmation is declined, Mneme stops and changes nothing.

- **merge** — fold ANOTHER store into this one. If no path was given, Mneme first checks whether the repo
  already references one; if not, it asks you for the **full path** to the other store's `.jsonl`. Mneme runs
  `--run-cli merge --repo "${CLAUDE_PROJECT_DIR}" --incoming "<path>"`. It unions losslessly. If it finds
  **semantic contradictions**, it does NOT finalize: it writes an **HTML report** and gives you its **full
  path**. You open it, decide which value is correct for each contradiction, and tell Mneme; Mneme discusses
  until every conflict is resolved (`--run-cli resolve --repo "${CLAUDE_PROJECT_DIR}" --staged "<staged>"
  --retire <id>` for each superseded value — kept as history, never deleted), THEN finalizes the merge into
  the canonical store. If there are no contradictions, Mneme merges immediately.

- **migrate** — backfill structure onto pre-0.2.0 **flat** memories (run once after upgrading to the
  structured store). Mneme lists the memories awaiting structure via `node
  "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" unstructured --db "${CLAUDE_PROJECT_DIR}/.genesis/memory.db"`,
  confirms with you before starting, then for each one classifies its type and extracts its
  `(subject, relation, object)` (leaving the triple empty when it states no clear fact), writing each back
  with the `structure` subcommand — exactly as it does on write, but over the existing store. Nothing is
  deleted; any contradiction surfaced during the pass is resolved with you, as in `merge`.

Mneme escalates every genuine decision to you and never resolves a contradiction on its own. After any
operation that changed the store, remind the user they may want to `/genesis:sync` to commit and push the
updated `.db` + `.jsonl`.
