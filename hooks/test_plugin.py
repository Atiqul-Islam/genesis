#!/usr/bin/env python3
"""Tests for the PLUGIN scaffold + the payload agent-derivation (the plugin re-architecture, items A + B).

Proves the shipped plugin artifacts obey the verified plugin constraints (no self-verification of a live
install — that is a manual gate; here we prove the STATIC artifacts are correct):
  * .claude-plugin/plugin.json parses; name == genesis
  * agents/{sensei,method}.md frontmatter carries NONE of hooks/mcpServers/permissionMode (security-blocked)
    and uses the plugin-scoped memory tool names; Method has NO `Agent` tool (its boundary)
  * hooks/hooks.json parses, uses ${CLAUDE_PLUGIN_ROOT}, wires the right scripts to the right events
  * .mcp.json parses + references the ${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory launcher
  * settings.json == {"agent": "sensei"}
  * agent_ident derives the active agent from a representative hook payload

Run:  python3 hooks/test_plugin.py
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)                       # hooks/ -> plugin root
sys.path.insert(0, HERE)
import agent_ident  # noqa: E402

SCOPED = ["mcp__plugin_genesis_genesis-memory__store",
          "mcp__plugin_genesis_genesis-memory__recall",
          "mcp__plugin_genesis_genesis-memory__consolidate"]
BANNED_KEYS = {"hooks", "mcpServers", "permissionMode"}


def frontmatter_and_body(path):
    txt = open(path, encoding="utf-8").read()
    assert txt.startswith("---\n"), path
    _, fm, body = txt.split("---\n", 2)
    return fm, body


def fm_keys(fm):
    """Top-level YAML keys in a frontmatter block (lines like `key: ...`)."""
    keys = []
    for line in fm.splitlines():
        if line and not line[0].isspace() and ":" in line:
            keys.append(line.split(":", 1)[0].strip())
    return keys


def fm_value(fm, key):
    for line in fm.splitlines():
        if line.startswith(key + ":"):
            return line.split(":", 1)[1].strip()
    return ""


def tool_set(fm):
    return {t.strip() for t in fm_value(fm, "tools").split(",") if t.strip()}


def main():
    passed = failed = 0

    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    # ---- plugin.json ----
    pj = json.load(open(os.path.join(REPO, ".claude-plugin", "plugin.json"), encoding="utf-8"))
    check("plugin.json parses and name == genesis", pj.get("name") == "genesis")

    # ---- agents/*.md ----
    for name, wants_agent in (("sensei", True), ("method", False)):
        fm, body = frontmatter_and_body(os.path.join(REPO, "agents", f"{name}.md"))
        keys = set(fm_keys(fm))
        check(f"{name}.md frontmatter has NO hooks/mcpServers/permissionMode",
              not (keys & BANNED_KEYS))
        tools = tool_set(fm)
        check(f"{name}.md uses the 3 plugin-scoped memory tools", all(s in tools for s in SCOPED))
        check(f"{name}.md declares name/description/tools", {"name", "description", "tools"} <= keys)
        check(f"{name}.md {'has' if wants_agent else 'has NO'} Agent tool",
              ("Agent" in tools) == wants_agent)
        check(f"{name}.md body carries the persona (mentions {name.capitalize()})",
              name.capitalize() in body)
    # method must NOT be able to spawn/delegate; sensei must.
    sfm, _ = frontmatter_and_body(os.path.join(REPO, "agents", "sensei.md"))
    check("sensei preloads build-agent + research-expertise skills",
          "build-agent" in fm_value(sfm, "skills") and "research-expertise" in fm_value(sfm, "skills"))

    # ---- hooks/hooks.json ----
    hj = json.load(open(os.path.join(REPO, "hooks", "hooks.json"), encoding="utf-8"))
    H = hj.get("hooks", {})
    check("hooks.json parses with a 'hooks' object", isinstance(H, dict) and H)
    for ev in ("SessionStart", "PreToolUse", "Stop", "SubagentStop"):
        check(f"hooks.json wires {ev}", ev in H)
    # collect every command string
    cmds = [h["command"] for blocks in H.values() for blk in blocks for h in blk.get("hooks", [])]
    check("every hook command uses ${CLAUDE_PLUGIN_ROOT}", all("${CLAUDE_PLUGIN_ROOT}" in c for c in cmds))
    for script in ("inject.py", "gate.py", "enforce_research.py", "validate.py", "review.py"):
        check(f"hooks.json references {script}", any(script in c for c in cmds))
    pre_matchers = {blk.get("matcher") for blk in H.get("PreToolUse", [])}
    check("PreToolUse matches Write|Edit and Bash", {"Write|Edit", "Bash"} <= pre_matchers)
    sub_matchers = " ".join(blk.get("matcher", "") for blk in H.get("SubagentStop", []))
    check("SubagentStop matcher targets method", "method" in sub_matchers)
    check("Stop runs validate THEN review",
          any("validate.py" in h["command"] for blk in H["Stop"] for h in blk["hooks"])
          and any("review.py" in h["command"] for blk in H["Stop"] for h in blk["hooks"]))

    # ---- .mcp.json ----
    mj = json.load(open(os.path.join(REPO, ".mcp.json"), encoding="utf-8"))
    gm = mj["mcpServers"]["genesis-memory"]
    check(".mcp.json launcher: python3 + ${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory",
          gm["command"] == "python3" and any("${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory" in a for a in gm["args"]))

    # ---- settings.json ----
    sj = json.load(open(os.path.join(REPO, "settings.json"), encoding="utf-8"))
    check("settings.json == {'agent': 'sensei'}", sj == {"agent": "sensei"})

    # ---- agent_ident: payload derivation ----
    check("normalize strips the plugin scope (genesis:method -> method)",
          agent_ident.normalize("genesis:method") == "method" and agent_ident.normalize("^genesis:method$") == "method")
    check("resolve from SubagentStop payload agent_type",
          agent_ident.resolve_agent({"agent_type": "genesis:method"}) == "method")
    check("resolve bare agent_type",
          agent_ident.resolve_agent({"agent_type": "sensei"}) == "sensei")
    check("argv positional wins over payload",
          agent_ident.resolve_agent({"agent_type": "method"}, "acme-bot") == "acme-bot")
    check("main-thread fallback when payload omits agent_type",
          agent_ident.resolve_agent({}, "", "sensei") == "sensei")
    check("no identity anywhere -> ''", agent_ident.resolve_agent({}) == "")
    check("split_args parses . --main-agent sensei",
          agent_ident.split_args([".", "--main-agent", "sensei"]) == (".", "", "sensei"))
    check("split_args keeps the historical <root> <agent> form",
          agent_ident.split_args([".", "method"]) == (".", "method", ""))

    # ---- drift: committed agents/*.md == what build_plugin_agents.py regenerates from team/ sources ----
    import subprocess
    before = {n: open(os.path.join(REPO, "agents", f"{n}.md"), encoding="utf-8").read() for n in ("sensei", "method")}
    r = subprocess.run([sys.executable, os.path.join(REPO, "install", "build_plugin_agents.py"), REPO],
                       capture_output=True, text=True)
    after = {n: open(os.path.join(REPO, "agents", f"{n}.md"), encoding="utf-8").read() for n in ("sensei", "method")}
    check("build_plugin_agents.py regenerates cleanly", r.returncode == 0)
    check("committed plugin agents match the team sources (no drift)", before == after)

    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
