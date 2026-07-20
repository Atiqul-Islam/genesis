# Genesis — Expertise-Application Enforcement Plan

**Goal.** Make every Genesis agent *provably apply* its assigned expertise, not just hold it — closing the
"presence ≠ application" gap with deterministic hooks + front-loaded prompting, at near-zero token cost.

**Grounded in** (expertise applied): `expertise/expertise-application.md` (§3 design rules, §7 delta, C1/C2/C3),
`agent-building` (§16 deterministic expertise = inject/gate/validate), `prompt-engineering` (scoped/positive/ID'd/XML),
`persona-creation` (behavior.md structure, ≤200-line budget, positive framing).

**Design decisions locked with Atiqul:**
- Enforcement strictness **(b)**: declaration present **AND** re-verify the checkable rules actually hold.
- Checklist-first: every task starts by loading + declaring the expertise (prevention > correction).
- Deterministic hooks only (skip the LLM-reviewer for now — token cost); Method's semantic review stays for judgment.
- Fail-closed; loop-guard + escalate; tight per-task scoping; cache-stable expertise prefix.

---

## Phase 0 — Turn expertise into enforceable rules  *(prompt-engineering + persona)*
- **0.1** Rewrite each expertise file's guidance into **scoped, positively-framed, ID'd imperative rules** —
  fixes Opus 4.x literalism ("apply to every X," never "apply generally").
- **0.2** Add a machine-readable `rules.json` per expertise: `{id, text, scope, type: checkable|judgment, predicate?}`.
- **0.3** For each **checkable** rule, write its predicate (regex / line-count / AST / banned-string) — the hook reads these.
- Output: every expertise has a human doc + a rule manifest the hooks and agents share.

## Phase 1 — Per-agent required-expertise map  *(agent-building)*
- **1.1** Define required expertise per agent: Sensei → {agent-building, agentic-teams, expertise-application};
  Method → {persona-creation, prompt-engineering, expertise-application}.
- **1.2** Store as config (`expertise/required.json`) that inject.py, validate.py, and the assembler all read.
- Keep it **tight** — only expertise relevant to the agent's job (token mitigation #5).

## Phase 2 — Behavior: the task checklist  *(expertise-application §3, front-load mitigation)*
- **2.1** Prepend every task in Sensei/Method `behavior.md` with a fixed checklist:
  1. Load required expertise. 2. Declare which rules apply. 3. Reason using them. … N. Declare rules applied.
- **2.2** Declaration format the hook can parse: `APPLIED-EXPERTISE: <name>#<rule-ids>` (one line, machine-checkable).
- Front-loading = the #1 token saver: declared upfront → Stop hook passes first time → no redo.

## Phase 3 — Hooks: the enforcement  *(expertise-application §7, C2; agent-building §16)*
- **3.1 `validate.py` (Stop) — the gate.** Check: (a) declaration names **all** required expertise, AND
  (b) every **checkable** rule's predicate holds. Fail → `block` + a **minimal-correction reason** (what/where/what-to-do;
  "just add the declaration" when work is fine). Loop-guard (`stop_hook_active`) + **cap N retries → escalate to human**.
- **3.2 `gate.py` (PreToolUse Write|Edit) — during-work checks.** Keep blocking violations immediately (localized fix,
  not full redo). Add: surface the relevant rule text on block (**tail re-assertion** before the risky step).
- **3.3 `inject.py` (SessionStart) — presence + requirement.** Inject the required-expertise map + the "declare it" rule.
- **3.4 (optional) `UserPromptSubmit`** — re-assert top rules per turn on long tasks (tail re-assert; enable only if needed).
- **3.5 Fail-closed + logging.** Every hook blocks on its own error (never silently allow); log every allow/block+reason
  to `.genesis/hook-decisions.log` for audit.

## Phase 4 — Assembler wiring  *(agent-building)*
- **4.1** `install/assemble.py`: wire validate/gate/inject into each agent's frontmatter (valid YAML, already proven).
- **4.2** Inject the per-agent required-expertise map so the hooks know what to enforce for that agent.

## Phase 5 — Prove enforcement (not assume it)  *(expertise-application measurement; "test everything")*
- **5.1 Unit-test each hook:** passes clean output; blocks missing declaration; blocks a declared-but-violated
  checkable rule; **fails closed** on a crash; loop-guard stops after N.
- **5.2 Live end-to-end:** an agent that skips the declaration → blocked → redoes → passes. The real proof.
- **5.3 Adherence harness:** per-rule adherence rate on a fixed task set (deterministic checks + a multi-turn slice).

---

## Token-cost accounting (mitigations mapped)
| Mitigation | Where in plan | Effect |
|---|---|---|
| Front-load declaration | Phase 2.1 | Stop hook passes first time → **no redo** |
| Check during (localized) | Phase 3.2 | Fix one file, not the whole task |
| Minimal-correction reason | Phase 3.1 | One-line fix, not a rebuild |
| Cap retries + escalate | Phase 3.1 | No runaway burn |
| Tight per-task scope | Phase 1.2 | Fewer misses → fewer redos |
| Cache-stable expertise prefix | Phase 3.3 | Re-reads cost ~10% |
| Hooks are Python (free) | Phase 3 | Enforcement itself = ~0 tokens |

**Net:** near-zero tokens when prevention works; one small, localized redo when it doesn't. The Stop hook is the
safety net, not the main mechanism.

## Honest limits (carried from the research)
- Only **checkable** rules are guaranteed; pure-**judgment** rules are *raised*, not forced (that's the wall, not a gap).
- Stop hook catches at the end → forces a redo; the checklist (Phase 2) is what prevents most redos.
- Skipping the LLM-reviewer means judgment rules rely on prompting + Method's existing review, not an independent gate.

## Build order
Phase 0 → 1 (data the hooks need) → 2 (behavior) → 3 (hooks) → 4 (wiring) → 5 (proof). Each phase testable before the next.
