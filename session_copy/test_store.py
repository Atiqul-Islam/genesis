#!/usr/bin/env python3
"""Tests for session_copy/store.py (Phase 2, deterministic). Run: python3 session_copy/test_store.py"""
import json, os, sqlite3, sys, tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import store as S  # noqa: E402

RECS = [
    {"source": "auto-memory", "kind": "memory-file", "ts": "", "title": "MEMORY.md",
     "text": "# index\n- fact one\n- fact two\n"},
    {"source": "transcript", "kind": "user", "ts": "t1", "title": "", "text": "please build the widget"},
    {"source": "transcript", "kind": "assistant", "ts": "t2", "title": "", "text": "widget built and tested"},
    {"source": "context-mode", "kind": "chunk", "ts": "t3", "title": "Doc", "text": "indexed doc text"},
]


def main():
    passed = failed = 0

    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    tmp = tempfile.mkdtemp(prefix="store-")
    recs_path = os.path.join(tmp, "records.jsonl")
    with open(recs_path, "w", encoding="utf-8") as fh:
        for r in RECS:
            fh.write(json.dumps(r) + "\n")

    out = os.path.join(tmp, ".genesis", "agents", "widgetbot")
    m = S.build_bundle(recs_path, out, "widgetbot")

    # history.sqlite
    db = os.path.join(out, "history.sqlite")
    check("history.sqlite created", os.path.isfile(db))
    con = sqlite3.connect(db)
    n = con.execute("SELECT COUNT(*) FROM records").fetchone()[0]
    srcs = {r[0] for r in con.execute("SELECT DISTINCT source FROM records").fetchall()}
    txt = con.execute("SELECT text FROM records WHERE source='transcript' AND kind='user'").fetchone()[0]
    con.close()
    check("all records stored", n == len(RECS))
    check("every source present in db", srcs == {"auto-memory", "transcript", "context-mode"})
    check("record text preserved", txt == "please build the widget")

    # summary.md
    sp = os.path.join(out, "summary.md")
    summary = open(sp, encoding="utf-8").read()
    check("summary.md created", os.path.isfile(sp))
    check("summary lists carried-over sources + counts", "transcript: 2 records" in summary and "auto-memory: 1 records" in summary)
    check("summary embeds prior MEMORY.md", "fact one" in summary and "Standing memory" in summary)
    check("summary shows recent turns", "widget built and tested" in summary and "Most recent" in summary)
    check("summary tells the agent history is recallable", "recall" in summary.lower())

    # manifest
    check("manifest counts records + sources", m["records"] == len(RECS) and m["by_source"]["transcript"] == 2)

    # portability: the bundle is self-contained files under .genesis/agents/<name>/
    files = set(os.listdir(out))
    check("bundle is self-contained (db + summary + manifest)",
          {"history.sqlite", "summary.md", "store_manifest.json"} <= files)

    # re-run overwrites cleanly (idempotent history db)
    m2 = S.build_bundle(recs_path, out, "widgetbot")
    con = sqlite3.connect(os.path.join(out, "history.sqlite"))
    n2 = con.execute("SELECT COUNT(*) FROM records").fetchone()[0]; con.close()
    check("re-run is idempotent (no duplicate rows)", n2 == len(RECS))

    import shutil
    shutil.rmtree(tmp, ignore_errors=True)
    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
