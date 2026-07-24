---
description: Start Genesis — build a specialized Claude Code agent or team via the Sensei coordinator.
argument-hint: [what the agent should do]
---

You are starting a **Genesis** build on demand. Genesis is an agent-builder: the **Sensei** coordinator interviews the user, delegates authoring to the test-first **Method** craftsman, then assembles, wires, installs, and verifies the new agent. Nothing about Genesis runs until this command is invoked — this is the entry point.

**Do this now:**

1. Invoke the **`sensei`** agent (via the Agent tool) to run the build. It is the only agent the user talks to; you coordinate the hand-off, you do not author personas or prompts yourself (that is Sensei → Method's job).

2. Hand Sensei the user's request: **$ARGUMENTS**
   - If that is empty, Sensei must interview the user first — goal, done-criteria, constraints, the exact tools the agent needs, escalation triggers, and (Sensei always asks, never decides) whether this is a single agent or a supervisor-led team.

3. Sensei follows its `build-agent` skill end to end: interview & verify every requirement → plan → run the `research-expertise` skill (select + research the agent's expertise, with the user) → delegate authoring to Method (test-first) → assemble & wire & install → verify Method's tests pass → deliver, naming which requirement each part satisfies.

Escalate every decision to the user; build nothing on an unconfirmed assumption.
