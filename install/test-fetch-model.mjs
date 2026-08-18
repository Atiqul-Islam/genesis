#!/usr/bin/env node
// Tests for scripts/fetch-model.mjs — the cross-platform (Node stdlib) model fetcher.
//
// Node port of install/test_fetch_model.py. Pure Node, NO NETWORK (the download stream opener
// `_internals.openStream` is stubbed). Verifies the pinned revision + SHA-256 have NOT drifted
// from the load-bearing source (server/src/embed.rs), and that the fetch writes both artifacts,
// verifies fail-closed, and honors the capture (--print-only) and non-default-revision guards.
//
// Run:  node install/test-fetch-model.mjs

import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import process from "node:process";
import { Readable } from "node:stream";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const GENESIS = dirname(HERE);
const FM_MJS = join(GENESIS, "scripts", "fetch-model.mjs");
const EMBED_RS = join(GENESIS, "server", "src", "embed.rs");

const M = await import(pathToFileURL(FM_MJS).href);

let passed = 0;
let failed = 0;
function check(name, cond) {
  if (cond) passed += 1;
  else failed += 1;
  process.stdout.write(`  ${cond ? "PASS" : "FAIL"}  ${name}\n`);
}
const sha = (buf) => createHash("sha256").update(buf).digest("hex");
const isHex = (s, n) => s.length === n && /^[0-9a-f]+$/.test(s);
const mkTmp = () => mkdtempSync(join(tmpdir(), "genesis-fm-"));

// ── Pins have NOT drifted from the load-bearing source ────────────────────────────────
function testPinsMatchSources() {
  const embed = existsSync(EMBED_RS) ? readFileSync(EMBED_RS, "utf8") : "";
  check("REVISION is a 40-char lowercase hex git SHA", isHex(M.DEFAULT_REVISION, 40));
  check("EXPECTED_MODEL_SHA256 is a 64-char lowercase hex digest", isHex(M.EXPECTED_MODEL_SHA256, 64));
  check("model repo is the pinned all-MiniLM-L6-v2", M.REPO === "sentence-transformers/all-MiniLM-L6-v2");
  if (embed) {
    check("REVISION matches server embed::MODEL_REVISION", embed.includes(M.DEFAULT_REVISION));
    check("SHA-256 matches server embed::MODEL_SHA256", embed.includes(M.EXPECTED_MODEL_SHA256));
  } else {
    check("server embed.rs present to cross-check", false);
  }
}

function testDefaultDest() {
  check("defaultDest -> <repo>/server/models", M.defaultDest() === join(GENESIS, "server", "models"));
}

// ── Mocked fetch (no network) ─────────────────────────────────────────────────────────
function stubOpener(onnxBytes, tokBytes, seen) {
  return async (url) => {
    seen.push(url);
    if (url.endsWith("/onnx/model.onnx")) return Readable.from(onnxBytes);
    if (url.endsWith("/tokenizer.json")) return Readable.from(tokBytes);
    throw new Error(`unexpected url ${url}`);
  };
}

async function testFetchWritesAndVerifies() {
  const orig = M._internals.openStream;
  try {
    const onnxBytes = Buffer.concat([Buffer.from("ONNXFAKE"), createHash("sha256").update("x").digest()]);
    const tokBytes = Buffer.from('{"tok":1}');
    const onnxSha = sha(onnxBytes);
    const seen = [];
    M._internals.openStream = stubOpener(onnxBytes, tokBytes, seen);

    let dest = mkTmp();
    const digest = await M.fetchModel(dest, M.DEFAULT_REVISION, { verify: true, expectedSha: onnxSha });
    const onnxPath = join(dest, "onnx", "model.onnx");
    const tokPath = join(dest, "tokenizer.json");
    check("fetch writes onnx/model.onnx", existsSync(onnxPath));
    check("fetch writes tokenizer.json", existsSync(tokPath));
    check("returned digest == onnx sha", digest === onnxSha);
    check("onnx bytes intact on disk", Buffer.compare(readFileSync(onnxPath), onnxBytes) === 0);
    check(
      "URL pins the exact HF revision path",
      seen.some((u) => u.includes(`/resolve/${M.DEFAULT_REVISION}/onnx/model.onnx`)) &&
        seen.every((u) => u.includes(`huggingface.co/${M.REPO}`)),
    );
    rmSync(dest, { recursive: true, force: true });

    // verify:true with the WRONG expected sha → throws (fail closed) + onnx removed.
    dest = mkTmp();
    let threw = false;
    try {
      await M.fetchModel(dest, M.DEFAULT_REVISION, { verify: true, expectedSha: "0".repeat(64) });
    } catch {
      threw = true;
    }
    check("fetch verify FAILS closed on sha mismatch", threw);
    check("mismatched onnx removed", !existsSync(join(dest, "onnx", "model.onnx")));
    rmSync(dest, { recursive: true, force: true });

    // capture mode (verify:false): any bytes accepted.
    dest = mkTmp();
    const d2 = await M.fetchModel(dest, M.DEFAULT_REVISION, { verify: false });
    check("capture mode (verify:false) accepts + returns digest", d2 === onnxSha);
    rmSync(dest, { recursive: true, force: true });
  } finally {
    M._internals.openStream = orig;
  }
}

// ── main() argument handling ──────────────────────────────────────────────────────────
async function testMainGuards() {
  const orig = M._internals.openStream;
  try {
    const onnxBytes = Buffer.from("ONNXFAKE-guard");
    M._internals.openStream = stubOpener(onnxBytes, Buffer.from("{}"), []);

    // Non-default revision WITHOUT --print-only → refuse (throw); can't verify against the pin.
    let threw = false;
    try {
      await M.main(["--revision", "deadbeef".repeat(5), "--dest", mkTmp()]);
    } catch {
      threw = true;
    }
    check("non-default revision without --print-only is refused", threw);

    // --print-only with a fake revision succeeds (capture path, verify skipped).
    const dest = mkTmp();
    const rc = await M.main(["--print-only", "--revision", "deadbeef".repeat(5), "--dest", dest]);
    check("--print-only capture returns 0", rc === 0);
    rmSync(dest, { recursive: true, force: true });
  } finally {
    M._internals.openStream = orig;
  }
}

async function main() {
  testPinsMatchSources();
  testDefaultDest();
  await testFetchWritesAndVerifies();
  await testMainGuards();
  process.stdout.write(`\n${passed} passed, ${failed} failed\n`);
  process.exit(failed ? 1 : 0);
}

await main();
