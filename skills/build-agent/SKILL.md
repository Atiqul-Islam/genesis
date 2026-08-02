---
name: build-agent
description: The exact procedure Sensei follows to build an agent or team — interview, delegate authoring to Method, assemble, install, verify. Use whenever the user asks Genesis to build an agent or team.
---

# Building an agent or team

This is your mechanizable build procedure. Follow every step; skip none. Consult your agentic-teams and
agent-building expertise for the judgment calls. You (Sensei) never author persona/prompt content — Method does.

## Step 1 — Interview & verify
- Gather the full spec: goal, done-criteria, constraints, tools the agent needs, escalation triggers.
- Restate every requirement back to the user. Proceed only on confirmed facts.
- Ask the user: single agent, or a supervisor-led team? The user decides.
- Resolve every contradiction with the user before proceeding.

## Step 2 — Plan
- Decompose the goal into the smallest set of agents. Name each, its scope, its tools, its boundaries.
- Prefer a single agent unless the task genuinely needs a team (single-agent-first).
- State the plan back to the user for confirmation.

## Step 2b — Offer session-copy (SINGLE-agent builds only)
For a single custom agent, ASK the user: **a fresh custom agent, or copy your current session into it?**
(The user chooses; never assume.) If they choose **copy**, the agent is built as a portable copy of the user's
current Claude Code session — carrying its full conversation history + all memory/context — then specialized.
- Requires the session-pointer hook wired in the repo (records the live session id) so `--session current`
  resolves; else ask the user for their session id.
- Run the orchestrator (captures ALL stores, scrubbed, → portable history + summary, and embeds the history
  into the repo's shared memory under `agent_id=<name>`):
  ```
  node <genesis>/session_copy/build_session_agent.js --session current --name <name> --repo <target_repo> \
      --genesis-home <target_repo>/.genesis --server-bin <...>/genesis-memory-server \
      --model-dir <...>/models --memory-db <target_repo>/.genesis/memory.db
  ```
- Then continue the NORMAL build (Step 3): Method authors the agent's specialized persona (the "custom" part);
  the assembler wires it. At runtime the agent recalls its carried-over history via its memory tools and loads
  its `summary.md` digest at start (inject.py). The bundle lives at `<target_repo>/.genesis/agents/<name>/` and
  travels with a git clone (portable — no `~/.claude` dependency).
- Credentials are never copied; pass any known secret values via `--known-secret` for guaranteed removal.
- SCOPE NOTE: session-copy is single-agent only; the result is a separate named agent (not auto-mounted).

## Step 3 — Delegate authoring to Method (one TASK-SPEC per agent)
Spawn Method with the Agent tool, then use SendMessage to hand it this TASK-SPEC:
```json
{
  "agent_name": "<name>",
  "objective": "<one concrete sentence: what this agent is for>",
  "responsibilities": ["..."],
  "boundaries": ["what it must never do"],
  "tools": ["the exact tool list"],
  "voice": "<how it should respond>",
  "acceptance_criteria": ["checkable behaviors the agent MUST pass"],
  "required_expertise": ["expertise names this agent MUST apply — parity with sensei/method"],
  "output_dir": "<genesis>/team/<name>/   OR a build dir"
}
```
Method writes `persona.md`, `behavior.md`, `skills/`, a `meta.json` (`description`, `tools`,
`required_expertise`), writes acceptance tests FIRST, runs them, and returns:
```json
{ "agent_name":"<name>", "files":["..."], "tests":"<n> pass / <m> fail",
  "confidence":0.0, "gaps":"what's missing or assumed" }
```
- If Method asks a question you can answer from the requirements, answer it via SendMessage.
- If the answer is not in the requirements, take the question to the user.
- Do NOT accept a result with failing tests or unresolved gaps — send it back with a sharper spec.

## Step 3a — Expertise: run the `research-expertise` skill (never decide it alone)
- **Invoke the `research-expertise` skill** to select and (when the user asks) deeply research this agent's
  expertise. That skill IS the process — pick → suggest & discuss with the user → ask whether to research →
  confirm scope + documents → propose a method the user verifies → research in parallel and author a full
  enforceable module in the repo's `.genesis/` store. Never choose expertise silently, and this is enforced:
  the assembler refuses to build a non-builtin agent unless that skill ran this session.
- The user-confirmed expertise names become each agent's TASK-SPEC `required_expertise`. Method writes them
  into the agent's `meta.json`; the assembler auto-registers them in `.genesis/expertise/required.json` and
  wires the Stop hook that blocks finishing until the agent DECLARES it applied each
  (`APPLIED-EXPERTISE: <name>#<rules>`) — identical machinery to sensei/method.
- `expertise-application` is always included. An agent cannot be enforced to apply expertise that does not
  exist, so any new/deepened expertise is researched and authored (via the skill) BEFORE assembly.

## Step 3b — Error, retry & escalation policy (from your teams expertise)
- Treat every Method result as UNTRUSTED until you check it against the TASK-SPEC's acceptance criteria.
- Method returns garbage / off-schema / failing tests / low confidence → do NOT accept. Send it back once
  with a sharper spec. If it fails again, stop and escalate to the user with the specific gap.
- Bound the work: max 2 re-delegations per agent; if unresolved, escalate rather than loop.
- Method times out or goes silent → cancel, mark the sub-build failed, and escalate to the user.
- Escalate to the user on: ambiguous requirements, a contradiction you cannot resolve from the spec,
  repeated failure, or any irreversible/high-impact action. State the assumption; never act on a guess.

## Step 4 — Assemble & install
For each agent whose tests pass, run the assembler (it enforces tool-level boundaries + wires the hooks):
```
node <genesis>/install/assemble.js <source_member_dir> <name> <target_repo> <genesis_home>
```
This writes `<target_repo>/.claude/agents/<name>.md`.

## Step 5 — Wire memory (only what the user confirmed)
- Register the memory server for the built agents, using exactly the memory setup the user specified.
- Do not choose a memory design yourself — it was confirmed in Step 1.

## Step 6 — Verify & deliver
- Confirm each installed agent's frontmatter is valid and its acceptance tests pass.
- Deliver to the user: the agents built, and which requirement each part satisfies.
- Report in ≤20-word bullets.
