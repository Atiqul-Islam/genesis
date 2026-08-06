#!/usr/bin/env node
// Bump the plugin version in lockstep across the two places that must never drift:
//   * .claude-plugin/plugin.json  "version"      — what the plugin manager tracks (/plugin update)
//   * bin/genesis-memory.js       RELEASE_VERSION — which GitHub Release the launcher fetches binaries from
// Usage: node scripts/bump-version.mjs <version>        e.g. 0.1.0-beta.5
// The release workflow verifies both equal the git tag, so a release can't ship with mismatched versions.
import fs from "node:fs";
import path from "node:path";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error("usage: node scripts/bump-version.mjs <semver>   (e.g. 0.1.0-beta.5)");
  process.exit(2);
}

const root = path.resolve(import.meta.dirname, "..");

// 1. plugin.json
const pjPath = path.join(root, ".claude-plugin", "plugin.json");
const pj = JSON.parse(fs.readFileSync(pjPath, "utf8"));
pj.version = version;
fs.writeFileSync(pjPath, JSON.stringify(pj, null, 2) + "\n");

// 2. launcher RELEASE_VERSION
const lpPath = path.join(root, "bin", "genesis-memory.js");
let launcher = fs.readFileSync(lpPath, "utf8");
const re = /const RELEASE_VERSION = "[^"]*";/;
if (!re.test(launcher)) {
  console.error("ERROR: RELEASE_VERSION const not found in bin/genesis-memory.js");
  process.exit(1);
}
launcher = launcher.replace(re, `const RELEASE_VERSION = "${version}";`);
fs.writeFileSync(lpPath, launcher);

console.log(`bumped -> ${version}  (.claude-plugin/plugin.json + bin/genesis-memory.js)`);
