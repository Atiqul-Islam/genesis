# TODO

## Platform coverage — deferred targets

The alpha publishes the mainstream platforms. **Missing only Alpine-on-ARM and Windows-on-ARM**,
both hard-blocked by upstream dependencies (not by CI config):

- [ ] **`linux-arm64-musl`** (Alpine / musl on ARM64) — `esaxx-rs` (a C++ dependency of
  `tokenizers`) needs a musl C++ compiler (`aarch64-linux-musl-g++`), which `musl-tools` does not
  provide, and zig can't substitute (it fails compiling tract-linalg's ARM64 SVE kernels).
  *Fix path:* wire a full musl-ARM64 C++ cross toolchain (e.g. musl.cc) into the release build.

- [ ] **`win32-arm64`** (Windows on ARM64) — tract-linalg's hand-written ARM64 assembly is
  GNU/Clang syntax, which MSVC's `armasm64` cannot assemble — fails even on a native
  `windows-11-arm` runner (`LNK1181`, missing `.o`). *Fix path:* build the
  `aarch64-pc-windows-gnullvm` target (clang/LLVM assembler) instead of `-msvc`.

Both are the smallest niches. The launcher already lists all 8 targets in `optionalDependencies`,
so these platforms get a clean "unsupported platform" message until they're published.

## Beta hook rewrite — deferred optimizations (design calls, not blockers)

The Node → Rust hook rewrite (branch `beta`) fixed the measured bottlenecks: `validate` 62.5 s → 0.55 s
(ignore-aware prune), `review` moved off `claude -p` ×2×expertise onto a built-in Haiku `agent` hook, and
every deterministic hook is a ~2–10 ms native binary spawn instead of ~65 ms Node. Two smaller items were
deliberately deferred because each is a design decision rather than a safe drop-in:

- [ ] **tract warm-up embed.** The memory server loads the embedder LAZILY, so the first `store`/`recall`
  pays a ~1.5 s cold-embed (tract plan JIT) once per server lifetime. Warming it up requires EAGER load at
  startup (trades ~5 s startup for a fast first op) or a background warm-up thread — but the server's embed
  path is single-threaded `&mut Embedder`, so a background thread needs the concurrency model reworked.
  Decide eager-vs-lazy (or background-warm) before implementing.
- [ ] **review skip-when-unchanged.** A built-in `agent` hook can't be gated by a preceding deterministic
  hook, so "skip the LLM review when artifacts + expertise are unchanged" isn't expressible in the built-in
  hook model. Options: a content-hash check baked into the review prompt (Haiku reads a cache file), or move
  review back to a Rust command hook that hashes then shells Haiku. The Haiku agent hook is already fast, so
  this is a cost optimization, not a latency fix.

## Follow-ups

- [ ] Migrate npm publishing from the bypass-2FA token to **OIDC trusted publishing** (token-free).
  Requires the package to already exist (done), npm CLI ≥ 11.5.1 + Node ≥ 22.14 in CI, and a
  Trusted Publisher configured on npmjs.com. Bypass-2FA tokens lose direct-publish ~Jan 2027.
