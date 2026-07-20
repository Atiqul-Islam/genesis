#!/usr/bin/env python3
"""Assemble a member's clean source files into the `.claude/agents/<name>.md` the runtime needs.

Applies the deterministic layer (§16):
  * BOUNDARIES enforced at the TOOL layer (frontmatter `tools`) — e.g. Method has no `Agent` tool, so it
    physically cannot spawn/delegate. A real gate, not a prompt wish.
  * EXPERTISE + house rules delivered and enforced by the inject/gate/validate HOOK triple, wired here into
    each agent's frontmatter (verified YAML shape: event -> matcher -> hooks:[{type,command}]).

usage: assemble.py <source_member_dir> <name> <target_repo> <genesis_home>
reads:  <source_member_dir>/{persona.md, behavior.md, skills/}
writes: <target_repo>/.claude/agents/<name>.md
"""
import json, os, shutil, sys

AGENTS = {
    "sensei": {
        "description": "Genesis coordinator - the user talks to Sensei; it verifies requirements, plans, "
                       "delegates authoring to Method, then assembles, wires, installs, and delivers.",
        "tools": ["Read", "Write", "Edit", "Bash", "Glob", "Grep", "Agent", "SendMessage"],
    },
    "method": {
        # No `Agent` tool -> cannot spawn/delegate. Boundary enforced by config, not prompt.
        "description": "Genesis craftsman - authors and tests each agent's persona, behavior, and skills. "
                       "Writes tests first; ships nothing untested; never orchestrates.",
        "tools": ["Read", "Write", "Edit", "Bash", "Glob", "Grep", "SendMessage"],
    },
}

EXPERTISE_NOTE = (
    "## Your expertise\n"
    "- A SessionStart hook injects the house rules and pointers to your decoupled, versioned expertise store.\n"
    "- Read the expertise file your behavior names, on demand, before deep work. It is authoritative.\n"
    "- The hard, checkable rules are also enforced by gate/validate hooks — you cannot violate them.")

# Per-agent semantic memory via the Genesis MCP memory server (registered as `genesis-memory`).
MEMORY_TOOLS = ["mcp__genesis-memory__store", "mcp__genesis-memory__recall", "mcp__genesis-memory__consolidate"]
MEMORY_NOTE = (
    "## Your memory (per-agent, durable across sessions)\n"
    "- The `genesis-memory` MCP server gives you your own semantic memory: `store`, `recall`, `consolidate`.\n"
    "- ALWAYS pass your own agent name as `agent_id` — the store is scoped by it, so you only see your own memories.\n"
    "- `store` a durable fact/decision; `recall` before deep work to retrieve what you learned before; "
    "`consolidate` to dedup. This is separate from the transient session context.")


def read(p):
    with open(p, encoding="utf-8") as f:
        return f.read().rstrip()


def install_skills(src, target):
    """Copy each skill dir under <src>/skills/ into the repo's .claude/skills/ so Claude Code discovers
    them, and return their names to list in the agent's frontmatter `skills:` field (preloads them)."""
    src_skills = os.path.join(src, "skills")
    names = []
    if not os.path.isdir(src_skills):
        return names
    dest_root = os.path.join(target, ".claude", "skills")
    os.makedirs(dest_root, exist_ok=True)
    for entry in sorted(os.listdir(src_skills)):
        d = os.path.join(src_skills, entry)
        if os.path.isdir(d) and os.path.exists(os.path.join(d, "SKILL.md")):
            dest = os.path.join(dest_root, entry)
            if os.path.exists(dest):
                shutil.rmtree(dest)
            shutil.copytree(d, dest)
            names.append(entry)
    return names


def frontmatter(name, meta, gh, skills):
    hooks_dir = os.path.join(gh, "hooks")
    exp_dir = os.path.join(gh, "expertise")
    py = "python3"
    skills_line = f"skills: {', '.join(skills)}\n" if skills else ""
    # Verified frontmatter-hooks YAML shape (docs db-reader example): event -> [{matcher, hooks:[{type,command}]}].
    return (
        "---\n"
        f"name: {name}\n"
        f"description: {json.dumps(meta['description'], ensure_ascii=False)}\n"
        f"tools: {', '.join(meta['tools'])}\n"
        f"{skills_line}"
        "hooks:\n"
        "  SessionStart:\n"
        '    - matcher: "startup|resume|compact"\n'
        "      hooks:\n"
        "        - type: command\n"
        f'          command: "{py} {hooks_dir}/inject.py {exp_dir} {name}"\n'
        "  PreToolUse:\n"
        '    - matcher: "Write|Edit"\n'
        "      hooks:\n"
        "        - type: command\n"
        f'          command: "{py} {hooks_dir}/gate.py"\n'
        "  Stop:\n"
        "    - hooks:\n"
        "        - type: command\n"
        f'          command: "{py} {hooks_dir}/validate.py . {name}"\n'
        "---\n")


def main():
    src, name, target, gh = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
    # Genesis's own team (sensei/method) has built-in metadata; a BUILT agent supplies its own via meta.json
    # ({"description":..., "tools":[...]}) written alongside its persona.md/behavior.md.
    meta = AGENTS.get(name)
    if not meta:
        mp = os.path.join(src, "meta.json")
        if not os.path.exists(mp):
            sys.exit(f"no metadata for agent '{name}' — provide {mp} with 'description' and 'tools'.")
        meta = json.load(open(mp, encoding="utf-8"))
        if "description" not in meta or "tools" not in meta:
            sys.exit(f"{mp} must contain 'description' and 'tools'.")
    # Every assembled agent gets per-agent memory tools (design: each agent has its own vector memory).
    meta = {**meta, "tools": list(meta["tools"]) + MEMORY_TOOLS}
    skills = install_skills(src, target)
    body = "\n\n".join([read(os.path.join(src, "persona.md")),
                        read(os.path.join(src, "behavior.md")), EXPERTISE_NOTE, MEMORY_NOTE])
    out_dir = os.path.join(target, ".claude", "agents")
    os.makedirs(out_dir, exist_ok=True)
    out = os.path.join(out_dir, f"{name}.md")
    with open(out, "w", encoding="utf-8") as f:
        f.write(frontmatter(name, meta, gh, skills) + "\n" + body + "\n")
    print(json.dumps({"agent": name, "written": out, "tools": meta["tools"],
                      "skills_installed": skills, "body_lines": body.count("\n") + 1}))


if __name__ == "__main__":
    main()
