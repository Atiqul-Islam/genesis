#!/usr/bin/env python3
"""Session-Copy — Phase 2: store + summary (spec: docs/SESSION_COPY_AGENT_SPEC.md §6).

Takes the scrubbed `records.jsonl` from Phase 1 and produces the new agent's PORTABLE bundle under
<repo>/.genesis/agents/<name>/:
  history.sqlite   — the captured records in a clean, queryable table (travels with the repo; no plugin needed).
  summary.md       — a DETERMINISTIC running digest injected at session start (D5). An LLM can enrich it later
                     (Phase 3 live step); the deterministic baseline guarantees a useful, testable summary now.
  (embedding of the records into the Genesis memory server for semantic recall is embed.py, run separately,
   because it needs the built server binary — kept out of this pure/deterministic module so it is unit-testable.)

Deterministic + dependency-free (stdlib only). Run:
  python3 store.py --records <dir>/records.jsonl --out <repo>/.genesis/agents/<name>
"""
import argparse, json, os, sqlite3


def load_records(records_path):
    recs = []
    with open(records_path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                recs.append(json.loads(line))
            except Exception:
                continue
    return recs


def write_history_db(recs, db_path):
    """Portable SQLite: one row per captured record. Overwrites cleanly; travels with the repo."""
    if os.path.exists(db_path):
        os.remove(db_path)
    con = sqlite3.connect(db_path)
    try:
        con.execute("""CREATE TABLE records (
            seq INTEGER PRIMARY KEY, source TEXT, kind TEXT, ts TEXT, title TEXT, text TEXT)""")
        con.execute("CREATE INDEX idx_source ON records(source)")
        con.executemany(
            "INSERT INTO records (seq, source, kind, ts, title, text) VALUES (?,?,?,?,?,?)",
            [(i, r.get("source", ""), r.get("kind", ""), r.get("ts", ""), r.get("title", ""), r.get("text", ""))
             for i, r in enumerate(recs)])
        con.commit()
    finally:
        con.close()


def _counts(recs):
    c = {}
    for r in recs:
        c[r.get("source", "?")] = c.get(r.get("source", "?"), 0) + 1
    return c


def build_summary(recs, agent_name, recent_turns=12, memory_chars=4000):
    """A deterministic, portable digest: what was captured + the current MEMORY.md + the most recent turns.
    This is the baseline injected at start; a live LLM pass can replace/augment it (Phase 3)."""
    counts = _counts(recs)
    lines = [f"# Session-copy memory — {agent_name}", ""]
    lines.append("You were created by copying a prior Claude Code session. Its full history + memory is in your "
                 "portable store (`history.sqlite`) and is semantically recallable via your memory tools. This "
                 "summary is the always-loaded digest; recall specifics on demand.")
    lines.append("")
    lines.append("## What was carried over")
    for src in ("transcript", "auto-memory", "context-mode", "claude-mem", "genesis-memory", "user-config"):
        if src in counts:
            lines.append(f"- {src}: {counts[src]} records")
    lines.append("")

    # The prior session's index memory (MEMORY.md), if captured — the highest-signal standing context.
    mem = next((r for r in recs if r.get("source") == "auto-memory" and r.get("title") == "MEMORY.md"), None)
    if mem:
        body = mem.get("text", "").strip()
        lines.append("## Standing memory (from the prior session's MEMORY.md)")
        lines.append(body[:memory_chars] + ("\n…(truncated — recall the rest)" if len(body) > memory_chars else ""))
        lines.append("")

    # The tail of the conversation — most recent turns, compact.
    turns = [r for r in recs if r.get("source") == "transcript" and r.get("kind") in ("user", "assistant")]
    if turns:
        lines.append(f"## Most recent {min(recent_turns, len(turns))} turns (tail of the prior conversation)")
        for r in turns[-recent_turns:]:
            who = "You" if r.get("kind") == "assistant" else "User"
            snippet = " ".join(r.get("text", "").split())[:240]
            lines.append(f"- **{who}:** {snippet}")
        lines.append("")
    lines.append("_Recall any detail from the full history with your memory tools — it is all stored._")
    return "\n".join(lines)


def build_bundle(records_path, out_dir, agent_name=None):
    agent_name = agent_name or os.path.basename(os.path.normpath(out_dir)) or "agent"
    os.makedirs(out_dir, exist_ok=True)
    recs = load_records(records_path)
    db_path = os.path.join(out_dir, "history.sqlite")
    write_history_db(recs, db_path)
    summary = build_summary(recs, agent_name)
    summary_path = os.path.join(out_dir, "summary.md")
    with open(summary_path, "w", encoding="utf-8") as fh:
        fh.write(summary + "\n")
    manifest = {"agent": agent_name, "records": len(recs), "by_source": _counts(recs),
                "history_db": db_path, "summary": summary_path}
    with open(os.path.join(out_dir, "store_manifest.json"), "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, indent=2); fh.write("\n")
    return manifest


def main():
    ap = argparse.ArgumentParser(description="Build the portable session-copy store + summary.")
    ap.add_argument("--records", required=True, help="path to records.jsonl from capture.py")
    ap.add_argument("--out", required=True, help="the agent bundle dir (<repo>/.genesis/agents/<name>)")
    ap.add_argument("--name", default=None)
    a = ap.parse_args()
    print(json.dumps(build_bundle(a.records, a.out, a.name), indent=2))


if __name__ == "__main__":
    main()
