# Feature: APPLIED-EXPERTISE evidence must be a VERBATIM quote of the rule's own text

## Context / Problem

Today the validate hook accepts declaration evidence that is any existing file path OR a quote found in the
agent's own output. In practice the agent cites the expertise `.md` FILE PATH, which always exists, so the
evidence check passes without proving the rule was read or applied (verified: `check_evidence` accepts
`Path::is_file()` on any real file). The customer-facing docs claim evidence "resolves to a file it
produced or a quote that appears in one" — stronger than the code enforced.

New rule (user-directed): to force reading and stop speculation, **each declared rule's evidence MUST be a
verbatim quote of that rule's own text**, and the checker verifies the quote is a real substring of the
rule's `text` in `expertise/manifests/<name>.json`. You cannot quote a rule you did not read. Deterministic,
no LLM.

## Expected Behavior

1. For each `APPLIED-EXPERTISE: <name>#<id> — <evidence>`, the evidence must contain a verbatim snippet of
   rule `<id>`'s `text` from manifest `<name>` (whitespace/case-normalized substring match).
2. A snippet shorter than a minimum length (20 normalized chars) does NOT count (prevents trivial matches).
3. A file path, or any text not present in the rule, is REJECTED — the turn is blocked with a message
   telling the agent to read the rule and quote it.
4. The existing checks are unchanged: real rule-ids, the ≥3 coverage floor, turn-scoping.

## Acceptance Criteria

- AC1: Evidence that is a real ≥20-char substring of the rule's text → PASSES.
- AC2: Evidence wrapped in backticks/quotes → the inner snippet is matched (delimiters ignored).
- AC3: Evidence not present in the rule text → BLOCKS with a "verbatim quote from the rule" reason.
- AC4: A too-short snippet (< 20 normalized chars) → BLOCKS.
- AC5: A file path (the old trivial-pass evidence) → BLOCKS.
- AC6: `manifest_rule_texts` maps each rule id → its `text` from the manifest.

## Implementation Requirements

- Add `manifest_rule_texts(manifest_dir, name) -> HashMap<id, text>` (reads the `text` field per rule).
- Add `quote_is_from_rule(evid, rule_text) -> bool`: strip surrounding `"`/`` ` ``/`'`, normalize
  (lowercase + collapse whitespace), require ≥20 chars, then substring-match against the normalized rule text.
- In `verify_declaration`, replace the produced-file/output-quote spot-check with the rule-quote check.
- Update the SessionStart `inject` instruction: evidence = a verbatim quote of the rule's text.
- Update customer-facing docs (README.md, docs/index.html) to state the real rule (code ↔ docs parity).

## Constraints

- Deterministic; no LLM. Fail-closed on a fabricated/absent quote; tolerant (skip) if a rule has no text.
- Does not change the coverage floor or turn-scoping.
- Ships in the hook binary; reaches existing repos via the update path (binary + settings already handled).
