#!/usr/bin/env python3
"""Tests for session_copy/capture.py (Phase 1). Synthetic data matches the REAL on-disk schemas verified
2026-07-22. The load-bearing assertion: a planted credential value NEVER appears in the output — only the
label survives, redacted. No network. Run:  python3 session_copy/test_capture.py
"""
import json, os, sqlite3, sys, tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import capture as C  # noqa: E402

SECRET = "S3cr3tV@lue987654"  # distinctive; must never appear un-redacted in any output


def main():
    passed = failed = 0

    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    # 1. scrub_text across forms
    forms = [
        f'password = {SECRET}',
        f'"api_key": "{SECRET}"',
        f'token: {SECRET}',
        f'AKIA1234567890ABCDEF',
        f'ghp_{"a"*30}',
        f'sk-{"b"*30}',
        "-----BEGIN PRIVATE KEY-----\nMIIabc\n-----END PRIVATE KEY-----",
    ]
    all_scrubbed = True
    for f in forms:
        out, n = C.scrub_text(f)
        if n == 0 or SECRET in out or "AKIA1234567890ABCDEF" in out or "ghp_"+"a"*30 in out or "sk-"+"b"*30 in out or "MIIabc" in out:
            all_scrubbed = False
    check("scrub_text redacts every secret form (value never survives)", all_scrubbed)
    check("scrub_text keeps the label", "password" in C.scrub_text(f'password = {SECRET}')[0])
    check("scrub_text no-op on clean text", C.scrub_text("hello world")[1] == 0)
    # Honest limitation made visible: a BARE unlabelled unknown value is NOT auto-detectable...
    bare = "random note " + SECRET
    check("HONEST LIMIT: bare unlabelled unknown value is not auto-caught", C.scrub_text(bare)[1] == 0)
    # ...but the exact-value denylist guarantees its removal.
    o, n = C.scrub_text(bare, known=[SECRET])
    check("known-denylist guarantees removal of a bare value", n == 1 and SECRET not in o)

    # 2. _text_from_content
    t = C._text_from_content([{"type": "text", "text": "hi"},
                              {"type": "tool_use", "name": "Bash", "input": {"command": "ls"}},
                              {"type": "tool_result", "content": "output here"}])
    check("_text_from_content flattens blocks", "hi" in t and "Bash" in t and "output here" in t)
    check("_text_from_content handles a plain string", C._text_from_content("plain") == "plain")

    home = tempfile.mkdtemp(prefix="cc-home-")
    os.environ["CLAUDE_CONFIG_DIR"] = home
    SID = "test-session-uuid-0001"
    proj = os.path.join(home, "projects", "-tmp-fake-repo")
    os.makedirs(os.path.join(proj, "memory"))

    # 3. transcript
    tpath = os.path.join(proj, f"{SID}.jsonl")
    with open(tpath, "w", encoding="utf-8") as fh:
        fh.write(json.dumps({"type": "ai-title", "sessionId": SID}) + "\n")          # metadata (ignored)
        fh.write(json.dumps({"type": "user", "timestamp": "t1",
                             "message": {"content": f"my password = {SECRET} keep it"}}) + "\n")
        fh.write(json.dumps({"type": "assistant", "timestamp": "t2",
                             "message": {"content": [{"type": "text", "text": "understood"}]}}) + "\n")
        fh.write(json.dumps({"type": "system", "timestamp": "t3", "content": "a system note"}) + "\n")
        fh.write(json.dumps({"type": "mode", "permission-mode": "auto"}) + "\n")       # metadata (ignored)
    recs, red = C.extract_transcript(tpath)
    kinds = {r["kind"] for r in recs}
    blob = json.dumps(recs)
    check("transcript extracts only conversation turns", kinds == {"user", "assistant", "system"} and len(recs) == 3)
    check("transcript scrubs a planted secret", red >= 1 and SECRET not in blob and "password" in blob)

    # 4. auto-memory
    with open(os.path.join(proj, "memory", "MEMORY.md"), "w", encoding="utf-8") as fh:
        fh.write("# index\n- a note\n")
    with open(os.path.join(proj, "memory", "topic.md"), "w", encoding="utf-8") as fh:
        fh.write(f"detail with token: {SECRET}\n")
    recs, red = C.extract_auto_memory(proj)
    check("auto-memory reads MEMORY.md + topic files", len(recs) == 2 and any(r["title"] == "MEMORY.md" for r in recs))
    check("auto-memory scrubs secrets", red >= 1 and SECRET not in json.dumps(recs))

    # 5. context-mode (real chunks schema)
    cmdir = os.path.join(home, "context-mode", "content")
    os.makedirs(cmdir)
    db = os.path.join(cmdir, "abc123.db")
    con = sqlite3.connect(db)
    con.execute("CREATE TABLE chunks (title TEXT, content TEXT, source_id INT, content_type TEXT, source_category TEXT, session_id TEXT, event_id INT, timestamp TEXT)")
    con.execute("INSERT INTO chunks (title,content,session_id,timestamp) VALUES (?,?,?,?)",
                ("Doc A", f"indexed text with token: {SECRET}", SID, "t9"))
    con.execute("INSERT INTO chunks (title,content,session_id,timestamp) VALUES (?,?,?,?)",
                ("Other", "belongs to another session", "OTHER-SID", "t9"))
    con.commit(); con.close()
    recs, red = C.extract_contextmode(SID)
    check("context-mode extracts THIS session's chunks only", len(recs) == 1 and recs[0]["title"] == "Doc A")
    check("context-mode scrubs secrets", red >= 1 and SECRET not in json.dumps(recs))

    # 6. claude-mem observer jsonl
    obsdir = os.path.join(home, "projects", "-home-x--claude-mem-observer-sessions")
    os.makedirs(obsdir)
    with open(os.path.join(obsdir, "obs.jsonl"), "w", encoding="utf-8") as fh:
        fh.write(json.dumps({"type": "observation", "operation": "add", "timestamp": "t",
                             "sessionId": SID, "content": f"observed api_key={SECRET}"}) + "\n")
        fh.write(json.dumps({"type": "observation", "operation": "add", "timestamp": "t",
                             "sessionId": "OTHER", "content": "other session obs"}) + "\n")
    recs, red = C.extract_claude_mem(SID)
    check("claude-mem keeps ONLY session-matched observations", len(recs) == 1 and recs[0]["title"] == "session-match")
    check("claude-mem scrubs secrets", red >= 1 and SECRET not in json.dumps(recs))
    # the fix: a session with no matches captures NOTHING (never dumps the whole corpus)
    check("claude-mem no-match → 0 records (no corpus dump)", len(C.extract_claude_mem("NOPE-SID")[0]) == 0)

    # 7. genesis memory
    gdb = os.path.join(home, "gmem.db")
    con = sqlite3.connect(gdb)
    con.execute("CREATE TABLE memories (agent_id TEXT, text TEXT)")
    con.execute("INSERT INTO memories VALUES (?,?)", ("mine", "a durable fact"))
    con.execute("INSERT INTO memories VALUES (?,?)", ("other", "not mine"))
    con.commit(); con.close()
    recs, red = C.extract_genesis_memory(gdb, "mine")
    check("genesis-memory extracts this agent's rows", len(recs) == 1 and "durable fact" in recs[0]["text"])

    # 8. user config
    with open(os.path.join(home, "CLAUDE.md"), "w", encoding="utf-8") as fh:
        fh.write("# user rules\n")
    with open(os.path.join(home, "settings.json"), "w", encoding="utf-8") as fh:
        fh.write(json.dumps({"env": {"API_KEY": SECRET}, "model": "opus"}))
    skdir = os.path.join(home, "skills", "myskill")
    os.makedirs(skdir)
    with open(os.path.join(skdir, "SKILL.md"), "w", encoding="utf-8") as fh:
        fh.write("---\nname: myskill\n---\nbody\n")
    recs, red = C.extract_user_config()
    blob = json.dumps(recs)
    check("user-config captures CLAUDE.md + settings + skills", len(recs) == 3)
    check("user-config scrubs settings secrets (value gone)", SECRET not in blob and "API_KEY" in blob)

    # 9. capture() end-to-end — THE portability + no-leak guarantee.
    #    Plant a BARE secret in the transcript too, and rely on the denylist to guarantee its removal.
    with open(tpath, "a", encoding="utf-8") as fh:
        fh.write(json.dumps({"type": "user", "timestamp": "t4",
                             "message": {"content": f"here is a raw value {SECRET} with no label"}}) + "\n")
    out = tempfile.mkdtemp(prefix="cc-cap-")
    m = C.capture(SID, "/tmp/fake/repo", out, genesis_db=gdb, agent_id="mine", known_secrets=[SECRET])
    recs_path = os.path.join(out, "records.jsonl")
    all_text = open(recs_path, encoding="utf-8").read()
    check("capture() wrote records.jsonl + manifest",
          os.path.isfile(recs_path) and os.path.isfile(os.path.join(out, "capture_manifest.json")))
    check("capture() total_records covers every source", m["total_records"] >= 8 and len(m["by_source"]) == 6)
    check("capture() manifest counts redactions", m["total_redactions"] >= 5)
    check("★ NO planted credential value anywhere (incl. a bare one via denylist)", SECRET not in all_text)
    check("capture() resolved the transcript by session id", m["transcript"] == tpath)

    import shutil
    for d in (home, out):
        shutil.rmtree(d, ignore_errors=True)
    os.environ.pop("CLAUDE_CONFIG_DIR", None)

    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
