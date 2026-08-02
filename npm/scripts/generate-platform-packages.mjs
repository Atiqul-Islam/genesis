#!/usr/bin/env node
// Emit the eight per-platform package.json manifests for the Genesis memory server, and keep
// the launcher's exact-version optionalDependencies + model dependency in sync.
//
// Each generated package pins `os`/`cpu`/`libc` so npm installs exactly the one matching the
// consumer's machine (the esbuild / @napi-rs / biome model). It has a single `files` entry --
// the native binary CI drops in just before publish -- and no `exports` field, so the launcher
// can `require.resolve('<pkg>/genesis-memory-server[.exe]')` as a plain file.
//
// This script writes ONLY manifests (valid JSON). It never fabricates binaries or weights.
//
// Usage:
//   node scripts/generate-platform-packages.mjs                # reuse the launcher's version
//   node scripts/generate-platform-packages.mjs 0.2.0          # stamp version 0.2.0 everywhere
//   node scripts/generate-platform-packages.mjs --version v0.2.0
//   node scripts/generate-platform-packages.mjs --check        # verify only; nonzero exit if drift
//
// CI (release.yml) runs this after checkout with the release version, THEN stages each
// platform's native binary into the matching npm/@xcidos/genesis-memory-server-<key>/ dir
// (and the model into npm/@xcidos/genesis-memory-model/) before `npm publish`.

import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const NPM_ROOT = resolve(HERE, ".."); // .../genesis/npm
const SCOPE_DIR = join(NPM_ROOT, "@xcidos");
const LAUNCHER_PKG_PATH = join(SCOPE_DIR, "genesis-memory-server", "package.json");

const SCOPE = "@xcidos";
const SERVER_PREFIX = "genesis-memory-server-";
const MODEL_PKG = `${SCOPE}/genesis-memory-model`;
const BIN_STEM = "genesis-memory-server"; // Cargo [[bin]] name
const REPO_URL = "git+https://github.com/Atiqul-Islam/genesis.git";
const HOMEPAGE = "https://github.com/Atiqul-Islam/genesis#readme";
const BUGS = "https://github.com/Atiqul-Islam/genesis/issues";
const AUTHOR = "Atiqul Islam";
const LICENSE = "MIT OR Apache-2.0";
const NODE_ENGINE = ">=18";

// The eight published targets. `libc` is set only where npm needs it to disambiguate (Linux).
// `key` doubles as the package-name suffix AND the launcher's `${platform}-${arch}(-${libc})`.
const TARGETS = [
  { key: "darwin-arm64", os: "darwin", cpu: "arm64" },
  { key: "darwin-x64", os: "darwin", cpu: "x64" },
  { key: "linux-arm64-gnu", os: "linux", cpu: "arm64", libc: "glibc" },
  { key: "linux-arm64-musl", os: "linux", cpu: "arm64", libc: "musl" },
  { key: "linux-x64-gnu", os: "linux", cpu: "x64", libc: "glibc" },
  { key: "linux-x64-musl", os: "linux", cpu: "x64", libc: "musl" },
  { key: "win32-arm64", os: "win32", cpu: "arm64" },
  { key: "win32-x64", os: "win32", cpu: "x64" },
];

const OS_LABEL = { darwin: "macOS", linux: "Linux", win32: "Windows" };
const CPU_LABEL = { x64: "x64", arm64: "arm64" };

function exeFor(os) {
  return os === "win32" ? `${BIN_STEM}.exe` : BIN_STEM;
}

function humanFor(t) {
  let s = `${OS_LABEL[t.os] || t.os} ${CPU_LABEL[t.cpu] || t.cpu}`;
  if (t.libc) s += ` (${t.libc})`;
  return s;
}

