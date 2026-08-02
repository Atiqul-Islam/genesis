#!/usr/bin/env node
// scripts/fetch-model.mjs — fetch the Genesis embedder from a PINNED Hugging Face revision.
//
// Cross-platform (Windows / macOS / Linux) Node port of the former Python `scripts/fetch-model.py`
// and the Linux-only bash `scripts/fetch-model`. Node is the only runtime the plugin may assume,
// so this fetcher uses ONLY the Node standard library (node:https + node:crypto + node:fs) — no
// curl, no sha256sum, no POSIX shell, and no Python required. All three implementations pinned the
// SAME load-bearing revision + SHA.
//
// LOAD-BEARING REVISION: docs/SPEC_FORGE_RUST_UPDATE.md §6.2 #6 — ONNX exports of the SAME model
// differ in output shape (pooled vs last_hidden_state) and in whether token_type_ids is required.
// Pinning the exact commit is what makes the golden embedding vector reproducible. Do NOT change
// REVISION or EXPECTED_MODEL_SHA256 without regenerating the golden vectors + server constants.
//
// Used by:
//   * .github/workflows/release.yml — to fetch + package the model into a release asset.
//   * the @atiqul/genesis-memory-model npm package — CI packages this model into it; the Node
//     launcher pulls it from npm, so end users need no runtime model download.
//
// Usage:
//   node scripts/fetch-model.mjs                 # -> <repo>/server/models  (verifies the pinned SHA)
//   node scripts/fetch-model.mjs --dest DIR      # -> DIR/onnx/model.onnx + DIR/tokenizer.json
//   node scripts/fetch-model.mjs --print-only    # re-capture a NEW revision (skips SHA verify)

import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import { mkdir, rename, rm } from "node:fs/promises";
import { get as httpsGet } from "node:https";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath, pathToFileURL, URL } from "node:url";

export const REPO = "sentence-transformers/all-MiniLM-L6-v2";
// The exact commit, captured at first fetch. Overridable via GENESIS_MODEL_REVISION (matches the
// former bash/py env override) — but a non-default revision requires --print-only, since the pinned
// EXPECTED_MODEL_SHA256 below only holds for the default revision.
export const DEFAULT_REVISION = "c9745ed1d9f207416be6d2e6f8de32d1f16199bf";
// SHA-256 of onnx/model.onnx at DEFAULT_REVISION (== server embed::MODEL_SHA256).
export const EXPECTED_MODEL_SHA256 =
  "6fd5d72fe4589f189f8ebc006442dbb529bb7ce38f8082112682524616046452";

const USER_AGENT = "genesis-memory-launcher";
const HTTP_TIMEOUT_MS = 120_000;
const CHUNK = 256 * 1024;
const MAX_REDIRECTS = 5;

/**
 * Open an HTTPS GET as a readable response stream, following 3xx redirects.
 *
 * Hugging Face `resolve/` URLs 307-redirect to a CDN; Node's `https.get` does NOT follow
 * redirects on its own, whereas the Python port's `urllib` and the bash port's `curl -fL`
 * both did. HTTPS certificates are verified by default.
 */
function openStream(url, redirectsLeft = MAX_REDIRECTS) {
  return new Promise((resolve, reject) => {
    const req = httpsGet(url, { headers: { "User-Agent": USER_AGENT } }, (res) => {
      const status = res.statusCode ?? 0;
      const location = res.headers.location;
      if (status >= 300 && status < 400 && location) {
        res.resume(); // drain and discard the redirect body
        if (redirectsLeft <= 0) {
          reject(new Error(`too many redirects for ${url}`));
          return;
        }
        const next = new URL(location, url).href;
        resolve(openStream(next, redirectsLeft - 1));
        return;
      }
      if (status !== 200) {
        res.resume();
        reject(new Error(`HTTP ${status} for ${url}`));
        return;
      }
      resolve(res);
    });
    req.setTimeout(HTTP_TIMEOUT_MS, () => {
      req.destroy(new Error(`timeout after ${HTTP_TIMEOUT_MS}ms for ${url}`));
    });
    req.on("error", reject);
  });
}

// Injection seam for tests (mirrors the Python test's monkeypatch of `_urlopen`): `openStream(url)`
// resolves to a Readable of the (redirect-followed) response body.
export const _internals = { openStream };

/** Stream a URL to `dest` atomically (write `.part`, then rename). Cross-platform. */
export async function downloadTo(url, dest) {
  await mkdir(dirname(dest) || ".", { recursive: true });
  const tmp = `${dest}.part`;
  const body = await _internals.openStream(url);
  await new Promise((resolve, reject) => {
    const out = createWriteStream(tmp);
    body.on("error", reject);
    out.on("error", reject);
    out.on("finish", resolve);
    body.pipe(out);
  });
  await rename(tmp, dest);
}

