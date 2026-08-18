// Append-only HTML timeline writer for /spec-build supervisor runs.
//
// Faithful Node port of timeline_writer.py (stdlib only). Output HTML, validation,
// exit codes, and the append/anchor behavior are identical to the Python.
//
// Each invocation appends one row to timeline.html for a given run. Renders a vertical
// timeline; supervisor calls this via Bash on every decision, verdict, user question,
// and checkpoint event so the file survives session crashes.
//
// Usage:
//     node timeline_writer.mjs <run_dir> <actor> <event_type> <summary>
//     node timeline_writer.mjs <run_dir> --init <feature_name>

import { existsSync, readFileSync, writeFileSync, mkdirSync, realpathSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// Help text mirrors the Python module docstring printed to stderr on bad args.
const DOC = `Append-only HTML timeline writer for /spec-build supervisor runs.

Each invocation appends one row to timeline.html for a given run. Renders a vertical
timeline; supervisor calls this via Bash on every decision, verdict, user question,
and checkpoint event so the file survives session crashes.

Usage:
    python timeline_writer.py <run_dir> <actor> <event_type> <summary>
    python timeline_writer.py <run_dir> --init <feature_name>

actor:       supervisor | spec-agent | dev-agent | verify-agent | review-agent | docs-agent | user
             | forge-spec-agent | forge-dev-agent | forge-verify-agent | forge-review-agent | forge-docs-agent
event_type:  decision | verdict | user_question | user_answer | checkpoint | error
summary:     one-line description (HTML escaped automatically)
`;

const TIMELINE_FILENAME = "timeline.html";

const CSS = `
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
       max-width: 1000px; margin: 2em auto; padding: 0 1em; color: #1f2937; }
h1 { font-size: 1.4em; }
.meta { color: #6b7280; font-size: 0.85em; margin-bottom: 2em; }
table.timeline { width: 100%; border-collapse: collapse; }
table.timeline tr { border-bottom: 1px solid #e5e7eb; }
table.timeline td { padding: 0.5em 0.75em; vertical-align: top; font-size: 0.9em; }
td.ts { white-space: nowrap; color: #6b7280; font-family: ui-monospace, monospace; font-size: 0.8em; }
td.actor { white-space: nowrap; font-weight: 600; }
td.event { white-space: nowrap; }
td.summary { color: #111827; }
.actor-supervisor { color: #1d4ed8; }
.actor-spec-agent { color: #7c3aed; }
.actor-dev-agent  { color: #047857; }
.actor-verify-agent { color: #b45309; }
.actor-review-agent { color: #be123c; }
.actor-docs-agent { color: #0e7490; }
.actor-user       { color: #4b5563; }
.event-error     { background: #fef2f2; }
.event-checkpoint { background: #fef3c7; }
`;

const HEADER_TEMPLATE = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>spec-build timeline — {feature}</title>
<style>{css}</style>
</head>
<body>
<h1>spec-build timeline — {feature}</h1>
<div class="meta">Run started: {started} UTC</div>
<table class="timeline">
<tbody>
<!-- TIMELINE-ANCHOR -->
</tbody>
</table>
</body>
</html>
`;

const ANCHOR = "<!-- TIMELINE-ANCHOR -->";

const VALID_ACTORS = new Set([
  "supervisor", "spec-agent", "dev-agent",
  "verify-agent", "review-agent", "docs-agent", "user",
  "forge-spec-agent", "forge-dev-agent", "forge-verify-agent",
  "forge-review-agent", "forge-docs-agent",
]);

const VALID_EVENTS = new Set([
  "decision", "verdict", "user_question", "user_answer", "checkpoint", "error",
]);

// Python html.escape(s, quote=True)
function htmlEscape(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#x27;");
}

// Single-pass str.format substitution of the three named fields; inserted values
// are not re-scanned (matches Python str.format semantics).
function formatTemplate(tpl, values) {
  return tpl.replace(/\{(feature|css|started)\}/g, (_, k) => values[k]);
}

function pad2(n) {
  return String(n).padStart(2, "0");
}

function utcDateTime() {
  const d = new Date();
  return `${d.getUTCFullYear()}-${pad2(d.getUTCMonth() + 1)}-${pad2(d.getUTCDate())} ` +
    `${pad2(d.getUTCHours())}:${pad2(d.getUTCMinutes())}:${pad2(d.getUTCSeconds())}`;
}

function utcTime() {
  const d = new Date();
  return `${pad2(d.getUTCHours())}:${pad2(d.getUTCMinutes())}:${pad2(d.getUTCSeconds())}`;
}

export function initTimeline(runDir, feature) {
  mkdirSync(runDir, { recursive: true });
  const path = join(runDir, TIMELINE_FILENAME);
  if (existsSync(path)) return path;
  const started = utcDateTime();
  writeFileSync(
    path,
    formatTemplate(HEADER_TEMPLATE, { feature: htmlEscape(feature), css: CSS, started }),
    "utf-8"
  );
  return path;
}

export function appendEvent(runDir, actor, eventType, summary) {
  if (!VALID_ACTORS.has(actor)) {
    throw new Error(`invalid actor: ${actor}`);
  }
  if (!VALID_EVENTS.has(eventType)) {
    throw new Error(`invalid event_type: ${eventType}`);
  }

  const path = join(runDir, TIMELINE_FILENAME);
  if (!existsSync(path)) {
    throw new Error(`timeline not initialized at ${path}; run --init first`);
  }

  const ts = utcTime();
  const rowClasses = `actor-${actor} event-${eventType}`;
  const row =
    `<tr class="${rowClasses}">` +
    `<td class="ts">${ts}</td>` +
    `<td class="actor">${htmlEscape(actor)}</td>` +
    `<td class="event">${htmlEscape(eventType)}</td>` +
    `<td class="summary">${htmlEscape(summary)}</td>` +
    `</tr>\n`;

  const text = readFileSync(path, "utf-8");
  if (!text.includes(ANCHOR)) {
    throw new Error("timeline.html is missing the TIMELINE-ANCHOR marker");
  }
  // Replace only the first anchor; function replacement avoids $-pattern expansion.
  const updated = text.replace(ANCHOR, () => row + ANCHOR);
  writeFileSync(path, updated, "utf-8");
}

export function main(argv) {
  // argv mirrors Python sys.argv: argv[0] is the script path.
  if (argv.length < 3) {
    process.stderr.write(DOC + "\n");
    return 2;
  }

  const runDir = argv[1];

  if (argv[2] === "--init") {
    if (argv.length < 4) {
      process.stderr.write("missing feature name for --init\n");
      return 2;
    }
    initTimeline(runDir, argv[3]);
    return 0;
  }

  if (argv.length < 5) {
    process.stderr.write(
      "usage: timeline_writer.py <run_dir> <actor> <event_type> <summary>\n"
    );
    return 2;
  }

  const [actor, eventType, summary] = [argv[2], argv[3], argv[4]];
  appendEvent(runDir, actor, eventType, summary);
  return 0;
}

function isMainModule() {
  try {
    return realpathSync(fileURLToPath(import.meta.url)) === realpathSync(process.argv[1]);
  } catch {
    return false;
  }
}

// Run when invoked directly (process.argv.slice(1) mirrors sys.argv).
if (isMainModule()) {
  process.exit(main(process.argv.slice(1)));
}
