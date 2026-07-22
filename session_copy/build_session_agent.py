#!/usr/bin/env python3
"""Session-Copy — Phase 3: orchestrator. One call turns a live session into a session-copy agent's MEMORY.

Chains Phase 1+2 for the SESSION-COPY part of a build:
  1. capture   the current session's stores (scrubbed)            -> <bundle>/records.jsonl
  2. store     portable history + start-time summary              -> <bundle>/history.sqlite, summary.md
  3. embed     the records into the repo's shared Genesis memory  -> under agent_id=<name>  (recallable)

The bundle lives at <genesis_home>/agents/<name>/ — exactly where inject.py looks for summary.md, and the
records are embedded under agent_id=<name> in the repo's shared memory DB, so the assembled agent (built by
Sensei/Method the normal way) recalls its carried-over history via its standard `recall` tool.

This does NOT author the agent's persona or run the assembler — that is Sensei's normal single-agent build
(the "custom" specialization). This module owns only the copy-the-session-into-memory half. build-agent Step
2a invokes it when the user chose "copy my current session".

Run:
  python3 build_session_agent.py --session <id> --name <agent> --repo <target_repo> \
      --genesis-home <repo>/.genesis --server-bin <...>/genesis-memory-server \
      --model-dir <...>/models --memory-db <repo>/.genesis/memory.db [--known-secret V ...] [--no-user-config]
"""
import argparse, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import capture as CAP  # noqa: E402
import store as STORE  # noqa: E402
import embed as EMB    # noqa: E402


def resolve_session(session_id, repo):
    """Resolve `--session current` from the session-pointer hook's <repo>/.genesis/current-session.json."""
    if session_id and session_id != "current":
        return session_id
    ptr = os.path.join(repo, ".genesis", "current-session.json")
    try:
        sid = json.load(open(ptr, encoding="utf-8")).get("session_id", "")
    except Exception:
        sid = ""
    if not sid:
        raise SystemExit("session '--current' requested but no current-session.json pointer found — is the "
                         "session_pointer hook wired into this repo? (or pass an explicit --session <id>)")
    return sid


def build(session_id, name, repo, genesis_home, server_bin, model_dir, memory_db,
          known_secrets=None, include_user_config=True):
    session_id = resolve_session(session_id, repo)
    bundle = os.path.join(genesis_home, "agents", name)
    os.makedirs(bundle, exist_ok=True)

    # 1. capture (scrubbed) — the current agent has no prior Genesis memory to merge for a main-session copy.
    cap = CAP.capture(session_id, repo, bundle, genesis_db=None, agent_id=None,
                      include_user_config=include_user_config, known_secrets=known_secrets or [])

    # 2. store: portable history + start-time summary digest.
    st = STORE.build_bundle(os.path.join(bundle, "records.jsonl"), bundle, name)

    # 3. embed the records into the repo's SHARED memory DB under agent_id=<name> (Genesis isolates by id).
    recs = EMB._load_from_db(os.path.join(bundle, "history.sqlite"))
    em = EMB.embed_records(recs, name, server_bin, model_dir, memory_db)

    manifest = {
        "agent": name, "session_id": session_id, "repo": os.path.abspath(repo),
        "bundle": bundle, "memory_db": memory_db,
        "captured": cap["total_records"], "redactions": cap["total_redactions"],
        "embedded": em["stored"], "embed_failed": em["failed"],
        "summary": os.path.join(bundle, "summary.md"),
        "by_source": cap["by_source"],
        "next": "Sensei: author the agent's specialized persona (Method), then assemble.py — the agent will "
                "recall this history via its memory tools and load summary.md at start.",
    }
    with open(os.path.join(bundle, "session_copy_manifest.json"), "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, indent=2); fh.write("\n")
    return manifest


def main():
    ap = argparse.ArgumentParser(description="Build a session-copy agent's memory from a live session.")
    ap.add_argument("--session", required=True)
    ap.add_argument("--name", required=True)
    ap.add_argument("--repo", required=True)
    ap.add_argument("--genesis-home", required=True)
    ap.add_argument("--server-bin", required=True)
    ap.add_argument("--model-dir", required=True)
    ap.add_argument("--memory-db", required=True)
    ap.add_argument("--known-secret", action="append", default=[])
    ap.add_argument("--no-user-config", action="store_true")
    a = ap.parse_args()
    m = build(a.session, a.name, a.repo, a.genesis_home, a.server_bin, a.model_dir, a.memory_db,
              known_secrets=a.known_secret, include_user_config=not a.no_user_config)
    print(json.dumps(m, indent=2))


if __name__ == "__main__":
    main()
