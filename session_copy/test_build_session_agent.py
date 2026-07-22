#!/usr/bin/env python3
"""Integration test for the Phase 3 orchestrator (build_session_agent.py) against the REAL memory server.
Proves: one call captures a live session, lays down the bundle where inject.py finds it, and embeds the
history into the repo's SHARED memory under agent_id=<name> so it is recallable. Skips if server not built.
Run: python3 session_copy/test_build_session_agent.py
"""
import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
GH = os.path.dirname(HERE)
sys.path.insert(0, HERE)
import build_session_agent as B  # noqa: E402

BIN = os.path.join(GH, "server", "target", "release", "genesis-memory-server")
MODEL = os.path.join(GH, "server", "models")
INJECT = os.path.join(GH, "hooks", "inject.py")
SID = "orch-session-0001"


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

    # synthetic live session
    home = tempfile.mkdtemp(prefix="orch-home-")
    os.environ["CLAUDE_CONFIG_DIR"] = home
    proj = os.path.join(home, "projects", "-fake"); os.makedirs(os.path.join(proj, "memory"))
    with open(os.path.join(proj, f"{SID}.jsonl"), "w", encoding="utf-8") as fh:
        fh.write(json.dumps({"type": "user", "timestamp": "t1",
                             "message": {"content": "Remember: the nightly job must skip weekends."}}) + "\n")
        fh.write(json.dumps({"type": "assistant", "timestamp": "t2",
                             "message": {"content": [{"type": "text", "text": "Got it — nightly runs Mon–Fri only."}]}}) + "\n")
    with open(os.path.join(proj, "memory", "MEMORY.md"), "w", encoding="utf-8") as fh:
        fh.write("# index\n- Use UTC for all scheduled jobs.\n")

    repo = tempfile.mkdtemp(prefix="orch-repo-")
    ghome = os.path.join(repo, ".genesis"); os.makedirs(ghome)
    mem_db = os.path.join(ghome, "memory.db")

    m = B.build(SID, "nightbot", repo, ghome, BIN, MODEL, mem_db, include_user_config=False)
    check("orchestrator captured + embedded", m["captured"] >= 3 and m["embed_failed"] == 0 and m["embedded"] >= 3)

    # bundle laid down where inject.py looks (<genesis_home>/agents/<name>/summary.md)
    bundle = os.path.join(ghome, "agents", "nightbot")
    check("bundle at <genesis_home>/agents/<name>/", os.path.isfile(os.path.join(bundle, "summary.md"))
          and os.path.isfile(os.path.join(bundle, "history.sqlite")))

    # the agent recalls its carried-over history from the SHARED repo memory under its id
    hit = _recall("nightbot", "does the nightly job run on saturday?", mem_db)
    check("agent recalls carried-over history under its id", "weekend" in hit or "Mon" in hit)

    # inject.py surfaces the summary for this agent (exp_dir = <genesis_home>/expertise)
    os.makedirs(os.path.join(ghome, "expertise"), exist_ok=True)
    p = subprocess.run([sys.executable, INJECT, os.path.join(ghome, "expertise"), "nightbot"],
                       input="{}", capture_output=True, text=True, cwd=repo, timeout=20)
    ctx = json.loads(p.stdout.strip()).get("hookSpecificOutput", {}).get("additionalContext", "")
    check("inject.py surfaces the session-copy summary at start",
          "carried-over session memory" in ctx and "UTC" in ctx)

    import shutil
    for d in (home, repo):
        shutil.rmtree(d, ignore_errors=True)
    os.environ.pop("CLAUDE_CONFIG_DIR", None)
    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
