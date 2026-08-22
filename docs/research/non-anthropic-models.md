# Research: building/using Genesis agents with non-Anthropic models (issue #8)

_Findings for issue #8. Verified against Claude Code docs + the named repos (2026-08-22). No speculation:
anything unverified is labelled._

## 1. Native Claude Code: Claude models only (verified)

- A subagent's `model:` frontmatter accepts only Claude aliases/IDs (`sonnet`/`opus`/`haiku`/`fable`/`inherit`
  or full Claude IDs). Source: https://code.claude.com/docs/en/sub-agents.md
- Non-Anthropic providers (Gemini, OpenAI, …) are **not supported anywhere** — not via Bedrock/Vertex/Foundry
  (Claude-only), and explicitly **not via LLM gateways**: "doesn't support routing Claude Code to non-Claude
  models through any gateway." Sources: https://code.claude.com/docs/en/llm-gateway.md ,
  https://code.claude.com/docs/en/third-party-integrations.md , https://code.claude.com/docs/en/model-config.md
- **Conclusion:** a Genesis agent's OWN turns cannot run on Gemini/OpenAI. The only way to involve another
  model is to have the (Claude-driven) agent **delegate** work to it via an MCP server or an external CLI.

## 2. External plugins/MCP that spin up background agents on other models (verified repos)

| Option | Type | Other models | Background? | Requires |
|---|---|---|---|---|
| **ai-cli-mcp** (npm `ai-cli-mcp`) | MCP server | Claude, Codex (GPT), Gemini, Forge | **Yes — true async**, runs the CLI agent in the background, returns control immediately | Node 20+, the target CLIs installed |
| **claude-delegator** | Claude Code **plugin** (`/claude-delegator:setup`) | GPT (via Codex CLI), Gemini (via Gemini CLI) | Delegated subagents | Codex CLI or Gemini CLI |
| **PAL MCP** (formerly Zen MCP, BeehiveInnovations) | MCP server | Gemini, OpenAI, OpenRouter, Azure, Grok, DIAL, Ollama | Multi-model workflows, shared context | An API key per provider |

- ai-cli-mcp: "True Async Multitasking — agent execution happens in the background… The calling AI can proceed…"
  and "Freedom from Model/Provider Constraints" (Claude/Codex/Gemini/Forge).
- claude-delegator: "GPT expert subagents for Claude Code… Requires Codex CLI or Gemini CLI."
- PAL MCP: "Your AI's PAL — a Provider Abstraction Layer"; API keys for OpenRouter/Gemini/OpenAI/Azure/X.AI/DIAL/Ollama.

## 3. How this maps to Genesis (honest)

- Genesis builds agents as Claude Code subagents → they are Claude. They cannot BE a Gemini/OpenAI agent.
- They CAN delegate to one: add one of the MCP servers above to an agent's tools (its `meta.json` `tools` +
  the repo `.mcp.json`), so the Claude agent dispatches a background task to a Gemini/GPT agent and reads the
  result. This is an integration, not a model swap.
- Separate, in-scope Genesis gap (verified): the assembler emits **no `model:` field** at all
  (`cli/src/render.rs::frontmatter`), so even per-agent CLAUDE model selection (opus/sonnet/haiku) is not
  currently carried. Fixing that is a real, buildable improvement independent of cross-provider.

## 4. Recommendation (for the maintainer to decide — not yet built)

- If the goal is "a Genesis agent can offload work to Gemini/GPT": wire one MCP server (ai-cli-mcp for
  background CLI agents, or PAL for API-key providers) as an optional agent tool. Genesis change is small
  (tool grant + `.mcp.json`), no model swap.
- If the goal is "pick the Claude model per agent": add a `model:` field to `meta.json` → carry it through
  `render::frontmatter`/`main_thread_hooks` (fixes the current drop).
- True non-Anthropic native subagents are **not possible** in Claude Code today.

## Sources

- Claude Code: sub-agents.md, llm-gateway.md, third-party-integrations.md, model-config.md (code.claude.com/docs)
- https://github.com/mkXultra/ai-cli-mcp
- https://github.com/jarrodwatts/claude-delegator
- https://github.com/BeehiveInnovations/pal-mcp-server (Zen/PAL MCP)
