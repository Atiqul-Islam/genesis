// detect_mode.mjs
//
// Single source of truth (testable code) for the mode-detection rule that
// skills/build-engineer/SKILL.md Step 1 describes in prose:
//
//   - If the target is a git work-tree (a `.git` entry present), OR the
//     target contains any non-binary source/manifest file ⇒ 'deep-read'.
//   - If the target is empty, or contains no git repository AND no such
//     file (ignoring dotfiles / `.gitkeep`) ⇒ 'grill'.
//
// Deterministic, dependency-free: node:fs + node:path only.

import fs from 'node:fs';
import path from 'node:path';

const BINARY_PROBE_BYTES = 8000;

function isDotEntry(name) {
  // Ignore dotfiles (e.g. .gitkeep, .gitignore) for the "has source" check.
  // `.git` itself is handled separately by hasGitEntry().
  return name.startsWith('.');
}

function hasGitEntry(dir) {
  return fs.existsSync(path.join(dir, '.git'));
}

// Heuristic: a file is "binary" if a null byte appears in its first N bytes.
// This mirrors the common text/binary heuristic used by git and most tools,
// keeps the check dependency-free, and is deterministic for a given file.
function looksBinary(filePath) {
  let fd;
  try {
    fd = fs.openSync(filePath, 'r');
    const buf = Buffer.alloc(BINARY_PROBE_BYTES);
    const bytesRead = fs.readSync(fd, buf, 0, BINARY_PROBE_BYTES, 0);
    for (let i = 0; i < bytesRead; i += 1) {
      if (buf[i] === 0) return true;
    }
    return false;
  } catch {
    // Unreadable file: treat as not usable as evidence of "source present".
    return true;
  } finally {
    if (fd !== undefined) fs.closeSync(fd);
  }
}

function hasNonBinarySourceOrManifest(dir) {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name === '.git') continue; // handled by hasGitEntry()
    if (isDotEntry(entry.name)) continue; // dotfiles-only doesn't count

    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (hasNonBinarySourceOrManifest(entryPath)) return true;
    } else if (entry.isFile()) {
      if (!looksBinary(entryPath)) return true;
    }
  }
  return false;
}

/**
 * Detect the build-engineer recipe mode for a target directory.
 * @param {string} dir - absolute or relative path to the target directory
 * @returns {'deep-read'|'grill'}
 */
export function detectMode(dir) {
  if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
    throw new Error(`detectMode: not a directory: ${dir}`);
  }
  if (hasGitEntry(dir)) return 'deep-read';
  if (hasNonBinarySourceOrManifest(dir)) return 'deep-read';
  return 'grill';
}

function listRelativeFilesSorted(dir, base) {
  const entries = fs
    .readdirSync(dir, { withFileTypes: true })
    .slice()
    .sort((a, b) => a.name.localeCompare(b.name));
  let out = [];
  for (const entry of entries) {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out = out.concat(listRelativeFilesSorted(entryPath, base));
    } else if (entry.isFile()) {
      out.push(path.relative(base, entryPath).split(path.sep).join('/'));
    }
  }
  return out.sort();
}

/**
 * Pure scaffolding helper: a deterministic, byte-identical-on-repeat summary
 * of a target directory's detected mode + relative file listing. Used to
 * guard the "no-drift" acceptance criterion at the deterministic-contract
 * level — no filesystem writes, no timestamps, no randomness.
 * @param {string} dir
 * @returns {string} deterministic JSON string
 */
export function scaffoldSummary(dir) {
  const mode = detectMode(dir);
  const files = listRelativeFilesSorted(dir, dir);
  return JSON.stringify({ dir: path.basename(dir), mode, files }, null, 2);
}
