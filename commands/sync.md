---
description: Update the remote repo with this repo's Genesis memory — commit BOTH the vector DB and the JSONL, reconcile with the remote losslessly, and push. Consults you only on a genuine git conflict.
argument-hint: (no arguments)
---

You are running **`/genesis:sync`** — the deterministic "update the remote repo" procedure for Genesis
memory. Memory is committed as BOTH `.genesis/memory.db` (the ready-to-use vector DB) and
`.genesis/memory/memory.jsonl` (the diff-friendly merge substrate). On sync, git takes the latest `.db` and
`genesis-cli reconcile` unions the JSONL so **no memory is ever lost**. See the **memory-sync** skill for the
model. Run every step; do not skip. Never invent memory; only reconcile what exists.

Let `P="${CLAUDE_PROJECT_DIR}"`. **Do this now:**

1. **Preconditions.** Confirm `P` is a git repo with an upstream:
   `git -C "$P" rev-parse --abbrev-ref --symbolic-full-name @{u}` → this is `<upstream>` (e.g. `origin/main`).
   If there is no upstream/remote, tell the user and stop. Confirm `P/.genesis/memory/` exists (a Genesis
   workspace); if not, stop and tell them to bootstrap first.

2. **Snapshot + commit local memory.** Stage and commit both artifacts:
   ```
   git -C "$P" add .genesis/memory.db .genesis/memory/memory.jsonl
   git -C "$P" commit -m "genesis: snapshot memory" || true   # no-op if nothing changed
   ```

3. **Fetch the remote's memory.** `git -C "$P" fetch <remote>`. Then capture the remote JSONL, if any:
   ```
   git -C "$P" show <upstream>:.genesis/memory/memory.jsonl > "$P/.genesis/memory/.incoming.jsonl" 2>/dev/null
   ```
   If that fails (the remote has no memory JSONL yet), skip to step 5 — there is nothing to reconcile.

4. **Reconcile losslessly** (this is the safety check — the JSONL decides before the `.db` is trusted):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli reconcile --repo "$P" --incoming "$P/.genesis/memory/.incoming.jsonl"
   ```
   Read the JSON `status`: `add-only` (remote had only new memories — safe), `already-synced` (identical), or
   `merged` (local-only memories were PRESERVED that a plain replace would have lost). Then remove the temp
   file and re-commit the reconciled memory:
   ```
   rm -f "$P/.genesis/memory/.incoming.jsonl"
   git -C "$P" add .genesis/memory.db .genesis/memory/memory.jsonl
   git -C "$P" commit -m "genesis: reconcile memory (<status>)" || true
   ```

5. **Push.** `git -C "$P" push`.
   - If it succeeds, report `status` and the counts (`added_from_incoming`, `kept_local_only`) from step 4.
   - If push is REJECTED (someone pushed in between), re-run from step 3 (fetch → reconcile → commit → push).
   - If git reports a **conflict it cannot resolve** (e.g. the JSONL is left with `<<<<<<<` markers, or an
     unrelated-histories error), **STOP and consult the user** — do not force-push, do not discard either
     side. This is the only case that needs a human; the reconcile itself never loses memory.

Report plainly what happened: the reconcile `status`, how many memories came from the remote, how many
local-only memories were preserved, and that both the `.db` and JSONL are now pushed.
