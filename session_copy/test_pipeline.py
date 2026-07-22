#!/usr/bin/env python3
"""End-to-end + PORTABILITY integration test for Session-Copy (Phases 1+2) against the REAL memory server.
Chains capture → store → embed on a synthetic session, then COPIES the agent bundle to a fresh location
(simulating a `git clone` on another machine) and proves the copied agent still recalls its history.
Skips if the server binary/model aren't built. Run: python3 session_copy/test_pipeline.py
"""
import json, os, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
GH = os.path.dirname(HERE)
sys.path.insert(0, HERE)
import capture as CAP, store as STORE, embed as EMB  # noqa: E402

BIN = os.path.join(GH, "server", "target", "release", "genesis-memory-server")
MODEL = os.path.join(GH, "server", "models")
SID = "pipeline-session-0001"


def _recall(agent_id, query, db, k=3):
    env = dict(os.environ); env["GENESIS_MODEL_DIR"] = MODEL; env["GENESIS_MEMORY_DB"] = db
    p = subprocess.Popen([BIN], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env)
    def rpc(o): p.stdin.write(json.dumps(o) + "\n"); p.stdin.flush(); return json.loads(p.stdout.readline())
    try:
        rpc({"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}})
        p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); p.stdin.flush()
        r = rpc({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                 "params": {"name": "recall", "arguments": {"agent_id": agent_id, "query": query, "k": k}}})
        return json.dumps(r.get("result", {}))
    finally:
        p.terminate()


def main():
    if not (os.path.exists(BIN) and os.path.isdir(MODEL)):
        print(f"  SKIP  memory server not built ({BIN})")
        print("\n0 passed, 0 failed (skipped)")
        return
    passed = failed = 0
    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    # ---- synthetic session on a fake ~/.claude ----
    home = tempfile.mkdtemp(prefix="pipe-home-")
    os.environ["CLAUDE_CONFIG_DIR"] = home
    proj = os.path.join(home, "projects", "-fake-repo"); os.makedirs(os.path.join(proj, "memory"))
    with open(os.path.join(proj, f"{SID}.jsonl"), "w", encoding="utf-8") as fh:
        fh.write(json.dumps({"type": "user", "timestamp": "t1",
                             "message": {"content": "We agreed the deploy script must never run on a red build."}}) + "\n")
        fh.write(json.dumps({"type": "assistant", "timestamp": "t2",
                             "message": {"content": [{"type": "text", "text": "Understood — I gate deploy on green CI only."}]}}) + "\n")
    with open(os.path.join(proj, "memory", "MEMORY.md"), "w", encoding="utf-8") as fh:
        fh.write("# index\n- The API base url is configured per environment, never hardcoded.\n")

    repo = tempfile.mkdtemp(prefix="pipe-repo-")
    agent_dir = os.path.join(repo, ".genesis", "agents", "deploybot")

    # ---- Phase 1: capture ----
    cap = CAP.capture(SID, "/fake/repo", agent_dir, include_user_config=False)
    check("capture produced records (transcript + memory)", cap["total_records"] >= 3)

    # ---- Phase 2: store + embed ----
    st = STORE.build_bundle(os.path.join(agent_dir, "records.jsonl"), agent_dir, "deploybot")
    check("store built history.sqlite + summary.md", os.path.isfile(os.path.join(agent_dir, "summary.md")))
    check("summary embeds standing memory", "API base url" in open(os.path.join(agent_dir, "summary.md")).read())

    mem_db = os.path.join(agent_dir, "memory.sqlite")
    em = EMB.embed_records(EMB._load_from_db(os.path.join(agent_dir, "history.sqlite")),
                           "deploybot", BIN, MODEL, mem_db)
    check("embed stored all records", em["failed"] == 0 and em["stored"] >= 3)

    # recall works on the freshly built agent
    hit = _recall("deploybot", "can we deploy when the build is failing?", mem_db)
    check("agent recalls its history semantically", "red build" in hit or "green CI" in hit)

    # ---- PORTABILITY: copy the bundle to a fresh location (simulate a git clone on machine B) ----
    machineB = tempfile.mkdtemp(prefix="pipe-cloneB-")
    b_agent = os.path.join(machineB, ".genesis", "agents", "deploybot")
    shutil.copytree(agent_dir, b_agent)
    b_mem = os.path.join(b_agent, "memory.sqlite")
    check("bundle files travelled (memory.sqlite + summary + history)",
          all(os.path.isfile(os.path.join(b_agent, f)) for f in ("memory.sqlite", "summary.md", "history.sqlite")))
    # recall from the COPIED db — no ~/.claude dependency, no re-embed
    b_hit = _recall("deploybot", "can we deploy when the build is failing?", b_mem)
    check("★ copied agent recalls history on 'machine B' (portable)", "red build" in b_hit or "green CI" in b_hit)

    for d in (home, repo, machineB):
        shutil.rmtree(d, ignore_errors=True)
    os.environ.pop("CLAUDE_CONFIG_DIR", None)
    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
