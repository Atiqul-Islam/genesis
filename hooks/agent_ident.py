#!/usr/bin/env python3
"""Shared helper: resolve the ACTIVE AGENT + the PROJECT runtime dir for a hook.

WHY THIS EXISTS (plugin re-architecture):
  Genesis's own team (sensei/method) is now shipped as PLUGIN agents. A plugin cannot put
  per-agent `hooks` in an agent's frontmatter (security-blocked), so enforcement moves to ONE
  session-level `hooks/hooks.json` at the plugin root. That single hooks.json has no per-agent
  argv the way the built-agent frontmatter hooks do — so each hook must derive WHICH agent is
  active from the hook PAYLOAD (the JSON on stdin).

AGENT-IDENTITY FIELD (verified — code.claude.com/docs/en/hooks, 2026-07-22):
  The payload carries `agent_type` — the agent/subagent name/type — on PreToolUse, Stop,
  SubagentStart, SubagentStop and SessionStart. Plugin-provided agents surface under their
  SCOPED name `<plugin>:<name>` (e.g. `genesis:method`; plugins-reference §Agents). `agent_id`
  is a unique per-invocation id (not a name), so we key on `agent_type` and normalize the scope.

  LIVE-INSTALL GATE (documented as uncertain, do NOT fake): whether a plugin's settings.json
  `{"agent":"sensei"}` main-thread activation populates `agent_type` on the top-level Stop/
  SessionStart payload is not explicitly guaranteed by the docs. So the plugin hooks.json passes
  `--main-agent sensei` as a fallback for the main thread; if `agent_type` IS present it wins.

RESOLUTION PRECEDENCE (keeps the built-agent frontmatter path + all existing tests intact):
  1. an explicit argv positional agent (assemble.py's built-agent frontmatter hooks pass it);
  2. the payload's `agent_type`, scope-normalized (the plugin session-level path);
  3. a `--main-agent <name>` fallback the plugin hooks.json supplies for the main thread.
"""
import os


def normalize(agent_type):
    """`genesis:method` / `^genesis:method$` / `method` -> `method`. '' -> ''."""
    if not agent_type:
        return ""
    a = str(agent_type).strip().strip("^$").strip()
    if ":" in a:                       # plugin-scoped "<plugin>:<name>" -> bare "<name>"
        a = a.rsplit(":", 1)[-1]
    return a.strip()


def agent_from_payload(ev):
    """The active agent name from a hook payload, or '' if none. Reads `agent_type`
    (docs: the agent/subagent name/type; camelCase variant tolerated defensively)."""
    if not isinstance(ev, dict):
        return ""
    return normalize(ev.get("agent_type") or ev.get("agentType") or "")


def resolve_agent(ev, argv_agent="", default_agent=""):
    """Active agent, precedence: explicit argv > payload agent_type > --main-agent fallback."""
    if argv_agent:
        return argv_agent
    a = agent_from_payload(ev)
    if a:
        return a
    return default_agent or ""


def split_args(argv):
    """Parse a Stop/SubagentStop hook's argv into (root, agent, main_agent).

    Tolerates the historical positional form `<root> <agent>` (built-agent frontmatter hooks +
    existing tests) MIXED with a `--main-agent <name>` flag the plugin hooks.json adds for the
    main thread. Returns root=None when no positional root was given (caller defaults to cwd)."""
    root, agent, main_agent, pos = None, "", "", []
    i = 0
    while i < len(argv):
        t = argv[i]
        if t == "--main-agent" and i + 1 < len(argv):
            main_agent = argv[i + 1]
            i += 2
            continue
        pos.append(t)
        i += 1
    if pos:
        root = pos[0]
    if len(pos) > 1:
        agent = pos[1]
    return root, agent, main_agent


def runtime_dir(cwd=None):
    """The PROJECT's writable `.genesis/` runtime dir, derived from the CWD (the repo the user
    is in). NEVER the plugin dir (read-only + version-churned ~14 days after an update) and NEVER
    hook-dir-relative. Guards against a nested `.genesis/.genesis` if cwd already IS a workspace."""
    cwd = cwd or os.getcwd()
    if os.path.basename(os.path.normpath(cwd)) == ".genesis":
        return cwd
    return os.path.join(cwd, ".genesis")
