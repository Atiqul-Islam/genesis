#!/usr/bin/env python3
"""Session-Copy — Phase 1: capture/extract library (spec: docs/SESSION_COPY_AGENT_SPEC.md).

Extracts the readable CONTENT of every way Claude Code holds context/memory for a session, into ONE
normalized, credential-scrubbed record stream. We EXTRACT text rather than copy native plugin DBs — so the
result is portable (a target machine needs none of the source plugins to read it) and uniform for embedding
into the Genesis memory server (Phase 2).

Stores captured (verified on-disk schemas 2026-07-22):
  A. transcript      ~/.claude/projects/<enc>/<session>.jsonl  (user/assistant/system turns)
  B3 auto-memory     <same project dir>/memory/*.md            (MEMORY.md + topic files)
  B5a context-mode   ~/.claude/context-mode/content/*.db       (table `chunks(content, session_id, ...)`)
  B5b claude-mem     ~/.claude/projects/*claude-mem-observer*/*.jsonl (records with a `content` field)
  B4 genesis-memory  <GENESIS_MEMORY_DB>                        (only if the current agent already has one)
  C7 user config     ~/.claude/{CLAUDE.md, settings.json, skills/**/SKILL.md}  (snapshot; scrubbed hard)

Every extracted text is CREDENTIAL-SCRUBBED before it leaves this module — a matched secret becomes
"[REDACTED credential]"; the value is never written or returned. (Hard workspace rule.)

SCRUBBING — HONEST SCOPE (not a false guarantee): the scrubber catches (a) known credential SHAPES (AWS/GitHub/
OpenAI/Slack tokens, private-key blocks), (b) LABELLED secrets (`password:`/`api_key=`/`"token": "..."` incl. the
JSON form), and (c) any exact values the caller passes as `known_secrets` (a denylist — e.g. values a prior
credential scan found). It CANNOT detect an arbitrary UNLABELLED, unknown high-entropy string, because doing so
would false-redact legitimate hashes/UUIDs/base64 pervasive in real transcripts. This is defense-in-depth, not a
proof that no secret survives — surface any true secret value to the caller's denylist for a guaranteed removal.

Output: <out_dir>/records.jsonl  (one normalized record per line) + <out_dir>/capture_manifest.json.
A normalized record: {"source","kind","ts","title","text"}.

CLI:  python3 capture.py --session <id> --cwd <repo> --out <dir> [--genesis-db <path>] [--agent-id <name>]
"""
import argparse, glob, json, os, re, sqlite3, sys

