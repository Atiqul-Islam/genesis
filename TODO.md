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

## Follow-ups

- [ ] Migrate npm publishing from the bypass-2FA token to **OIDC trusted publishing** (token-free).
  Requires the package to already exist (done), npm CLI ≥ 11.5.1 + Node ≥ 22.14 in CI, and a
  Trusted Publisher configured on npmjs.com. Bypass-2FA tokens lose direct-publish ~Jan 2027.
