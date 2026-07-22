#!/usr/bin/env python3
"""SESSION-POINTER hook (SessionStart | UserPromptSubmit | Stop) — records the LIVE session's id so
"copy my current session into an agent" knows WHICH session to capture.

The capture step (session_copy) needs the current session's id/transcript, but a Genesis agent (Sensei) runs
in its OWN session and can't see the user's main-session transcript. Solution: wire this tiny hook into the
repo's session so every turn it writes the current session id to <cwd>/.genesis/current-session.json. The
orchestrator resolves `--session current` from that pointer (the transcript itself is then found by globbing
~/.claude/projects/*/<id>.jsonl — encoding-agnostic, see capture.find_transcript).

Wire in the repo's .claude/settings.json:
  "SessionStart":      [{ "hooks":[{ "type":"command","command":"<python> <this>" }] }]
  "UserPromptSubmit":  [{ "hooks":[{ "type":"command","command":"<python> <this>" }] }]

Fail-open + silent: a pointer-write failure must never disrupt the session.
"""
import datetime, json, os, sys


def main():
    try:
        ev = json.load(sys.stdin)
    except Exception:
        ev = {}
    sid = ev.get("session_id") or ev.get("sessionId") or ""
    tpath = ev.get("transcript_path") or ""
    if not sid:
        sys.exit(0)  # nothing to record
    try:
        d = os.path.join(os.getcwd(), ".genesis")
        os.makedirs(d, exist_ok=True)
        rec = {"session_id": sid, "transcript_path": tpath,
               "ts": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")}
        with open(os.path.join(d, "current-session.json"), "w", encoding="utf-8") as fh:
            json.dump(rec, fh)
    except Exception:
        pass  # fail-open: never disrupt the session
    sys.exit(0)


if __name__ == "__main__":
    main()