/** Streaming SHA-256 of a file, returned as a lowercase hex digest. */
export function sha256File(path) {
  return new Promise((resolve, reject) => {
    const h = createHash("sha256");
    const rs = createReadStream(path, { highWaterMark: CHUNK });
    rs.on("data", (chunk) => h.update(chunk));
    rs.on("error", reject);
    rs.on("end", () => resolve(h.digest("hex")));
  });
}

/** `<repo>/server/models` — the same default target as the former py/bash scripts. */
export function defaultDest() {
  const here = dirname(fileURLToPath(import.meta.url)); // <repo>/scripts
  return join(dirname(here), "server", "models"); // default dest: <repo>/server/models
}

/**
 * Fetch `onnx/model.onnx` + `tokenizer.json` into `dest`. Returns the model.onnx SHA-256.
 *
 * When `verify`, fail closed (throw) if the downloaded model.onnx does not match `expectedSha`,
 * removing the unverified file first.
 */
export async function fetchModel(
  dest,
  revision,
  { verify = true, expectedSha = EXPECTED_MODEL_SHA256 } = {},
) {
  const base = `https://huggingface.co/${REPO}/resolve/${revision}`;
  const onnxDest = join(dest, "onnx", "model.onnx");
  const tokDest = join(dest, "tokenizer.json");

  process.stderr.write(`Fetching ${REPO}@${revision} into ${dest} ...\n`);
  await downloadTo(`${base}/onnx/model.onnx`, onnxDest);
  await downloadTo(`${base}/tokenizer.json`, tokDest);

  const digest = await sha256File(onnxDest);
  if (verify && digest.toLowerCase() !== expectedSha.toLowerCase()) {
    await rm(onnxDest, { force: true });
    throw new Error(
      `ERROR: model.onnx SHA-256 mismatch for ${REPO}@${revision}\n` +
        `  expected ${expectedSha.toLowerCase()}\n  got      ${digest.toLowerCase()}\n` +
        `Refusing to keep an unverified model. (Use --print-only to capture a new revision.)`,
    );
  }
  return digest;
}

/** Parse argv into `{ dest, revision, printOnly }`, matching the former argparse interface. */
export function parseArgs(argv) {
  const args = {
    dest: null,
    revision: process.env.GENESIS_MODEL_REVISION || DEFAULT_REVISION,
    printOnly: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--dest") {
      args.dest = argv[++i];
    } else if (a === "--revision") {
      args.revision = argv[++i];
    } else if (a === "--print-only") {
      args.printOnly = true;
    } else if (a === "-h" || a === "--help") {
      args.help = true;
    } else {
      throw new Error(`unknown argument: ${a}`);
    }
  }
  return args;
}

const HELP = `Fetch the pinned Genesis ONNX embedder.

Usage:
  node scripts/fetch-model.mjs                 # -> <repo>/server/models  (verifies the pinned SHA)
  node scripts/fetch-model.mjs --dest DIR      # target dir (default: <repo>/server/models)
  node scripts/fetch-model.mjs --revision REV  # HF commit to fetch (default: the pinned revision)
  node scripts/fetch-model.mjs --print-only    # capture mode: skip SHA verify, print revision + SHA
`;

export async function main(argv = process.argv.slice(2)) {
  const args = parseArgs(argv);
  if (args.help) {
    process.stdout.write(HELP);
    return 0;
  }

  const dest = args.dest || defaultDest();
  const verify = !args.printOnly;
  if (args.revision !== DEFAULT_REVISION && verify) {
    throw new Error(
      "Refusing to verify a non-default revision against the pinned SHA; re-run with " +
        "--print-only to capture the new revision's SHA, then update the pins deliberately.",
    );
  }

  const digest = await fetchModel(dest, args.revision, { verify });
  process.stdout.write(`REVISION = ${args.revision}\n`);
  process.stdout.write(`SHA-256  = ${digest}\n`);
  if (args.printOnly) {
    process.stdout.write(
      "Paste REVISION into embed::MODEL_REVISION and SHA-256 into embed::MODEL_SHA256.\n",
    );
  } else {
    process.stdout.write("Verified against the pinned SHA-256.\n");
  }
  return 0;
}

// Run only when invoked directly (`node scripts/fetch-model.mjs`), not when imported by a test.
const isMain = import.meta.url === pathToFileURL(process.argv[1] || "").href;
if (isMain) {
  main().then(
    (code) => process.exit(code),
    (err) => {
      process.stderr.write(`${err.message}\n`);
      process.exit(1);
    },
  );
}
