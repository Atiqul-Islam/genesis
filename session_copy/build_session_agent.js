#!/usr/bin/env node
/* Session-Copy — Phase 3: orchestrator. One call turns a live session into a session-copy agent's MEMORY.

   Faithful Node (CommonJS, stdlib-only) port of build_session_agent.py.

   Chains Phase 1+2 for the SESSION-COPY part of a build:
     1. capture   the current session's stores (scrubbed)            -> <bundle>/records.jsonl
     2. store     portable history + start-time summary              -> <bundle>/history.sqlite, summary.md
     3. embed     the records into the repo's shared Genesis memory  -> under agent_id=<name>  (recallable)

   The bundle lives at <genesis_home>/agents/<name>/ — exactly where inject.js looks for summary.md, and the
   records are embedded under agent_id=<name> in the repo's shared memory DB, so the assembled agent (built by
   Sensei/Method the normal way) recalls its carried-over history via its standard `recall` tool.

   This does NOT author the agent's persona or run the assembler — that is Sensei's normal single-agent build
   (the "custom" specialization). This module owns only the copy-the-session-into-memory half. build-agent Step
   2a invokes it when the user chose "copy my current session".

   Run:
     node build_session_agent.js --session <id> --name <agent> --repo <target_repo> \
         --genesis-home <repo>/.genesis --server-bin <...>/genesis-memory-server \
         --model-dir <...>/models --memory-db <repo>/.genesis/memory.db [--known-secret V ...] [--no-user-config]
*/
"use strict";
const fs = require("fs");
const path = require("path");

const CAP = require("./capture.js");
const STORE = require("./store.js");
const EMB = require("./embed.js");

function resolve_session(session_id, repo) {
  // Resolve `--session current` from the session-pointer hook's <repo>/.genesis/current-session.json.
  if (session_id && session_id !== "current") {
    return session_id;
  }
  const ptr = path.join(repo, ".genesis", "current-session.json");
  let sid = "";
  try {
    const data = JSON.parse(fs.readFileSync(ptr, "utf8"));
    sid = (data && data.session_id) || "";
  } catch (e) {
    sid = "";
  }
  if (!sid) {
    throw new Error(
      "session '--current' requested but no current-session.json pointer found — is the " +
        "session_pointer hook wired into this repo? (or pass an explicit --session <id>)"
    );
  }
  return sid;
}

async function build(session_id, name, repo, genesis_home, server_bin, model_dir, memory_db, known_secrets, include_user_config) {
  if (include_user_config === undefined) {
    include_user_config = true;
  }
  session_id = resolve_session(session_id, repo);
  const bundle = path.join(genesis_home, "agents", name);
  fs.mkdirSync(bundle, { recursive: true });

  // 1. capture (scrubbed) — the current agent has no prior Genesis memory to merge for a main-session copy.
  const cap = CAP.capture(session_id, repo, bundle, null, null, include_user_config, known_secrets || []);

  // 2. store: portable history + start-time summary digest.
  const st = STORE.build_bundle(path.join(bundle, "records.jsonl"), bundle, name);

  // 3. embed the records into the repo's SHARED memory DB under agent_id=<name> (Genesis isolates by id).
  const recs = EMB._load_from_db(path.join(bundle, "history.sqlite"));
  const em = await EMB.embed_records(recs, name, server_bin, model_dir, memory_db);

  const manifest = {
    agent: name,
    session_id: session_id,
    repo: path.resolve(repo),
    bundle: bundle,
    memory_db: memory_db,
    captured: cap.total_records,
    redactions: cap.total_redactions,
    embedded: em.stored,
    embed_failed: em.failed,
    summary: path.join(bundle, "summary.md"),
    by_source: cap.by_source,
    next:
      "Sensei: author the agent's specialized persona (Method), then genesis-cli assemble — the agent will " +
      "recall this history via its memory tools and load summary.md at start.",
  };
  fs.writeFileSync(path.join(bundle, "session_copy_manifest.json"), JSON.stringify(manifest, null, 2) + "\n", "utf8");
  return manifest;
}

// ---- CLI ---------------------------------------------------------------------------------------------
function _parseArgs(argv) {
  const a = { known_secret: [] };
  let i = 0;
  while (i < argv.length) {
    const t = argv[i];
    if (t === "--session") {
      a.session = argv[++i];
    } else if (t === "--name") {
      a.name = argv[++i];
    } else if (t === "--repo") {
      a.repo = argv[++i];
    } else if (t === "--genesis-home") {
      a.genesis_home = argv[++i];
    } else if (t === "--server-bin") {
      a.server_bin = argv[++i];
    } else if (t === "--model-dir") {
      a.model_dir = argv[++i];
    } else if (t === "--memory-db") {
      a.memory_db = argv[++i];
    } else if (t === "--known-secret") {
      a.known_secret.push(argv[++i]);
    } else if (t === "--no-user-config") {
      a.no_user_config = true;
    }
    i += 1;
  }
  return a;
}

async function main() {
  const a = _parseArgs(process.argv.slice(2));
  const required = ["session", "name", "repo", "genesis_home", "server_bin", "model_dir", "memory_db"];
  for (const k of required) {
    if (!a[k]) {
      process.stderr.write(
        "usage: node build_session_agent.js --session <id> --name <agent> --repo <target_repo> --genesis-home <dir> --server-bin <bin> --model-dir <dir> --memory-db <path> [--known-secret V ...] [--no-user-config]\n"
      );
      process.exit(2);
    }
  }
  try {
    const m = await build(
      a.session,
      a.name,
      a.repo,
      a.genesis_home,
      a.server_bin,
      a.model_dir,
      a.memory_db,
      a.known_secret,
      !a.no_user_config
    );
    process.stdout.write(JSON.stringify(m, null, 2) + "\n");
  } catch (e) {
    process.stderr.write((e && e.message ? e.message : String(e)) + "\n");
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = { resolve_session, build, main };
