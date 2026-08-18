# Reducing Token Usage in Large Multi-Agent Claude Code Workflows (Claude Max)

**A definitive, evidence-backed guide.** Written 2026-07-17. Every quota/cache number is tagged
`[VERIFIED]` (with a primary source) or `[INFERRED]` / `[ESTIMATE]` (reasoning, labelled). The full
source ledger is the appendix at the bottom.

**The concrete case that triggered this.** A workflow fanned out **69 review agents + 8 team-lead
agents (77 agents)** over a large code portfolio. It burned **~3.6M subagent tokens**, hit the **Max
session limit** partway through, and **52 of 69 review agents failed** until the rolling window reset
(~04:50). Recovery: finished reports were made to **replay from a passed-in `done` map at zero token
cost**, so the re-run paid only for the 52 that were missing. This guide generalises that fix into the
full theory.

---

## Executive summary — the 5 highest-impact levers

Ranked by token saved per unit of effort, for a fan-out of this size on a Max plan.

| # | Lever | Mechanism | Est. saving on a run like ours | Quality risk |
|---|-------|-----------|-------------------------------|--------------|
| **1** | **Idempotent resume / `done`-map replay** | A finished unit returns its cached result with **no agent spawned**. Re-runs after a limit-stop or crash pay only for what's missing. | The exact difference between a **full re-run and a resume**. Our recovery re-ran 52/69 instead of 69/69 → **~25% avoided on the first retry, ~100% on any later no-op re-run.** | **None.** Pure win; the output is byte-identical (it *is* the prior output). |
| **2** | **Pre-compute everything deterministic and pass it in as a brief** | LOC (`cloc`), branch list, git ahead/behind, file trees, agent surfaces are computed **once by a script**, not re-discovered by 69 agents. Each agent reads a ~600-token brief instead of running dozens of tool calls. | **~30–50%** of each agent's input + the elimination of **~10–40 tool round-trips per agent**, whose verbose raw output never enters any context. | **Low.** Risk is a stale/wrong brief; mitigated by generating briefs fresh each run. |
| **3** | **Cache the large stable prefix; vary only a small tail** | Claude Code runs a **1-hour prompt cache** `[VERIFIED]`. Identical system prompt + tools + rules across a fan-out are written once and **read at 0.1× price** by every later agent. | **~50–70%** of the *shared-prefix* input tokens across the fan-out (illustrative math below). | **None** if prompts are structured correctly; the risk is *accidentally* busting cache with a varying prefix. |
| **4** | **Model + effort tiering per phase** | Opus only where deep reasoning pays (synthesis, planning); Sonnet for mechanical review; Haiku for the long tail of tiny repos. Opus "costs several times more per turn than Sonnet, and Sonnet more than Haiku." `[VERIFIED]` | **2–5× on every stage moved down a tier.** The long tail of ~30 sub-1k-LOC repos on Haiku instead of Opus is a large fraction of the fleet. | **Medium.** Wrong tier degrades output; mitigate by tiering on a measured proxy (LOC/complexity), not a guess. |
| **5** | **Right-size the fan-out to the session limit** | Don't launch 77 agents into a window that can't hold 3.6M tokens. Batch M small repos per agent, run in waves, and keep the whole run inside one rolling window — or accept resume (lever 1) as the safety net. | Prevents the **52-agent failure + retry** entirely; the retry itself is pure waste. | **Low.** Batching too aggressively can blur per-repo focus; keep batches small and homogeneous. |

**One sentence:** *pre-compute the facts, cache the boilerplate, right-size and tier the agents, and
make every run resumable* — and a 3.6M-token run becomes a fraction of that, with the session-limit
failure designed out rather than retried around.

---

## 1. How the Claude Max subscription's limits actually work

### 1.1 Two independent limits: a 5-hour rolling window and a weekly cap

