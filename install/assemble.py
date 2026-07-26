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
        "expertise": ["agent-building", "agentic-teams", "expertise-application"],
        # Sensei's skills now live at the genesis-home `skills/` dir (plugin-root skills/), not under team/.
        "skills": ["build-agent", "research-expertise"],
    },
    "method": {
        # No `Agent` tool -> cannot spawn/delegate. Boundary enforced by config, not prompt.
        "description": "Genesis craftsman - authors and tests each agent's persona, behavior, and skills. "
                       "Writes tests first; ships nothing untested; never orchestrates.",
        "tools": ["Read", "Write", "Edit", "Bash", "Glob", "Grep", "SendMessage"],
        "expertise": ["persona-creation", "prompt-engineering", "expertise-application"],
        "skills": [],
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


def register_required(gh, name, expertise):
    """Upsert this agent's REQUIRED expertise into expertise/required.json so the validate (Stop) hook
    enforces the APPLIED-EXPERTISE declaration for it — identical machinery to sensei/method. Preserves
    every other agent's entry and the _doc note."""
    path = os.path.join(gh, "expertise", "required.json")
    try:
        data = json.load(open(path, encoding="utf-8"))
    except Exception:
        data = {"_doc": "Per-agent REQUIRED expertise (auto-registered by assemble.py); the validate Stop "
                        "hook blocks finishing until each is declared via APPLIED-EXPERTISE."}
    data[name] = list(expertise)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
        f.write("\n")


def _copy_skill(skill_dir, dest_root):
    """Copy one skill dir (must contain SKILL.md) into dest_root; return its name or None."""
    if not (os.path.isdir(skill_dir) and os.path.exists(os.path.join(skill_dir, "SKILL.md"))):
        return None
    dest = os.path.join(dest_root, os.path.basename(skill_dir))
    if os.path.exists(dest):
        shutil.rmtree(dest)
    shutil.copytree(skill_dir, dest)
    return os.path.basename(skill_dir)


def install_skills(src, target):
    """BUILT agents: copy each skill dir under <src>/skills/ into the repo's .claude/skills/ so Claude Code
    discovers them, and return their names for the agent's frontmatter `skills:` field (preloads them)."""
    src_skills = os.path.join(src, "skills")
    if not os.path.isdir(src_skills):
        return []
    dest_root = os.path.join(target, ".claude", "skills")
    os.makedirs(dest_root, exist_ok=True)
    names = []
    for entry in sorted(os.listdir(src_skills)):
        n = _copy_skill(os.path.join(src_skills, entry), dest_root)
        if n:
            names.append(n)
    return names


def install_named_skills(gh, names, target):
    """BUILT-IN agents (sensei/method): their skills live at the genesis-home `skills/` dir (the plugin-root
    skills/), not under team/<name>/skills/. Copy the NAMED skills into the repo's .claude/skills/ and return
    the ones actually found (so a missing skill dir never fabricates a frontmatter reference)."""
    if not names:
        return []
    src_root = os.path.join(gh, "skills")
    dest_root = os.path.join(target, ".claude", "skills")
    os.makedirs(dest_root, exist_ok=True)
    out = []
    for name in names:
        n = _copy_skill(os.path.join(src_root, name), dest_root)
        if n:
            out.append(n)
    return out


def _yaml_cmd(*parts):
    """Build a cross-platform hook `command:` YAML line from already-native path parts.

    Portability (Windows + POSIX): each path is double-quoted so paths with spaces survive, and NATIVE
    separators are kept (this assembler runs on the same machine the hooks will run on, so os.sep is right —
    backslashes on Windows, forward on POSIX). The whole value is a YAML SINGLE-quoted scalar, which keeps
    backslashes and inner double-quotes literal (only `'` doubles). So a Windows path like
    C:\\Users\\Jane Doe\\genesis\\hooks\\gate.py round-trips intact and the shell still sees it quoted."""
    shell = " ".join(parts)
    return "          command: '" + shell.replace("'", "''") + "'\n"


def _q(p):
    """Double-quote a path arg for the generated shell command (handles spaces; native separators kept)."""
    return '"' + p + '"'


def frontmatter(name, meta, home, skills):
    # PORTABLE hook paths (cross-platform): reference the repo's own .genesis via $CLAUDE_PROJECT_DIR, which
    # Claude Code substitutes at runtime to the project root (Windows + macOS + Linux) — NEVER an absolute
    # machine path. So a BUILT agent survives a clone to any machine/OS. `home` is e.g.
    # "$CLAUDE_PROJECT_DIR/.genesis". Interpreter is `python3` (resolved via PATH), matching the plugin's
    # own hooks.json rather than baking this machine's sys.executable.
    hooks_dir = home + "/hooks"
    exp_dir = home + "/expertise"
    py = "python3"
    inject = _q(py) + " " + _q(hooks_dir + "/inject.py") + " " + _q(exp_dir) + " " + name
    gate = _q(py) + " " + _q(hooks_dir + "/gate.py")
    stop = _q(py) + " " + _q(hooks_dir + "/validate.py") + " . " + name
    review = _q(py) + " " + _q(hooks_dir + "/review.py") + " . " + name
    skills_line = f"skills: {', '.join(skills)}\n" if skills else ""
    # PreToolUse: every agent gets the Write|Edit gate (house rules + rule surfacing). SENSEI additionally
    # gets a Bash gate that blocks assembling a built agent unless the research-expertise skill ran this
    # session — only Sensei orchestrates/assembles, so it is wired Sensei-scoped and no other agent changes.
    pretooluse = (
        "  PreToolUse:\n"
        '    - matcher: "Write|Edit"\n'
        "      hooks:\n"
        "        - type: command\n"
        + _yaml_cmd(gate))
    if name == "sensei":
        enforce = _q(py) + " " + _q(hooks_dir + "/enforce_research.py")
        pretooluse += (
            '    - matcher: "Bash"\n'
            "      hooks:\n"
            "        - type: command\n"
            + _yaml_cmd(enforce))
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
        + _yaml_cmd(inject)
        + pretooluse +
        "  Stop:\n"
        "    - hooks:\n"
        "        - type: command\n"
        + _yaml_cmd(stop) +
        "        - type: command\n"
        + _yaml_cmd(review) +
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
    # Parity with sensei/method: register this agent's required expertise so the Stop hook enforces the
    # declaration. Built agents supply it via meta.json ("required_expertise" or "expertise"); built-ins
    # via the AGENTS table. No expertise assigned -> nothing to enforce (declaration check is a no-op).
    expertise = meta.get("expertise") or meta.get("required_expertise") or []
    register_required(gh, name, expertise)
    # Copy this agent's skills into <target>/.claude/skills/ and collect their names for the frontmatter.
    # Built-in team members (sensei/method) source their skills from the genesis-home `skills/` dir (the
    # plugin-root skills/); a BUILT agent supplies its own under its source dir's skills/.
    if name in AGENTS:
        skills = install_named_skills(gh, AGENTS[name].get("skills", []), target)
    else:
        skills = install_skills(src, target)
    body = "\n\n".join([read(os.path.join(src, "persona.md")),
                        read(os.path.join(src, "behavior.md")), EXPERTISE_NOTE, MEMORY_NOTE])
    out_dir = os.path.join(target, ".claude", "agents")
    os.makedirs(out_dir, exist_ok=True)
    out = os.path.join(out_dir, f"{name}.md")
    # Portable hook base: express genesis_home RELATIVE to the target repo via $CLAUDE_PROJECT_DIR so the
    # written agent has NO absolute machine path (works on any machine/OS after the repo's .genesis exists).
    # Normal case: gh = <target>/.genesis -> "$CLAUDE_PROJECT_DIR/.genesis". If gh is somehow outside the
    # repo, fall back to the (non-portable) absolute path rather than emit a broken reference.
    rel = os.path.relpath(gh, target).replace(os.sep, "/")
    home = ("$CLAUDE_PROJECT_DIR/" + rel) if not rel.startswith("..") else gh.replace(os.sep, "/")
    with open(out, "w", encoding="utf-8") as f:
        f.write(frontmatter(name, meta, home, skills) + "\n" + body + "\n")
    print(json.dumps({"agent": name, "written": out, "tools": meta["tools"],
                      "skills_installed": skills, "body_lines": body.count("\n") + 1}))


if __name__ == "__main__":
    main()
