#!/usr/bin/env python3
"""Tests for hooks/session_pointer.py + build_session_agent.resolve_session('current').
Run: python3 hooks/test_session_pointer.py"""
import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
GH = os.path.dirname(HERE)
POINTER = os.path.join(HERE, "session_pointer.py")
sys.path.insert(0, os.path.join(GH, "session_copy"))
import build_session_agent as B  # noqa: E402


def run(payload, cwd):
    subprocess.run([sys.executable, POINTER], input=json.dumps(payload), capture_output=True, text=True,
                   cwd=cwd, timeout=15)


def main():
    passed = failed = 0
    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    repo = tempfile.mkdtemp(prefix="ptr-")
    ptr = os.path.join(repo, ".genesis", "current-session.json")

    # 1. records the current session id
    run({"session_id": "SID-123", "transcript_path": "/x/SID-123.jsonl"}, repo)
    check("pointer file written", os.path.isfile(ptr))
    rec = json.load(open(ptr))
    check("pointer holds the session id + transcript", rec["session_id"] == "SID-123" and rec["transcript_path"].endswith("SID-123.jsonl"))

    # 2. accepts the camelCase sessionId variant too
    run({"sessionId": "SID-456"}, repo)
    check("pointer updates on a new turn (sessionId variant)", json.load(open(ptr))["session_id"] == "SID-456")

    # 3. no id -> no crash, no bogus write
    os.remove(ptr)
    run({"transcript_path": "/x/y.jsonl"}, repo)
    check("no session id -> no pointer written (fail-open)", not os.path.isfile(ptr))

    # 4. resolve_session('current') reads the pointer
    run({"session_id": "SID-789"}, repo)
    check("resolve_session('current') returns the pointer id", B.resolve_session("current", repo) == "SID-789")
    check("resolve_session passes through an explicit id", B.resolve_session("EXPLICIT", repo) == "EXPLICIT")

    # 5. missing pointer -> clear error
    empty = tempfile.mkdtemp(prefix="ptr-empty-")
    try:
        B.resolve_session("current", empty); raised = False
    except SystemExit:
        raised = True
    check("missing pointer -> SystemExit with guidance", raised)

    import shutil
    for d in (repo, empty):
        shutil.rmtree(d, ignore_errors=True)
    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
