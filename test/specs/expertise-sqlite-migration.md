# Phase A — expertise → SQLite (migration + DB-first readers + propagation)

Part of Feature 2 ([[memory-expertise-redesign]]). Foundation phase: move the RUNTIME expertise store the
hooks read from JSON manifests to `expertise.db`, built (migrated) from the committed substrate, with a
fail-open file fallback so the guard stack can never brick.

## Expected behavior
- A `genesis-cli migrate-expertise <root>` builds `<root>/expertise.db` from `manifests/*.json` +
  `required.json` + `*.md` guides + `learned.jsonl` (if present). Idempotent (source_sha no-op), atomic
  (temp + rename), reversible (`--export` regenerates the committed JSON byte-identically).
- The hooks (validate/gate/inject) read `expertise.db` when present; else fall back to today's file bodies
  — identical enforcement either way.
- Propagation: `syncRepo` (launcher `--sync`) runs `migrate-expertise` so every updated repo rebuilds its
  DB; bootstrap builds it on fresh install; the read-only plugin root ships a pre-built DB from release.

## Acceptance criteria
- A1: `migrate-expertise` on the repo's expertise root creates `expertise.db` with every manifest rule
  (id, text, type, section, ordinal), every required.json mapping, and every guide.
- A2: `hook/expertise_db` queries return results EQUAL to the file-path readers over the same store
  (per-reader parity: required_for, load_manifest ids+checkable, manifest_rule_texts, top_rules, guides).
- A3: With `expertise.db` absent or corrupt, every reader falls back to the file body — no panic, no block.
- A4: `migrate-expertise --export` regenerates required.json + each manifest byte-identically (json_dump_ascii).
- A5: Running `migrate-expertise` twice is a no-op on the second run (source_sha unchanged); the canonical
  logical dump of the DB is byte-identical across runs.
- A6: A `learned.jsonl` row with status "active" appears as an enforced rule (in ids + rule_texts + required);
  a row with status "proposed" does NOT appear in the active set.
- A7: The hook crate still builds and its cold-spawn stays within budget after adding rusqlite; a
  `--no-default-features` (expertise-db off) build compiles file-only (pure fallback).

## Implementation
- `hook/Cargo.toml`: add `rusqlite = { version = "=0.39.0", features = ["bundled"] }` behind a default-on
  `expertise-db` cargo feature.
- `hook/src/expertise_db.rs`: read-only (`SQLITE_OPEN_READ_ONLY`, `PRAGMA query_only`) query fns; any
  error → `None` (fallback). Behind `#[cfg(feature = "expertise-db")]`; a stub returns `None` when off.
- Reader shims (validate/gate/inject): DB-first, file body as else; exact same signatures/returns.
- `cli/src/expertise_migrate.rs` + `main.rs`/`lib.rs` dispatch; `bootstrap.rs` calls it (fail-open).
- `bin/genesis-memory.js syncRepo`: add a fail-open `migrate-expertise` spawnSync.
- Tests: cli (build/export/idempotence/learned), hook (per-reader parity + fallback), all deterministic.

## Schema (expertise.db — gitignored, regenerable; canonical logical dump is the test surface)
```
meta(key PRIMARY KEY, value)                    -- schema_version, source_sha, migrated_at
expertise(name PRIMARY KEY, source, note, origin DEFAULT 'migrated')
rules(expertise, id, section, text, type, predicate, reviewer_criterion,
      origin DEFAULT 'migrated', status DEFAULT 'active', ordinal, PRIMARY KEY(expertise,id))
required(agent, expertise, ordinal, PRIMARY KEY(agent,expertise))
guides(stem PRIMARY KEY, rel_path, sha256, body)
```

## Non-goals (later phases)
- Mneme reflection loop + `expertise-learn` writes (Phase B).
- Retro-learn sweep (Phase C).