function manifestFor(t, version) {
  const name = `${SCOPE}/${SERVER_PREFIX}${t.key}`;
  const dir = `npm/@xcidos/${SERVER_PREFIX}${t.key}`;
  // Object key order is significant for a stable diff: keep libc between cpu and files.
  const m = {
    name,
    version,
    description: `Prebuilt genesis-memory-server native binary for ${humanFor(t)}.`,
    license: LICENSE,
    author: AUTHOR,
    homepage: HOMEPAGE,
    bugs: { url: BUGS },
    repository: { type: "git", url: REPO_URL, directory: dir },
    os: [t.os],
    cpu: [t.cpu],
    ...(t.libc ? { libc: [t.libc] } : {}),
    files: [exeFor(t.os)],
    engines: { node: NODE_ENGINE },
    preferUnplugged: true,
    publishConfig: { access: "public" },
  };
  return m;
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function writeJson(path, obj) {
  writeFileSync(path, JSON.stringify(obj, null, 2) + "\n");
}

function parseArgs(argv) {
  const out = { version: null, check: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--check") out.check = true;
    else if (a === "--version") out.version = argv[++i];
    else if (a.startsWith("--version=")) out.version = a.slice("--version=".length);
    else if (!a.startsWith("-")) out.version = a;
  }
  return out;
}

function main() {
  const args = parseArgs(process.argv.slice(2));

  const launcher = readJson(LAUNCHER_PKG_PATH);
  let version = args.version || launcher.version;
  if (!version) {
    console.error("ERROR: no version given and launcher package.json has none.");
    process.exit(1);
  }
  version = String(version).replace(/^v/, "");
  if (!/^\d+\.\d+\.\d+(?:[-+].+)?$/.test(version)) {
    console.error(`ERROR: not a valid semver version: ${version}`);
    process.exit(1);
  }

  const drift = [];

  // 1) The eight platform manifests.
  for (const t of TARGETS) {
    const dir = join(SCOPE_DIR, `${SERVER_PREFIX}${t.key}`);
    const path = join(dir, "package.json");
    const desired = manifestFor(t, version);
    const desiredStr = JSON.stringify(desired, null, 2) + "\n";
    const current = existsSync(path) ? readFileSync(path, "utf8") : null;
    if (current === desiredStr) continue;
    drift.push(`${SCOPE}/${SERVER_PREFIX}${t.key}/package.json`);
    if (!args.check) {
      mkdirSync(dir, { recursive: true });
      writeFileSync(path, desiredStr);
    }
  }

  // 2) Keep the launcher's exact-version links in sync (deps must resolve at publish time).
  const optional = {};
  for (const t of TARGETS) {
    optional[`${SCOPE}/${SERVER_PREFIX}${t.key}`] = version;
  }
  const launcherDesired = {
    ...launcher,
    version,
    optionalDependencies: optional,
    dependencies: { ...(launcher.dependencies || {}), [MODEL_PKG]: version },
  };
  const launcherStr = JSON.stringify(launcherDesired, null, 2) + "\n";
  if (readFileSync(LAUNCHER_PKG_PATH, "utf8") !== launcherStr) {
    drift.push("@xcidos/genesis-memory-server/package.json");
    if (!args.check) writeJson(LAUNCHER_PKG_PATH, launcherDesired);
  }

  // 3) Keep the model package version in sync (launcher pins it exactly).
  const modelPath = join(SCOPE_DIR, "genesis-memory-model", "package.json");
  if (existsSync(modelPath)) {
    const model = readJson(modelPath);
    if (model.version !== version) {
      const modelStr = JSON.stringify({ ...model, version }, null, 2) + "\n";
      if (readFileSync(modelPath, "utf8") !== modelStr) {
        drift.push("@xcidos/genesis-memory-model/package.json");
        if (!args.check) writeFileSync(modelPath, modelStr);
      }
    }
  }

  if (args.check) {
    if (drift.length) {
      console.error(`Out of sync (${drift.length}):`);
      for (const d of drift) console.error("  - " + d);
      console.error("Run: node scripts/generate-platform-packages.mjs " + version);
      process.exit(1);
    }
    console.log(`OK: all manifests match version ${version}.`);
    return;
  }

  if (drift.length) {
    console.log(`Wrote version ${version} to ${drift.length} manifest(s):`);
    for (const d of drift) console.log("  - " + d);
  } else {
    console.log(`Already up to date at version ${version}.`);
  }
}

main();
