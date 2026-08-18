# Language Detection — `/spec-forge`

Phase 0 (Initiation) detects the target language once and records it on the run state so
every downstream seam (test execution, the CRAP/quality gate, scaffold-to-stubs) can branch
without re-deriving it. This mirrors the `new-model` **Step 0b** precedent (`--lang` detection),
which is an existing mechanism — this file extends it, it does not invent it.

## Rule (first match wins)

1. **Flag** — if a `--lang <value>` flag was passed on the `/spec-forge` invocation, use it.
2. **Rust** — else if `Cargo.toml` (or `server/Cargo.toml`) exists → `"rust"`.
3. **Python** — else if `pyproject.toml` exists → `"python"`.
4. **Default** — else fall back to the default language.

## Output

Write the resolved value to **`state.json.language`** during Phase 0 (right after the initial
`state.json` is written). It is read, not re-computed, by later phases.

## Genesis note

Genesis ships a Rust MCP memory server (`server/Cargo.toml`), so this detection resolves to
`"rust"` for every Genesis run. The field is kept **parameterized rather than hardcoded** so this
copy of `/spec-forge` stays reusable for a non-Rust target — matching the `new-model` Step 0b
precedent. Nothing else in the workflow changes: phase order, the routing table, the handoff
schema, gate semantics, thresholds, the hallucination audit, and run-state/resume are all
language-agnostic and untouched.
