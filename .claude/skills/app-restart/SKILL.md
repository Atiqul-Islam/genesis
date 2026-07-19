---
name: app-restart
description: Restart the application (stop, start, verify)
context: none
allowed-tools: Bash(cd *), Bash(cmd *), Bash(curl *), Bash(sleep *), Bash(sh *), Bash(./*)*, Glob
---

# App Restart Skill

Stop and restart the application, then verify it's healthy.

## Usage

```
/app-restart
```

No arguments needed.

## Workflow

### Step 1: Detect Start/Stop Mechanism

Look for the project's start and stop scripts. Common patterns:
- `start.bat` / `stop.bat` (Windows batch)
- `start.sh` / `stop.sh` (Unix shell)
- `docker-compose.yml` (Docker)
- `Makefile` with `start`/`stop` targets
- `package.json` with `start`/`stop` scripts

Use Glob to find these files. If multiple options exist, prefer the most specific one (e.g., a dedicated start script over a generic Makefile target).

### Step 2: Stop

Run the project's stop mechanism. If no stop script exists, find and kill the running process.

### Step 3: Start

Run the project's start mechanism.

### Step 4: Verify

Poll the app up to 10 times, 2 seconds apart, until it responds successfully:
- Try the app's health endpoint if one exists
- Otherwise try the base URL
- Check for HTTP 200 (or any successful response)

- If healthy within 10 attempts: report success.
- If not healthy after 10 attempts: report failure and suggest checking logs.

### Step 5: Report

**Success:**
```
App restarted successfully.
- Status: HTTP 200
```

**Failure:**
```
App failed to start after 20 seconds.
- Check logs for errors.
```

## WORKFLOW COMPLETE

After reporting, this skill workflow is FINISHED. Return to normal conversation mode.