`[VERIFIED]` **All plans reset on a rolling 5-hour session window**, and a **weekly cap** sits on top of
it. Anthropic's Claude Code limits article states plans "reset every 5 hours with exact countdown timing
displayed" and that "weekly limits are now active alongside the existing 5-hour cycles." When you run
out you get a **"limit reached, resets at *time*" message**
([support 14552983](https://support.claude.com/en/articles/14552983-models-usage-and-limits-in-claude-code), updated April 15 2026).

- The 5-hour window is **rolling**: it opens when you first use Claude in a window and the reset clock is
  computed from that start. `[VERIFIED, mechanism]` The UI shows "how much of your plan's five-hour
  session limit you've used thus far, plus the amount of time remaining in the session"
  ([support 9797557](https://support.claude.com/en/articles/9797557-usage-limit-best-practices), June 2 2026).
  → Our run's ~04:50 recovery is consistent with the window that opened ~5 hours earlier resetting.
- The weekly cap was **announced July 28 2025, effective August 28 2025** `[VERIFIED, reported]`
  ([TechCrunch, 2025-07-28](https://techcrunch.com/2025/07/28/anthropic-unveils-new-rate-limits-to-curb-claude-code-power-users/);
  [Anthropic on X, 2025-07-28](https://x.com/AnthropicAI/status/1949898502688903593) — "new weekly rate
  limits for Claude Pro and Max in late August … less than 5% of subscribers").

### 1.2 There are actually TWO weekly limits: Opus-only and all-other-models

`[VERIFIED]` The usage UI shows weekly reset times **"for Opus only and all other models"**
([support 9797557](https://support.claude.com/en/articles/9797557-usage-limit-best-practices)). So Opus
has its own separate weekly budget that drains independently of Sonnet/Haiku. **Implication for
fan-out:** a 69-agent Opus review can exhaust the *Opus weekly* budget while leaving the all-models
budget almost untouched — moving mechanical agents to Sonnet/Haiku (lever 4) protects the scarce Opus
pool directly.

### 1.3 Rough per-tier numbers (reported, not on Anthropic's current pages)

Anthropic's current help pages deliberately **do not publish raw hour numbers** (they vary and are
described as "40–80 hours"-style ranges). The figures below are `[VERIFIED, reported]` from Anthropic's
July 2025 announcement as covered by TechCrunch — treat as order-of-magnitude, not a contract:

| Plan | Sonnet / week (reported) | Opus / week (reported) |
|------|--------------------------|------------------------|
| Pro ($20) | 40–80 h | — |
| Max 5× ($100) | 140–280 h | 15–35 h |
| Max 20× ($200) | 240–480 h | 24–40 h |

Source: [TechCrunch 2025-07-28](https://techcrunch.com/2025/07/28/anthropic-unveils-new-rate-limits-to-curb-claude-code-power-users/).
**Note the Opus row is small** — 15–40 h/week — which is exactly why a big Opus fan-out is the thing
most likely to hit a wall. `[VERIFIED, primary]` Anthropic's pricing page corroborates the multiplier
framing directly: the Max/premium seat gives **"5x more usage than standard seats"**
([claude.com/pricing](https://www.claude.com/pricing)) — but it, too, does not publish raw hour numbers.

**May 6 2026 change** `[VERIFIED]`: Anthropic **doubled Claude Code's five-hour rate limits** for Pro,
Max, Team and seat-based Enterprise, and **removed the peak-hours reduction**
([anthropic.com/news/higher-limits-spacex](https://www.anthropic.com/news/higher-limits-spacex)). So the
5-hour ceiling today is ~2× what it was when the weekly caps launched.

### 1.4 Subagent / workflow tokens count against the SAME pool as the main loop

This is the crux for multi-agent work, and it's **verified three independent ways**:

1. `[VERIFIED]` "Your usage of all different Claude product surfaces (claude.ai, Claude Code, Claude
   Desktop) **counts towards the same usage limit**"
   ([support 11647753](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work)).
2. `[VERIFIED]` Claude Code's `/usage` "attributes recent usage to **skills, subagents, plugins, and
   individual MCP servers**, with each shown as a percentage of the total" against your plan limits
   ([code.claude.com/docs/costs](https://code.claude.com/docs/en/costs)).
3. `[VERIFIED, first-party]` The Workflow runtime's own budget accounting: `budget.spent()` returns
   "output tokens spent this turn **across the main loop and all workflows — the pool is shared, not
   per-workflow**" (Claude Code Workflow tool contract, this session).

**Consequence:** 69 subagents don't get 69 private budgets. Every subagent token is a main-pool token.
A fan-out is a *multiplier* on your session and weekly consumption, and "each Claude Code turn carries
file contents, tool calls, and multi-step reasoning, so one debugging session can consume more than a
day of chat" `[VERIFIED]` ([code.claude.com/docs/costs](https://code.claude.com/docs/en/costs)).

### 1.5 What happens on overage — including the cache-TTL drop

- `[VERIFIED]` On hitting a limit you **wait for reset, upgrade, or purchase usage credits**
  ([support 11647753](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work)).
  In Claude Code you can also **switch to a lighter model** (`/model`) or fall back to an API key if your
  org allows ([support 14552983](https://support.claude.com/en/articles/14552983-models-usage-and-limits-in-claude-code)).
- `[VERIFIED, first-party]` **Prompt-cache TTL drops from 1h to 5m in overage.** The Claude Code runtime
  states: "This session's requests use a 1-hour Anthropic prompt-cache TTL … **If the session enters
  usage overage, later requests drop to the 5-minute TTL**" (ScheduleWakeup tool contract, this session).
  This matters for fan-outs: once you're in overage the shared prefix must be **re-written every 5
  minutes** instead of surviving an hour, so a slow, spread-out fan-out that previously rode one cache
  write now pays repeated 1.25× writes — the run gets *more* expensive exactly when you can least afford
  it. **Lever:** finish fan-outs *inside* the cache window; don't let a throttled run dribble across it.
- `[VERIFIED]` `/usage` degrades gracefully when the usage endpoint is itself rate-limited: it "shows the
  last usage bars it loaded on this machine within the past 60 minutes" with a *Showing last-known usage*
  note ([code.claude.com/docs/costs](https://code.claude.com/docs/en/costs)). So the bars you see mid-run
  can be up to an hour stale — don't treat them as real-time during a heavy fan-out.

### 1.6 How the "resets at *time*" is computed

`[INFERRED, from verified pieces]` The message is the **end of the currently-open rolling window**: window
opens at first use → the "resets at" time is that start + 5h (for the session limit) or the 7-day weekly
anchor for the weekly limit. Anthropic exposes the countdown in-product but does not publish the exact
arithmetic; the "rolling window, resets at start+duration" model is consistent with every verified
statement above and with our observed ~04:50 reset.

---

## 2. Prompt caching — the single biggest lever you don't control explicitly

### 2.1 The mechanics, verified

`[VERIFIED]` from [platform.claude.com prompt-caching docs](https://platform.claude.com/docs/en/build-with-claude/prompt-caching):

- **TTLs:** default **5-minute**; optional **1-hour** via `"cache_control": {"type":"ephemeral","ttl":"1h"}`.
  The TTL **resets on each successful cache hit** (so an actively-used prefix stays warm).
- **Prices** (relative to base input tokens): **5-min write = 1.25×** ("25% more than base input
  tokens"), **1-hour write = 2×**, **read = 0.10×** (a 90% discount). Anthropic's own pricing page
  confirms the multipliers numerically — e.g. Haiku 4.5: input **$1/MTok**, prompt-cache **write
  $1.25/MTok** (=1.25×), **read $0.10/MTok** (=0.10×), "reflects 5-minute TTL"
  ([claude.com/pricing](https://www.claude.com/pricing)).
- **Prefix model:** the cache references the **full prefix — `tools`, then `system`, then `messages`,
  in that order** — up to and including the block marked with `cache_control`. The hash is cumulative:
  **changing any block at or before the breakpoint changes the hash and misses the cache.**
- **Breakpoints:** up to **4** explicit `cache_control` breakpoints; the system does a **20-block
  lookback** to find the longest already-cached prefix.
- **Minimum cacheable length:** `[VERIFIED]` "Shorter prompts cannot be cached, even if marked with
  `cache_control`" and are silently processed uncached. `[VERIFIED, reported]` the per-model minimum is
  **~1,024 tokens for current Opus/Sonnet (Opus 4.8, Sonnet 5), ~2,048 for older Haiku, up to ~4,096 for
  some newer variants** ([Anthropic docs, per-model minimum table](https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching)).
  Verify with the response fields (below); if both are 0 you're under the minimum.
- **Accounting fields:** `cache_creation_input_tokens` (tokens written) and `cache_read_input_tokens`
  (tokens read at 0.1×). "If both … are 0, the prompt was not cached." Claude Code's `/usage` surfaces
  these — e.g. `940.0k cache read, 50.0k cache write` in the sample session block.
- **Isolation:** `[VERIFIED]` "As of February 5, 2026, prompt caching uses **workspace-level
  isolation**." All agents in one Claude Code workspace/session share the same cache namespace — which is
  *why* identical fan-out prefixes can share a cache at all.

### 2.2 How identical fan-out prompts share cache — and the cold-start race

Because the cache is workspace-scoped and keyed by a **cumulative prefix hash**, the 69 review agents —
same agent type, same system prompt, same tool schemas, same verbatim `RULES` block — have **identical
prefixes** up to the point where the per-repo tail begins. The first agent to run **writes** that prefix;
every later agent **reads** it at 0.1×.

`[INFERRED]` **The catch is concurrency.** With ~10–16 agents launching near-simultaneously (§3.4), the
first wave can all start *before* any of them has committed the shared prefix to cache — so the first
wave pays **cache-creation** (1.25–2×) and only later waves get the 0.1× reads. Net effect: real sharing
is very good but not perfect. A cheap trick to force a warm cache: **run one tiny "primer" agent first**
(same prefix, trivial tail) so the prefix is written once before the big wave fans out.

### 2.3 Structure prompts so the stable prefix caches and only the tail varies

This is the actionable rule, straight from the docs: `[VERIFIED]` *"Place `cache_control` on the last
block whose prefix is identical across the requests you want to share a cache … For a prompt with a
varying suffix (timestamps, per-request context, the incoming message), place the breakpoint at the end
of the static prefix, not on the varying block."*

**In practice for a fan-out:**
- Put the big invariant material **first**: the hard rules, the report template, the output contract,
  the schema description — everything identical across all 69 agents.
- Put the per-item material **last**: the repo name, the one-line pointer to that repo's brief file, the
  slug. Keep this tail small.
- **Never interpolate a timestamp, run-id, counter, or `Date.now()` into the shared region.** One varying
  byte early in the prompt invalidates the entire downstream cache for that agent. (This is also why
  workflow scripts forbid `Date.now()`/`Math.random()` — a varying prefix would both break resume *and*
  bust cache.)

### 2.4 Quantifying the caching saving

`[ESTIMATE — illustrative, using the verified multipliers]` Let a fan-out of **N = 69** agents each have
a **shared prefix P** and a **unique tail U** (input tokens):

- **No caching:** input ≈ `N × (P + U)`.
- **Ideal caching** (1 write, N−1 reads, within the 1h TTL): ≈ `(2P + U) + (N−1) × (0.1P + U)`.

With **P = 8,000, U = 2,000, N = 69**:
- No cache ≈ 69 × 10,000 = **690,000 input tokens**.
- Ideal cache ≈ 18,000 + 68 × 2,800 = **208,400 input tokens** → **~70% reduction** in fan-out input.
- Realistic (first wave of ~10 all write, §2.2): ≈ **~55–65% reduction**.

The **break-even is trivial**: a 1-hour write costs 2× but each read saves 0.9× versus uncached, so the
prefix pays for itself after **~2 reads** — and a 69-way fan-out gives you 68. `[VERIFIED, corroborated]`
("break even after two cache hits," [Respan](https://www.respan.ai/articles/claude-prompt-caching)).

---

## 3. Workflow-tool levers (`Workflow` / `agent()` / `pipeline()` / `parallel()`)

All primitive behaviours in this section are `[VERIFIED, first-party]` from the Claude Code **Workflow
tool contract** in this session unless otherwise noted.

### 3.1 Per-phase model tiering and per-call `effort`

- `agent(prompt, {model, effort})` — `model` overrides the tier for that call; `effort` ∈
  `low|medium|high|xhigh|max` overrides reasoning effort. Both default to inheriting the session's.
- **Token/quality tradeoff** `[VERIFIED]`: "Opus costs several times more per turn than Sonnet, and
  Sonnet more than Haiku … Spending Opus on routine work is the fastest way to drain a daily limit"
  ([support 14552983](https://support.claude.com/en/articles/14552983-models-usage-and-limits-in-claude-code)).
  The built-in `/model opusplan` pattern — **Opus to plan, Sonnet to execute** — encodes the highest-ROI
  split; "switching models doesn't clear the conversation, so Sonnet still sees everything Opus produced."
- **When it's worth it:**
  - **Opus / high effort:** the 8 team-lead *syntheses* (cross-repo reasoning, contradiction-finding) and
    any adversarial verify step. This is where deep reasoning changes the answer.
  - **Sonnet / medium:** the 69 per-repo reviews. Reading a pre-measured brief + a few files and filling
    a template is "mostly mechanical" once the brief exists.
  - **Haiku / low:** the long tail of sub-1k-LOC repos (our fleet had many: 55, 58, 90, 138, 204, 316 LOC
    repos) and any pure-extraction stage. `[VERIFIED]` Haiku is "well suited to … high-volume scripted
    runs."
- **Rule of thumb** `[ESTIMATE]`: moving a stage down one tier is a **2–5× token cost cut on that stage**.
  Tier on a *measured* proxy (LOC from the brief), not a guess, so you don't send a 100k-LOC repo to Haiku.

### 3.2 Schema-forced structured returns vs free text

- `agent(prompt, {schema})` forces the subagent to return a **validated object**, not prose; validation
  happens at the tool-call layer so the model retries on mismatch.
- Our workflow used `REVIEW_SCHEMA` / `LEAD_SCHEMA` so each agent returns a **compact record** (`slug`,
  `status`, `loc_total`, `critical_work[]`, …) while **the real artifact is the HTML file written to
  disk.** The orchestrator's context receives ~15 small fields per agent, not a 2,000-word review.
- **Why it saves tokens:** the *return value* is the thing that flows back into the orchestrator and into
  the next phase's prompt (`LEAD(team, results)` embeds `JSON.stringify(results)`). A prose return would
  put 69 full reviews into the team-lead prompts; a schema return puts ~69 small objects. `[ESTIMATE]`
  This is often a **5–20× reduction** in what crosses the phase boundary, and it's the difference between
  the orchestrator staying small and it compacting mid-run.
- **Discipline:** subagents are told their final text *is* the return value, so they emit raw data, not a
  human message. Keep schemas lean — only fields a downstream phase actually consumes.

### 3.3 `pipeline` vs `parallel`: barriers waste wall-clock, not tokens

- `parallel(thunks)` is a **barrier** — it awaits all thunks. `pipeline(items, stage1, stage2, …)` runs
  each item through all stages with **no barrier between stages**: item A can be in stage 2 while item B
  is still in stage 1.
- Our script uses `pipeline` over *teams* with `parallel` *within* a team, so **each team lead fires the
  moment that team's reviews land**, instead of every lead waiting on the single slowest reviewer
  anywhere in the fleet.
- **Token vs wall-clock:** a barrier does **not** cost extra tokens — the same agents run either way. It
  costs **wall-clock**: fast agents sit idle waiting for the slowest. `[VERIFIED, first-party]` (Workflow
  docs: "barrier latency is real"). **But wall-clock matters for tokens indirectly:** a fan-out that
  drags past the 1-hour cache TTL (or into overage, §1.5) starts *re-writing* cache — so the barrier's
  idle time can quietly convert into cache-write cost. Prefer `pipeline` unless a stage genuinely needs
  *all* prior results at once (dedup/merge across the whole set, early-exit on zero findings).

### 3.4 Concurrency and the Bash safety classifier — the failure mode we hit live

- `[VERIFIED, first-party]` Concurrent `agent()` calls are **capped at `min(16, cpu_cores − 2)` per
  workflow**; excess calls queue. Lifetime cap is **1,000 agents**; a single `parallel`/`pipeline` call
  takes **≤ 4,096 items**.
- **The classifier interaction** `[VERIFIED — observed in this very session]`: Claude Code gates
  potentially-unsafe tool calls (Bash, some MCP calls) through a **Sonnet safety classifier**. Under load
  it becomes transiently unavailable and **blocks the gated call**, returning *"claude-sonnet-5 is
  temporarily unavailable, so auto mode cannot determine the safety of … right now."* I hit this **repeatedly
  while researching this report**, and — tellingly — **parallel classifier-gated calls were blocked more
  often than single ones** (two `ctx_execute` calls issued together both failed; the same call issued
  alone succeeded on retry). Read-only operations (Read, code search) are **not** gated and kept working
  throughout.
- **Quantified tradeoff** `[INFERRED, from the live observation + the concurrency cap]`: at the 16-agent
  ceiling, many agents issue Bash/MCP calls simultaneously → classifier saturation → **transient blocks →
  failed or retried agents.** Each retry is **wasted tokens** (the agent re-runs its prefix). So raw
  concurrency has a **hidden cost curve**: beyond some point, more parallelism buys *more retries*, not
  more throughput.
- **Mitigations that reduce classifier-induced failures:**
  1. **Prefer read-only tools inside agents.** Read/Grep/Glob aren't classifier-gated. Our design already
     leaned this way: agents read pre-exported **git-less trees** and a **brief**, so they rarely need
     Bash at all. An agent that never shells out never trips the classifier.
  2. **Fewer, larger agents.** Batching M repos per agent (§6) cuts the agent count below the concurrency
     ceiling, so fewer gated calls collide.
  3. **Do the shell work *before* the fan-out** (§4): all `git`/`cloc`/`clone` ran in pre-compute scripts,
     not inside 69 agents, so the classifier never saw 69 concurrent `git` calls.
  4. **Idempotent retries** (§3.5): if an agent *does* die to a transient block, resume replays the ones
     that finished, so the classifier blip costs one agent's retry, not the whole run.

### 3.5 Idempotent resume — the mechanism that saved our run

Three layers, cheapest first:

1. **`done`-map / disk check (application-level, our fix).** The script accepts
   `args = {manifest, done}`. `reviewOne(r)` short-circuits: `doneSlugs.has(r.slug)` → returns the cached
   `#project-meta` **with no `agent()` call**. The comment says it plainly: *"A repo present in `done`
   costs ZERO tokens — we return its meta as the review result instead of spawning an agent — so a re-run
   only pays for the reports that are actually missing."* `args_resume.json` = `{manifest:[69],
   done:{17 slugs}}`; `done_reports.json` holds those 17 parsed metas. **This is what let the post-limit
   retry re-run 52, not 69.**
2. **`resumeFromRunId` (workflow-level).** `[VERIFIED, first-party]` Relaunching with
   `{scriptPath, resumeFromRunId}` returns cached results for the **longest unchanged prefix of
   `agent()` calls** instantly; the first edited/new call and everything after runs live. **Same script +
   same args → 100% cache hit.** Keying is on `(prompt, opts)` — so a stable prompt replays.
3. **Prompt cache (token-level, §2).** Even work that *does* re-run pays 0.1× on its shared prefix.

**Design rule:** make the unit of work **idempotent and keyed** (slug → output file). Check the output
exists *before* spawning. Then a limit-stop, a crash, or a classifier blip costs only the missing units.
`[ESTIMATE]` For a run that's 75% done when it dies, this is a **~75% saving on the retry**; for a no-op
re-run, **~100%.**

---

## 4. Do the deterministic work outside the LLM

**Principle:** anything a script can compute exactly should be computed **once, by the script**, and
handed to agents as a small brief — never re-derived by N agents, each paying tokens *and* dumping raw
tool output into its context.

### 4.1 What our pre-compute pipeline did

- **`probe.py`** — for each repo: shallow-clone (`git clone --depth 1 --no-single-branch`) to scratch,
  run **`cloc` per branch**, export each branch as a **git-less folder**, detect **agent surfaces**
  (`.claude/`, `skills/`, `CLAUDE.md`, …), then **delete the clone**. Deterministic, scriptable, zero LLM.
- **`local_probe.py` / `scan_clones.py`** — classify every local clone as **ahead / behind / diverged /
  never-pushed / in-sync** with modified/untracked counts, *without fetching*.
- **`make_briefs.py`** — fuse manifest + probe + clone-scan into **one `brief.json` per repo**:
  `loc_per_branch`, `all_branches`, `tip_commits`, `agent_surfaces`, `local_clones` (with the ahead/behind
  verdict pre-labelled), `commits_author`, `hazard_flag`.

### 4.2 Quantifying it

`[VERIFIED, measured on disk]` **69 briefs, ~94,000 tokens total, median ~635 tokens each** (min ~344,
max ~14k for the biggest multi-branch repo). Each brief replaces, per agent:

`[ESTIMATE]` the work of enumerating branches (1 call), checking out + running `cloc` per branch (2×
branches), a tip-commit `git log` per branch (branches), an agent-surface tree walk (1), and per-clone
ahead/behind computation (several `git rev-list` per branch per clone) — realistically **~10–15 tool
calls for a 1-branch/1-clone repo and 30–50+ for a multi-branch one.** Across 69 repos that's **hundreds
to a couple thousand tool round-trips eliminated**, and — the bigger win — **all their verbose raw
output** (full `cloc` JSON, `git` plumbing, directory walks) **never enters any agent's context.**

### 4.3 Template-driven rendering instead of 69 agents hand-writing HTML

Our agents fill a **`_TEMPLATE.html`** (fixed section order, ids, CSS classes, a machine-readable
`#project-meta` JSON block) rather than inventing HTML each. `[INFERRED]` Benefits: (a) the agent spends
tokens on *findings*, not on re-deriving boilerplate markup; (b) the shared template text is part of the
**cacheable prefix** (§2); (c) `make_index.py` parses `#project-meta` **deterministically** — no agent
is spent building the index. Anything the render is mechanical about → move to a template + a builder
script.

### 4.4 Generalising

Pre-compute and pass in: **LOC (`cloc`/`tokei`), git state (branches, ahead/behind, last-commit), file
trees, dependency graphs, test/lint results, sizes/counts.** The agent's job should start at *judgement*,
never at *measurement*. `[VERIFIED, corroborated]` Anthropic's own cost guidance says the same in
miniature: give "complete context about your coding environment in your initial message … all relevant
data in a single, well-structured message"
([support 9797557](https://support.claude.com/en/articles/9797557-usage-limit-best-practices)).

---

## 5. Context hygiene — keeping the ORCHESTRATOR small

The orchestrator's context growth is what triggers **compaction** (`[VERIFIED]` "Claude Code also
auto-compacts when you get close to the limit"; and the context window "reserves a portion for the
response"). Compaction and re-reads are pure token waste. Levers:

- **Fork vs fresh subagents.** A `fork` inherits the parent's full context (use only when the child truly
  needs it); a fresh subagent starts clean. `[VERIFIED, first-party]` A fork "runs in the background and
  keeps its tool output out of your context." **Default to fresh** for fan-out units — 69 forks would
  each carry the orchestrator's whole context as prefix (huge, and mostly irrelevant to one repo).
- **Keep raw tool output OUT of the orchestrator.** This is the deepest principle: a subagent's verbose
  work (file dumps, logs, `cloc` JSON) stays in *its* context; only its **schema'd conclusion** returns
  (§3.2). "The agent's final report is not shown to the user — relay what matters." Delegating a search
  returns *the conclusion, not the file dumps.*
- **Deferred tools / `ToolSearch`.** `[VERIFIED, first-party + observed]` Tool schemas load **on demand**:
  they appear by name and you fetch the full schema via `ToolSearch` only when you actually call them.
  This keeps dozens of MCP tool definitions out of the base prompt — and since **`tools` is the first,
  largest slice of the cache prefix** (§2.1), a bloated tool list also bloats every cache write. I used
  this in this very session (loading only WebSearch/WebFetch/ctx tools when needed).
- **Don't re-read files.** `[VERIFIED, first-party]` "The harness tracks file state for you" — re-reading
  a just-edited file to 'verify' is wasted tokens; Edit/Write would have errored on failure. Read only
  the part of a file you need (offset/limit), not the whole thing.
- **Return conclusions, not file dumps** — from agents *and* in your own turns. A pointer
  (`file_path:line`) beats pasting the file.
- **Process data in code, not in reasoning.** Summarise large outputs with a script that prints only the
  answer (I used `ctx_execute` throughout this research so raw HTML/JSON never hit my context). `[VERIFIED]`
  Anthropic: "Tools and connectors are token-intensive"
  ([support 11647753](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work)).
- **`CLAUDE.md` is paid on every turn.** `[VERIFIED, corroborated]` "A 5,000-token CLAUDE.md costs 5,000
  tokens on every single turn" ([KDnuggets](https://www.kdnuggets.com/7-practical-ways-to-reduce-claude-code-token-usage)).
  Keep it lean; it's stable, so it caches — but it's still in every subagent's prefix.

**Why this reduces the orchestrator's growth specifically:** every one of these keeps bytes *out of the
main loop's message history*. The main loop is the one context that must survive the whole run; if it
compacts, you pay to summarise and you lose fidelity. Subagents are disposable — let them hold the mess.

---

## 6. Right-sizing the fan-out

**69 agents was too many for one window.** 77 agents × ~average per-agent cost = ~3.6M tokens, which
exceeded the rolling-window budget mid-run. The fix isn't "never fan out" — it's **match the fan-out to
the window and the work distribution.**

### 6.1 When 1-agent-per-repo is right

- Repos are **large and heterogeneous** (each needs genuine independent judgement).
- The count fits comfortably under the session budget (with headroom for the shared-prefix writes).
- You want **maximum parallel wall-clock** and per-repo isolation.

### 6.2 When to batch M repos per agent

- A **long tail of tiny repos** (ours had many sub-300-LOC repos: 55, 58, 90, 138, 204, 316 LOC). Paying a
  full agent's fixed overhead (system prompt + tools + rules prefix ≈ several k tokens) to review 55 LOC
  is almost all overhead. **Batch 5–10 tiny repos into one agent** and the fixed prefix is amortised once,
  not ten times.
- Homogeneous repos where cross-repo context within the batch is *useful*, not distracting.
- **This also cuts the agent count under the concurrency ceiling (§3.4)** → fewer classifier collisions.

### 6.3 When to run in waves / use a cheaper model for the tail

- **Waves:** if the whole fleet can't fit one window, split into waves sized to the budget, each wave
  resumable (§3.5). A wave that finishes writes its outputs; the next wave's `done`-map skips them for
  free even if the first wave partially failed.
- **Cheaper model for the tail:** send the sub-1k-LOC tail to **Haiku**, reserve **Sonnet** for mid-size,
  **Opus** only for synthesis. On our fleet the tail was a large fraction of the 69, so this alone is a
  big cut on the *all-models weekly* pool while leaving *Opus weekly* for the leads.

### 6.4 A decision rule

`[INFERRED — synthesised from the above]` Let **R** = repo count, **S** = median repo size (LOC),
**B** = your remaining session budget (tokens), **C** ≈ per-agent fixed overhead (prefix+tools, ~5–10k
tokens uncached / ~1–2k cached).

1. **Estimate the run:** `cost ≈ Σ_repos (C_cached + work(size))`. If `cost > B`, you *will* hit the wall
   → plan waves or resume up front. **Never launch a run you've estimated exceeds the window without a
   `done`-map.**
2. **Batch the tail:** any repo with `work(size) < ~2×C` should be **batched** — the overhead dominates
   the work. Group them 5–10 per agent.
3. **Tier by size:** `size < ~1k LOC → Haiku`; `~1k–20k → Sonnet`; `> ~20k or cross-cutting → Opus`.
   Syntheses always Opus.
4. **Cap concurrency below the collision point:** keep effective parallel agents ≲ 8–10 when agents shell
   out; push higher only when agents are read-only (§3.4).
5. **On Max 5× vs 20×:** the **Opus weekly pool is the binding constraint** (§1.3, ~15–40 h/week). The
   smaller your Opus budget, the more aggressively you must push reviews to Sonnet/Haiku and reserve Opus
   strictly for synthesis. On 20× you can afford a few more Opus reviewers; on 5× keep Opus to the ~8
   leads only.

---

## 7. Prioritised recommendations checklist for the NEXT large workflow

Ordered by (impact ÷ effort). Savings are `[ESTIMATE]` on a run like ours unless a verified figure exists.

| # | Action | Est. token saving | Quality risk | Why it's safe / how to de-risk |
|---|--------|-------------------|--------------|--------------------------------|
| 1 | **Make every unit idempotent + keyed to an output file; always pass a `done`-map / check disk before spawning.** | ~75% on any retry; ~100% on a no-op re-run | None | The cached result *is* the prior output. Already proven on our run. |
| 2 | **Estimate `cost` vs remaining budget BEFORE launching; if it exceeds the window, split into resumable waves.** | Avoids the entire 52-agent failure + retry | None | Turns a hard wall into a checkpoint. Pure planning. |
| 3 | **Pre-compute all deterministic facts (LOC, git state, trees) into a per-unit brief; agents read the brief, not the repo.** | ~30–50% of each agent's input + hundreds of tool calls eliminated | Low | Regenerate briefs each run so they can't go stale. |
| 4 | **Structure prompts: big invariant material first (cacheable prefix), tiny per-item tail last; never interpolate timestamps/ids/counters into the prefix.** | ~55–70% of shared-prefix input across the fan-out | None | Verified docs rule; costs nothing but prompt ordering. |
| 5 | **Schema-force every agent return; write the real artifact to disk, return a compact object.** | 5–20× on what crosses each phase boundary; prevents orchestrator compaction | Low | Keep schemas lean; validation auto-retries. |
| 6 | **Model-tier by measured size: Haiku (tail) / Sonnet (mid) / Opus (synthesis only). Use `opusplan`-style plan-Opus/execute-Sonnet.** | 2–5× per stage moved down a tier; protects the scarce Opus weekly pool | Medium | Tier on LOC from the brief, not a guess. Verify a sample of Haiku outputs. |
| 7 | **Batch the long tail of tiny repos (5–10 per agent).** | Amortises ~5–10k fixed overhead per batched repo | Low | Keep batches small + homogeneous so per-repo focus survives. |
| 8 | **Prefer read-only tools inside agents; do shell work in pre-compute, not in 69 concurrent agents.** | Cuts classifier-induced retries (each retry = a wasted agent) | Low | Read/Grep/Glob aren't classifier-gated; verified live this session. |
| 9 | **Prefer `pipeline` over `parallel`; use barriers only for genuine cross-item dedup/early-exit.** | No direct token cut, but avoids cache-window/overage drift (§3.3) | None | Correctness identical; only scheduling changes. |
| 10 | **Keep raw output out of the orchestrator: fresh (not fork) subagents, deferred tools, no file re-reads, process data in code.** | Delays/avoids orchestrator compaction (which is itself a token tax) | None | Standard hygiene; verified harness behaviours. |
| 11 | **Warm the cache with one primer agent before the big wave; finish the fan-out inside the 1-hour TTL.** | Recovers the first-wave cold-start loss (§2.2); avoids 5-min-TTL overage re-writes | None | One tiny extra agent; strictly additive. |
| 12 | **Trim `CLAUDE.md` and the active tool list** (both sit in every subagent's cached prefix). | Smaller prefix → smaller cache writes on every agent | Low | Move rarely-needed detail out of always-loaded files. |

**If you do only three:** #1 (idempotent resume), #3 (pre-compute briefs), #4 (cacheable-prefix prompt
structure). Those three, together, are most of the 3.6M → ~1M kind of reduction, at near-zero quality
risk.

---

## Appendix — Verified vs Inferred ledger

Every quota/cache/mechanism claim, its status, and its source. **Primary** = Anthropic-owned page or the
Claude Code runtime's own tool contract. **Reported** = reputable secondary corroborating an Anthropic
announcement. **Inferred/Estimate** = my reasoning, labelled.

### Usage / quota mechanics

| Claim | Status | Source |
|-------|--------|--------|
| Rolling **5-hour** session window; resets with countdown shown in Claude Code | **VERIFIED (primary)** | [support 14552983](https://support.claude.com/en/articles/14552983-models-usage-and-limits-in-claude-code); [support 9797557](https://support.claude.com/en/articles/9797557-usage-limit-best-practices) |
| **Weekly** cap alongside the 5-hour cycle; announced 2025-07-28, effective 2025-08-28 | **VERIFIED (primary + reported)** | [Anthropic on X](https://x.com/AnthropicAI/status/1949898502688903593); [TechCrunch](https://techcrunch.com/2025/07/28/anthropic-unveils-new-rate-limits-to-curb-claude-code-power-users/) |
| **Two** weekly limits: "Opus only" and "all other models" | **VERIFIED (primary)** | [support 9797557](https://support.claude.com/en/articles/9797557-usage-limit-best-practices) |
| Per-tier reported hours (Max 5×: 140–280h Sonnet / 15–35h Opus; Max 20×: 240–480h / 24–40h) | **VERIFIED (reported)** — not on Anthropic's current pages | [TechCrunch](https://techcrunch.com/2025/07/28/anthropic-unveils-new-rate-limits-to-curb-claude-code-power-users/) |
| **5-hour limits doubled**, peak-hours reduction removed, **May 6 2026** | **VERIFIED (primary)** | [anthropic.com/news/higher-limits-spacex](https://www.anthropic.com/news/higher-limits-spacex) |
| All product surfaces (claude.ai, Claude Code, Desktop) share **one** usage pool | **VERIFIED (primary)** | [support 11647753](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work) |
| `/usage` attributes usage to **skills, subagents, plugins, MCP servers** against plan limits | **VERIFIED (primary)** | [code.claude.com/docs/costs](https://code.claude.com/docs/en/costs) |
| Workflow token budget is **shared across main loop + all workflows**, not per-workflow | **VERIFIED (first-party)** | Claude Code Workflow tool contract (this session) |
| On overage: wait for reset / upgrade / buy credits / switch model / API-key fallback | **VERIFIED (primary)** | [support 11647753](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work); [support 14552983](https://support.claude.com/en/articles/14552983-models-usage-and-limits-in-claude-code) |
| **Prompt-cache TTL drops 1h → 5m in usage overage** | **VERIFIED (first-party)** | Claude Code ScheduleWakeup tool contract (this session) |
| `/usage` bars can be up to **60 min stale** when the usage endpoint is rate-limited | **VERIFIED (primary)** | [code.claude.com/docs/costs](https://code.claude.com/docs/en/costs) |
| Exact arithmetic of "resets at *time*" (window start + duration) | **INFERRED** (consistent with all verified pieces + observed ~04:50 reset) | — |

### Prompt caching

| Claim | Status | Source |
|-------|--------|--------|
| Default **5-min** TTL; optional **1-hour** TTL (`ttl:"1h"`); TTL resets on each hit | **VERIFIED (primary)** | [platform.claude.com prompt-caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) |
| Prices: 5-min write **1.25×**, 1-hour write **2×**, read **0.10×** base input | **VERIFIED (primary ×2)** | [platform.claude.com prompt-caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching); multipliers confirmed numerically on [claude.com/pricing](https://www.claude.com/pricing) |
| Prefix order **tools → system → messages**; cumulative hash; any change at/before breakpoint misses | **VERIFIED (primary)** | [platform.claude.com prompt-caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) |
| **≤ 4** cache breakpoints; **20-block** lookback | **VERIFIED (primary)** | [platform.claude.com prompt-caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) |
| **Minimum cacheable** ~1,024 tok (Opus 4.8 / Sonnet 5), ~2,048 (older Haiku), up to ~4,096 (some newer); shorter = silently uncached | **VERIFIED (primary text) + reported (exact per-model table)** | [Anthropic prompt-caching docs](https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching) |
| Fields `cache_creation_input_tokens` / `cache_read_input_tokens`; both 0 ⇒ not cached | **VERIFIED (primary)** | [platform.claude.com prompt-caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) |
| **Workspace-level** cache isolation (since 2026-02-05) ⇒ same-workspace agents share cache | **VERIFIED (primary)** | [platform.claude.com prompt-caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) |
| "Place `cache_control` on the last block whose prefix is identical across requests you want to share" | **VERIFIED (primary)** | [platform.claude.com prompt-caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) |
| Concurrent cold-start race ⇒ first wave writes, later waves read (imperfect sharing) | **INFERRED** (from concurrency + write-on-breakpoint semantics) | — |
| ~55–70% fan-out input reduction from caching | **ESTIMATE** (illustrative math, verified multipliers) | — |

### Workflow primitives & Claude Code behaviour

| Claim | Status | Source |
|-------|--------|--------|
| Concurrency cap **min(16, cores−2)**; lifetime **≤1,000** agents; **≤4,096** items/call | **VERIFIED (first-party)** | Claude Code Workflow tool contract (this session) |
| `resumeFromRunId`: longest unchanged `agent()` prefix replays; same script+args ⇒ 100% hit | **VERIFIED (first-party)** | Claude Code Workflow tool contract (this session) |
| `schema` forces validated structured return; validation retries at tool layer | **VERIFIED (first-party)** | Claude Code Workflow tool contract (this session) |
| `pipeline` = no barrier; `parallel` = barrier; "barrier latency is real" | **VERIFIED (first-party)** | Claude Code Workflow tool contract (this session) |
| **Sonnet safety classifier** transiently blocks gated calls under load; parallel gated calls fail more; read-only calls ungated | **VERIFIED (observed live this session)** | This session's tool errors (repeated `claude-sonnet-5 … temporarily unavailable`) |
| Model cost order **Opus > Sonnet > Haiku** ("several times more per turn"); `/model opusplan` | **VERIFIED (primary)** | [support 14552983](https://support.claude.com/en/articles/14552983-models-usage-and-limits-in-claude-code) |
| Claude Code **auto-compacts** near the limit; context window 200K/500K/1M by model | **VERIFIED (primary)** | [support 14552983](https://support.claude.com/en/articles/14552983-models-usage-and-limits-in-claude-code); [support 11647753](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work) |
| Deferred tools / `ToolSearch` load schemas on demand (keeps tool defs out of the prefix) | **VERIFIED (first-party + observed)** | This session's tool environment |
| Model-tier saving ~2–5×/stage; schema saving 5–20×/boundary; classifier-retry cost | **ESTIMATE / INFERRED** | — |

### Case-study measurements (this run)

| Claim | Status | Source |
|-------|--------|--------|
| 69 repos, 8 teams (models-and-analytics 18, deploy-infra-cloud 11, datastreams-messaging 9, personal-and-early 8, fiber-insight-platform 7, public-tooling 6, iot-networking 6, dart-and-agents 4); 46 private / 23 public | **VERIFIED (measured on disk)** | `manifest_slim.json` |
| 69 review + 8 lead agents; ~3.6M subagent tokens; 52/69 failed at the session limit; recovered via `done`-map | **VERIFIED (task record + code)** | task brief; `review69.mjs` |
| 69 briefs, ~94k tokens total, median ~635 tok each | **VERIFIED (measured on disk)** | `briefs/` (69 files) |
| `done`-map replay returns cached meta with **0 tokens, no agent** | **VERIFIED (code)** | `review69.mjs` lines 210–239; `args_resume.json`, `done_reports.json` |
| ~10–40 tool calls per agent eliminated by the brief | **ESTIMATE** | reasoning from `probe.py` / `make_briefs.py` |

---

### Note on method (and eating our own cooking)

This report was produced **without** fanning out a large research swarm, deliberately: the load-bearing
numbers are quota/cache figures that demanded **centralised, primary-source verification** (a swarm would
have returned weakly-sourced paraphrases I'd have had to re-verify anyway), and the **Sonnet safety
classifier was degraded during the session** — spawning many agents into a degraded window is the exact
anti-pattern §3.4 and §6 warn against. Raw HTML/JSON was processed in code (`ctx_execute`) so it never
entered context (§5). That choice is itself an application of the guide: *right-size the fan-out to the
work and the window.*
