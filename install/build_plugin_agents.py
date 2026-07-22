#!/usr/bin/env python3
"""Generate the PLUGIN-shipped agents `agents/sensei.md` and `agents/method.md` from the team sources.

These are a DIFFERENT artifact from what assemble.py writes. assemble.py builds USER-repo agents
(`<repo>/.claude/agents/<name>.md`) that MAY carry frontmatter `hooks` and point at a repo-local
`genesis-memory` MCP server. A PLUGIN-shipped agent CANNOT declare `hooks`, `mcpServers`, or
`permissionMode` in frontmatter (security-blocked — plugins-reference, verified 2026-07-22); its
enforcement + memory come from the PLUGIN level instead (hooks/hooks.json + the plugin-root .mcp.json).
So these files carry ONLY name/description/tools/skills, and their memory tools use the plugin-scoped
names `mcp__plugin_<plugin>_genesis-memory__{store,recall,consolidate}`.

Body composition is identical to assemble.py (persona + behavior + the expertise/memory notes) and reuses
assemble.py's own constants, so the plugin agents never drift from the team sources.

usage: build_plugin_agents.py [<repo_root>]     # default: this file's repo root
writes: <repo_root>/agents/{sensei,method}.md
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import assemble  # noqa: E402  reuse AGENTS metadata + the EXPERTISE/MEMORY notes + read() (single source)

# Which plugin skills each built-in preloads (dirs live at <repo_root>/skills/). Method ships no skills.
BUILTIN_SKILLS = {"sensei": ["build-agent", "research-expertise"], "method": []}


def plugin_name(repo_root):
    """The plugin name from .claude-plugin/plugin.json — used to build the scoped MCP tool prefix."""
    p = os.path.join(repo_root, ".claude-plugin", "plugin.json")
    return json.load(open(p, encoding="utf-8"))["name"]


def memory_tools(plugin):
    # Plugin .mcp.json server tools are exposed to agents as `mcp__plugin_<plugin>_<server>__<tool>`.
    prefix = f"mcp__plugin_{plugin}_genesis-memory__"
    return [prefix + t for t in ("store", "recall", "consolidate")]


def frontmatter(name, meta, skills, mem_tools):
    tools = list(meta["tools"]) + mem_tools
    skills_line = f"skills: {', '.join(skills)}\n" if skills else ""
    return (
        "---\n"
        f"name: {name}\n"
        f"description: {json.dumps(meta['description'], ensure_ascii=False)}\n"
        f"tools: {', '.join(tools)}\n"
        f"{skills_line}"
        "---\n")


def build_one(repo_root, name, mem_tools):
    meta = assemble.AGENTS[name]
    src = os.path.join(repo_root, "team", name)
    body = "\n\n".join([assemble.read(os.path.join(src, "persona.md")),
                        assemble.read(os.path.join(src, "behavior.md")),
                        assemble.EXPERTISE_NOTE, assemble.MEMORY_NOTE])
    skills = [s for s in BUILTIN_SKILLS[name]
              if os.path.isdir(os.path.join(repo_root, "skills", s))]
    out_dir = os.path.join(repo_root, "agents")
    os.makedirs(out_dir, exist_ok=True)
    out = os.path.join(out_dir, f"{name}.md")
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write(frontmatter(name, meta, skills, mem_tools) + "\n" + body + "\n")
    return {"agent": name, "written": out, "tools": list(meta["tools"]) + mem_tools, "skills": skills}


def main():
    repo_root = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.dirname(HERE)
    mem_tools = memory_tools(plugin_name(repo_root))
    out = [build_one(repo_root, name, mem_tools) for name in ("sensei", "method")]
    print(json.dumps({"plugin_root": repo_root, "agents": out}, indent=2))


if __name__ == "__main__":
    main()
