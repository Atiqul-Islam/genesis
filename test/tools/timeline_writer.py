"""Append-only HTML timeline writer for /spec-build supervisor runs.

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
"""

from __future__ import annotations

import html
import sys
from datetime import datetime, timezone
from pathlib import Path

TIMELINE_FILENAME = "timeline.html"

CSS = """
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
"""

HEADER_TEMPLATE = """<!doctype html>
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
"""

ANCHOR = "<!-- TIMELINE-ANCHOR -->"

VALID_ACTORS = {
    "supervisor", "spec-agent", "dev-agent",
    "verify-agent", "review-agent", "docs-agent", "user",
    "forge-spec-agent", "forge-dev-agent", "forge-verify-agent",
    "forge-review-agent", "forge-docs-agent",
}

VALID_EVENTS = {"decision", "verdict", "user_question", "user_answer", "checkpoint", "error"}


def init_timeline(run_dir: Path, feature: str) -> Path:
    run_dir.mkdir(parents=True, exist_ok=True)
    path = run_dir / TIMELINE_FILENAME
    if path.exists():
        return path
    started = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")
    path.write_text(
        HEADER_TEMPLATE.format(feature=html.escape(feature), css=CSS, started=started),
        encoding="utf-8",
    )
    return path


def append_event(run_dir: Path, actor: str, event_type: str, summary: str) -> None:
    if actor not in VALID_ACTORS:
        raise ValueError(f"invalid actor: {actor}")
    if event_type not in VALID_EVENTS:
        raise ValueError(f"invalid event_type: {event_type}")

    path = run_dir / TIMELINE_FILENAME
    if not path.exists():
        raise FileNotFoundError(f"timeline not initialized at {path}; run --init first")

    ts = datetime.now(timezone.utc).strftime("%H:%M:%S")
    row_classes = f"actor-{actor} event-{event_type}"
    row = (
        f'<tr class="{row_classes}">'
        f'<td class="ts">{ts}</td>'
        f'<td class="actor">{html.escape(actor)}</td>'
        f'<td class="event">{html.escape(event_type)}</td>'
        f'<td class="summary">{html.escape(summary)}</td>'
        f"</tr>\n"
    )

    text = path.read_text(encoding="utf-8")
    if ANCHOR not in text:
        raise RuntimeError("timeline.html is missing the TIMELINE-ANCHOR marker")
    text = text.replace(ANCHOR, row + ANCHOR, 1)
    path.write_text(text, encoding="utf-8")


def main(argv: list[str]) -> int:
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2

    run_dir = Path(argv[1])

    if argv[2] == "--init":
        if len(argv) < 4:
            print("missing feature name for --init", file=sys.stderr)
            return 2
        init_timeline(run_dir, argv[3])
        return 0

    if len(argv) < 5:
        print("usage: timeline_writer.py <run_dir> <actor> <event_type> <summary>", file=sys.stderr)
        return 2

    actor, event_type, summary = argv[2], argv[3], argv[4]
    append_event(run_dir, actor, event_type, summary)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
