---
name: add-issue
description: The procedure for adding/filing a tracker or GitHub issue so it is self-contained and sourced — a zero-context autonomous agent can pick it up and complete it. Use EVERY time you are asked to add or file an issue. Hard rule — zero speculation.
---

# Adding an issue

Use this **every time** you are asked to add or file an issue. The goal: the issue is **self-contained and
sourced** so a **zero-context** autonomous agent can take it over and complete the task — no tribal
knowledge, no "ask the person who wrote it".

## Hard rules (non-negotiable)

- **Zero speculation.** Every claim is verified by reading the actual source in full, and cited. Never
  guess; label anything genuinely uncertain, or ask.
- Cite **Evidence** as `path:line` or a link — verified by reading, NEVER inferred.
- If a needed fact is unknown, **consult the developer-in-charge** — do not invent it.
- Filing an issue is **not a commitment to build it** — resolving an issue is not building it.
- Never write a credential value or private-repo content into a public issue.

## Required sections (every issue)

1. **Problem** — what is wrong or wanted, in plain terms.
2. **Evidence** — `path:line` and/or links, verified by reading (not inferred).
3. **Reproduction** — for a bug: exact steps + expected vs actual.
4. **Proposed resolution** — the intended fix/approach (a proposal, not a commitment).
5. **Acceptance criteria** — checkable conditions that mean "done".
6. **Constraints** — invariants to respect (fail-open, dormant-by-default, no secrets, etc.).
7. **References** — the exact documents/files an agent must read to complete it.

## Procedure

1. Read the request and every source it names, in full. Verify each fact by reading.
2. Draft the issue with all seven sections. Leave nothing to tribal knowledge.
3. Genericize: no credential values, no private-repo/user content in a public issue.
4. File it (e.g. `gh issue create`), then report the URL.

## Do / Don't

- **Do:** source every claim, make it zero-context-completable, ask when unsure.
- **Don't:** speculate, infer evidence, leak private data, or imply filing an issue means building it.
