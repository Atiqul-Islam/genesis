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
  "output_dir": "<genesis>/team/<name>/   OR a build dir"
}
```
Method writes `persona.md`, `behavior.md`, `skills/`, writes acceptance tests FIRST, runs them, and returns:
```json
{ "agent_name":"<name>", "files":["..."], "tests":"<n> pass / <m> fail",
  "confidence":0.0, "gaps":"what's missing or assumed" }
```
- If Method asks a question you can answer from the requirements, answer it via SendMessage.
- If the answer is not in the requirements, take the question to the user.
- Do NOT accept a result with failing tests or unresolved gaps — send it back with a sharper spec.

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
python3 <genesis>/install/assemble.py <source_member_dir> <name> <target_repo> <genesis_home>
```
This writes `<target_repo>/.claude/agents/<name>.md`.

## Step 5 — Wire memory (only what the user confirmed)
- Register the memory server for the built agents, using exactly the memory setup the user specified.
- Do not choose a memory design yourself — it was confirmed in Step 1.

## Step 6 — Verify & deliver
- Confirm each installed agent's frontmatter is valid and its acceptance tests pass.
- Deliver to the user: the agents built, and which requirement each part satisfies.
- Report in ≤20-word bullets.