# ---- credential scrubbing (reused patterns from hooks/gate.py + validate.py) --------------------------
_SECRET_PATTERNS = [
    re.compile(r"AKIA[0-9A-Z]{16}"),
    re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----[\s\S]*?-----END (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    # matches key=value, key: value, AND the JSON form "key": "value" (optional quote before the : / =)
    re.compile(r"(?i)(\b(?:password|passwd|secret|api[_-]?key|token|authorization|bearer)\b['\"]?\s*[:=]\s*['\"]?)([^\s'\"]{6,})"),
    re.compile(r"gh[pousr]_[A-Za-z0-9]{20,}"),          # GitHub tokens
    re.compile(r"sk-[A-Za-z0-9]{20,}"),                  # OpenAI-style keys
    re.compile(r"xox[baprs]-[A-Za-z0-9-]{10,}"),         # Slack tokens
]
_REDACTED = "[REDACTED credential]"
_KNOWN_SECRETS = []  # exact values to guarantee-redact; set per-capture via capture(known_secrets=...)


def scrub_text(text, known=None):
    """Return (scrubbed, n_redacted). Never returns a matched secret value.
    Redacts: caller-supplied exact `known` values (guaranteed) + known shapes + labelled secrets."""
    if not text:
        return text or "", 0
    n = 0
    out = text
    for val in (known if known is not None else _KNOWN_SECRETS):
        if val and val in out:
            out = out.replace(val, _REDACTED); n += 1
    for rx in _SECRET_PATTERNS:
        if rx.groups >= 2:  # keep the label, redact the value
            def _sub(m):
                nonlocal n
                n += 1
                return m.group(1) + _REDACTED
            out = rx.sub(_sub, out)
        else:
            new, k = rx.subn(_REDACTED, out)
            n += k
            out = new
    return out, n


# ---- path resolution (encoding-agnostic: find by the session id itself) ------------------------------
def claude_home():
    return os.environ.get("CLAUDE_CONFIG_DIR") or os.path.join(os.path.expanduser("~"), ".claude")


def find_transcript(session_id, cwd=None):
    """Locate <session>.jsonl by globbing every project dir (robust to path-encoding differences)."""
    home = claude_home()
    hits = glob.glob(os.path.join(home, "projects", "*", f"{session_id}.jsonl"))
    if hits:
        return hits[0]
    return None


def project_dir_for(transcript_path):
    """The ~/.claude/projects/<enc>/ dir that holds a transcript (auto-memory is its sibling)."""
    return os.path.dirname(transcript_path) if transcript_path else None


# ---- record helper -----------------------------------------------------------------------------------
def _rec(source, kind, ts, title, text):
    scrubbed, n = scrub_text(text if isinstance(text, str) else json.dumps(text, ensure_ascii=False))
    return {"source": source, "kind": kind, "ts": ts or "", "title": title or "", "text": scrubbed}, n


def _text_from_content(content):
    """Flatten an Anthropic message `content` (str | list of blocks) into readable text."""
    if isinstance(content, str):
        return content
    parts = []
    if isinstance(content, list):
        for b in content:
            if not isinstance(b, dict):
                parts.append(str(b)); continue
            t = b.get("type")
            if t == "text":
                parts.append(b.get("text", ""))
            elif t == "tool_use":
                parts.append(f"[tool_use {b.get('name','')}] " + json.dumps(b.get("input", {}), ensure_ascii=False)[:2000])
            elif t == "tool_result":
                c = b.get("content", "")
                parts.append("[tool_result] " + (_text_from_content(c) if not isinstance(c, str) else c)[:2000])
            elif t == "thinking":
                parts.append("[thinking] " + b.get("thinking", "")[:2000])
    return "\n".join(p for p in parts if p)


# ---- extractors: each yields (record, n_redacted) ----------------------------------------------------
def extract_transcript(transcript_path):
    """A: user/assistant/system conversation turns from the session .jsonl (streamed)."""
    recs, red = [], 0
    if not transcript_path or not os.path.isfile(transcript_path):
        return recs, red
    for line in open(transcript_path, encoding="utf-8", errors="replace"):
        line = line.strip()
        if not line:
            continue
        try:
            ev = json.loads(line)
        except Exception:
            continue
        typ = ev.get("type")
        if typ not in ("user", "assistant", "system"):
            continue
        ts = ev.get("timestamp", "")
        if typ == "system":
            text = ev.get("content", "") if isinstance(ev.get("content"), str) else _text_from_content(ev.get("content", ""))
        else:
            msg = ev.get("message", {}) or {}
            text = _text_from_content(msg.get("content", ""))
        if not text.strip():
            continue
        r, n = _rec("transcript", typ, ts, "", text)
        recs.append(r); red += n
    return recs, red


def extract_auto_memory(project_dir):
    """B3: MEMORY.md + topic files (already markdown)."""
    recs, red = [], 0
    mem = os.path.join(project_dir or "", "memory")
    if not os.path.isdir(mem):
        return recs, red
    for name in sorted(os.listdir(mem)):
        p = os.path.join(mem, name)
        if not (os.path.isfile(p) and name.endswith(".md")):
            continue
        try:
            txt = open(p, encoding="utf-8", errors="replace").read()
        except Exception:
            continue
        r, n = _rec("auto-memory", "memory-file", "", name, txt)
        recs.append(r); red += n
    return recs, red


def extract_contextmode(session_id, home=None):
    """B5a: readable text from context-mode content DBs — `chunks(title,content,session_id)` for THIS session."""
    recs, red = [], 0
    home = home or claude_home()
    for db in sorted(glob.glob(os.path.join(home, "context-mode", "content", "*.db"))):
        try:
            c = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
            has = c.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='chunks'").fetchone()
            if not has:
                c.close(); continue
            rows = c.execute("SELECT title, content, timestamp FROM chunks WHERE session_id=?",
                             (session_id,)).fetchall()
            for title, content, ts in rows:
                r, n = _rec("context-mode", "chunk", ts or "", title or "", content or "")
                recs.append(r); red += n
            c.close()
        except Exception:
            continue
    return recs, red


def extract_claude_mem(session_id, home=None):
    """B5b: observation `content` from claude-mem observer jsonl, scoped to EXACTLY this session.

    HONEST SCOPE (verified 2026-07-22): the observer jsonl carries the OBSERVER's own sessionId, not the main
    session's, and no project field — so from files we can only reliably capture records whose `sessionId`
    equals the given session. We do NOT fall back to "keep everything": that dumped the entire cross-project
    claude-mem corpus (~13k records / ~85MB in testing) — not this session's memory, and un-portable.
    COMPLETE project-scoped claude-mem capture requires claude-mem's own MCP search API and is done by the
    LIVE in-session capture step (Phase 3), which has MCP access; recorded here so it is not silently lost."""
    recs, red = [], 0
    home = home or claude_home()
    for f in sorted(glob.glob(os.path.join(home, "projects", "*claude-mem*observer*", "*.jsonl"))):
        for line in open(f, encoding="utf-8", errors="replace"):
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except Exception:
                continue
            content = ev.get("content")
            if not content or ev.get("sessionId") != session_id:
                continue
            r, n = _rec("claude-mem", ev.get("operation", "observation"), ev.get("timestamp", ""),
                        "session-match", content)
            recs.append(r); red += n
    return recs, red


def extract_genesis_memory(db_path, agent_id):
    """B4: the current agent's own Genesis semantic memory rows, if it has any."""
    recs, red = [], 0
    if not db_path or not os.path.isfile(db_path):
        return recs, red
    try:
        c = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
        tbls = {r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall()}
        tbl = "memories" if "memories" in tbls else ("memory" if "memory" in tbls else None)
        if tbl:
            cols = [r[1] for r in c.execute(f"PRAGMA table_info('{tbl}')").fetchall()]
            textcol = next((x for x in ("text", "content", "body") if x in cols), None)
            idcol = next((x for x in ("agent_id", "agent") if x in cols), None)
            if textcol:
                q = f"SELECT {textcol} FROM {tbl}"
                args = ()
                if idcol and agent_id:
                    q += f" WHERE {idcol}=?"; args = (agent_id,)
                for (txt,) in c.execute(q, args).fetchall():
                    r, n = _rec("genesis-memory", "memory", "", "", txt or "")
                    recs.append(r); red += n
        c.close()
    except Exception:
        pass
    return recs, red


def extract_user_config(home=None):
    """C7: user-level/global config snapshot (scrubbed). settings.json values are scrubbed hard."""
    recs, red = [], 0
    home = home or claude_home()
    for rel in ("CLAUDE.md",):
        p = os.path.join(home, rel)
        if os.path.isfile(p):
            r, n = _rec("user-config", "claude-md", "", rel, open(p, encoding="utf-8", errors="replace").read())
            recs.append(r); red += n
    sj = os.path.join(home, "settings.json")
    if os.path.isfile(sj):
        raw = open(sj, encoding="utf-8", errors="replace").read()
        r, n = _rec("user-config", "settings", "", "settings.json", raw)  # scrub_text redacts secret values
        recs.append(r); red += n
    for skill in sorted(glob.glob(os.path.join(home, "skills", "**", "SKILL.md"), recursive=True)):
        try:
            txt = open(skill, encoding="utf-8", errors="replace").read()
        except Exception:
            continue
        r, n = _rec("user-config", "skill", "", os.path.relpath(skill, home), txt)
        recs.append(r); red += n
    return recs, red


# ---- orchestration -----------------------------------------------------------------------------------
def capture(session_id, cwd, out_dir, genesis_db=None, agent_id=None, include_user_config=True,
            known_secrets=None):
    global _KNOWN_SECRETS
    _KNOWN_SECRETS = list(known_secrets or [])
    transcript = find_transcript(session_id, cwd)
    proj = project_dir_for(transcript)
    extractors = [
        ("transcript", lambda: extract_transcript(transcript)),
        ("auto-memory", lambda: extract_auto_memory(proj)),
        ("context-mode", lambda: extract_contextmode(session_id)),
        ("claude-mem", lambda: extract_claude_mem(session_id)),
        ("genesis-memory", lambda: extract_genesis_memory(genesis_db, agent_id)),
    ]
    if include_user_config:
        extractors.append(("user-config", lambda: extract_user_config()))

    os.makedirs(out_dir, exist_ok=True)
    per_source, total, total_red = {}, 0, 0
    out_path = os.path.join(out_dir, "records.jsonl")
    with open(out_path, "w", encoding="utf-8") as fh:
        for name, fn in extractors:
            try:
                recs, red = fn()
            except Exception as e:
                per_source[name] = {"records": 0, "redactions": 0, "error": f"{type(e).__name__}: {e}"}
                continue
            for r in recs:
                fh.write(json.dumps(r, ensure_ascii=False) + "\n")
            per_source[name] = {"records": len(recs), "redactions": red}
            total += len(recs); total_red += red

    manifest = {
        "session_id": session_id, "cwd": os.path.abspath(cwd) if cwd else None,
        "transcript": transcript, "project_dir": proj,
        "records_file": out_path, "total_records": total, "total_redactions": total_red,
        "by_source": per_source,
    }
    with open(os.path.join(out_dir, "capture_manifest.json"), "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, indent=2)
        fh.write("\n")
    return manifest


def main():
    ap = argparse.ArgumentParser(description="Capture a Claude Code session's context/memory (scrubbed).")
    ap.add_argument("--session", required=True)
    ap.add_argument("--cwd", default=os.getcwd())
    ap.add_argument("--out", required=True)
    ap.add_argument("--genesis-db", default=None)
    ap.add_argument("--agent-id", default=None)
    ap.add_argument("--no-user-config", action="store_true")
    ap.add_argument("--known-secret", action="append", default=[],
                    help="exact secret value to guarantee-redact (repeatable)")
    a = ap.parse_args()
    m = capture(a.session, a.cwd, a.out, a.genesis_db, a.agent_id, include_user_config=not a.no_user_config,
                known_secrets=a.known_secret)
    print(json.dumps({k: v for k, v in m.items() if k != "by_source"} | {"by_source": m["by_source"]}, indent=2))


if __name__ == "__main__":
    main()
