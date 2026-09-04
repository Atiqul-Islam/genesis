//! Vector skip-detection scorer (feature: cosine warn). The DETERMINISTIC core, no I/O.
//!
//! Given each required rule's distance to the response embedding + the set of rules the agent DECLARED,
//! return the undeclared rules that are among the nearest AND within `margin` of the closest — the
//! "you may have applied these without declaring them" WARNINGS. Warn-only; never blocks. The spike
//! (test/specs/vector-completeness-warn.md) confirmed used-topic rules are the response's nearest
//! neighbours, so a top-k + margin cut over the distances separates skips from noise. Thresholds are
//! calibration items (memory-management mm-26) surfaced as parameters, never hidden constants.

use crate::{io, validate};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::hash::BuildHasher;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// Calibration params (memory-management mm-26): the top-k nearest rules considered, and the distance
/// margin (relative to the response's NEAREST rule) within which an UNDECLARED rule is warned about.
/// Confirmed reasonable by the separation spike; tune on labelled data, never treat as sacred.
const WARN_K: usize = 8;
const WARN_MARGIN: f64 = 0.15;

/// Undeclared rules among the top-`k` nearest whose distance is within `margin` of the nearest rule's
/// distance — the skip warnings. Deterministic given fixed inputs. Empty inputs / k==0 -> no warnings.
#[must_use]
pub fn skip_warnings<S: BuildHasher>(
    distances: &[(String, f64)],
    declared: &HashSet<String, S>,
    k: usize,
    margin: f64,
) -> Vec<String> {
    if distances.is_empty() || k == 0 {
        return Vec::new();
    }
    let mut sorted: Vec<&(String, f64)> = distances.iter().collect();
    sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let nearest = sorted[0].1;
    sorted
        .iter()
        .take(k)
        .filter(|(key, dist)| !declared.contains(key) && *dist <= nearest + margin)
        .map(|(key, _)| key.clone())
        .collect()
}

/// Minimal JSON-RPC-over-stdio client for the memory server (store + recall). Killed on drop.
struct Server {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    id: i64,
}

impl Server {
    /// Spawn the shipped Node launcher (`node <launcher>`) in default stdio-MCP mode, pointed at an
    /// isolated `db` via `GENESIS_MEMORY_DB` so it never pollutes the agent's real store. The launcher
    /// resolves the cached server binary + ONNX model itself (a warm cache hit inside a genesis session).
    fn spawn(launcher: &Path, db: &Path) -> Option<Self> {
        let mut child = Command::new("node")
            .arg(launcher)
            .env("GENESIS_MEMORY_DB", db)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        Some(Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            id: 0,
        })
    }

    fn rpc(&mut self, method: &str, params: &Value) -> Option<Value> {
        self.id += 1;
        let req = json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params});
        writeln!(self.stdin, "{req}").ok()?;
        self.stdin.flush().ok()?;
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line).ok()? == 0 {
                return None;
            }
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(t).ok()?;
            if v.get("id").is_some() {
                return Some(v);
            }
        }
    }

    fn notify(&mut self, method: &str, params: &Value) {
        let req = json!({"jsonrpc":"2.0","method":method,"params":params});
        let _ = writeln!(self.stdin, "{req}");
        let _ = self.stdin.flush();
    }

    fn initialize(&mut self) -> Option<()> {
        self.rpc(
            "initialize",
            &json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"expertise-warn","version":"1"}}),
        )?;
        self.notify("notifications/initialized", &json!({}));
        Some(())
    }

    fn store(&mut self, agent: &str, text: &str) {
        let _ = self.rpc(
            "tools/call",
            &json!({"name":"store","arguments":{"agent_id":agent,"text":text}}),
        );
    }

    /// Recall `query` and return `(rule-key, distance)` pairs — the key is the `<name>#<id>` prefix the
    /// caller stored before ` :: `.
    fn recall(&mut self, agent: &str, query: &str, k: usize) -> Vec<(String, f64)> {
        let Some(res) = self.rpc(
            "tools/call",
            &json!({"name":"recall","arguments":{"agent_id":agent,"query":query,"k":k}}),
        ) else {
            return Vec::new();
        };
        let content = res
            .get("result")
            .and_then(|r| r.get("content"))
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|c| c.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("");
        parse_recall(content)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Parse the recall tool's JSON payload (`[{"text","score"},...]`) into `(rule-key, distance)`, where the
/// key is the `<name>#<id>` prefix before ` :: `. The memory server returns a SIMILARITY `score` where
/// HIGHER = nearer (verified against the shipped server: a TDD response scores its tdd rules ~1.45 vs an
/// unrelated rule ~0.45). `skip_warnings` works in DISTANCE space (lower = nearer), so we convert with
/// `distance = -score` — an order-preserving, margin-preserving flip. Tolerant: unparseable input -> empty.
fn parse_recall(content: &str) -> Vec<(String, f64)> {
    let Ok(arr) = serde_json::from_str::<Value>(content) else {
        return Vec::new();
    };
    arr.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| {
                    let text = e.get("text").and_then(Value::as_str)?;
                    let score = e.get("score").and_then(Value::as_f64)?;
                    let key = text.split(" :: ").next()?.trim().to_string();
                    if key.is_empty() {
                        None
                    } else {
                        Some((key, -score)) // higher similarity -> lower distance
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Render the advisory warnings file body for the given skipped rule keys.
#[must_use]
fn warnings_body(warns: &[String]) -> String {
    format!(
        "## Expertise skip-warning (advisory — NOT a block)\nYour last response looked related to these \
         rules you did NOT declare. If you applied one, cite it (with a verbatim rule quote) next turn; \
         if it truly didn't apply, ignore this:\n{}\n",
        warns
            .iter()
            .map(|w| format!("- {w}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// `genesis-hook expertise-warn --agent A --expertise E --launcher L [--main-agent N]` (the Stop-hook entry).
///
/// SPAWNER: reads the Stop event on stdin, DETACHES a background worker (which does the heavy embed), and
/// exits 0 immediately — zero added Stop latency, never blocks. The `--worker` form (re-exec'd by the
/// spawner, carrying `--transcript T`) does the actual skip-detection. FAIL-OPEN: any problem -> exit 0.
pub fn run(args: &[String]) {
    if args.iter().any(|a| a == "--worker") {
        run_worker(args);
        return;
    }
    // Spawner: pull the just-finished turn's transcript from the Stop event, then detach the worker.
    let ev = io::parse_event(&io::read_stdin());
    let transcript = ev
        .get("transcript_path")
        .and_then(Value::as_str)
        .unwrap_or("");
    if transcript.is_empty() {
        std::process::exit(0);
    }
    if let Ok(exe) = std::env::current_exe() {
        spawn_detached(&exe, &worker_argv(args, transcript));
    }
    std::process::exit(0);
}

/// The detached worker's argv: `expertise-warn --worker <base…> --transcript T`. Pure (unit-tested).
#[must_use]
fn worker_argv(base: &[String], transcript: &str) -> Vec<String> {
    let mut v = vec!["expertise-warn".to_string(), "--worker".to_string()];
    v.extend(base.iter().cloned());
    v.push("--transcript".to_string());
    v.push(transcript.to_string());
    v
}

/// Spawn `<exe> <args…>` fully DETACHED (own process group / detached process, all stdio to /dev/null) so
/// the background embed outlives the Stop hook process being reaped. Best-effort; a spawn failure is ignored.
#[cfg(unix)]
fn spawn_detached(exe: &Path, args: &[String]) {
    use std::os::unix::process::CommandExt;
    let mut c = Command::new(exe);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0); // new session/group -> survives the parent hook exiting
    let _ = c.spawn();
}

#[cfg(windows)]
fn spawn_detached(exe: &Path, args: &[String]) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut c = Command::new(exe);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    let _ = c.spawn();
}

/// The background WORKER: embed the just-finished turn's response + the agent's required rules, find rules
/// near the response that were NOT declared, and write them to `<repo>/.genesis/expertise-warnings.md` for
/// the next SessionStart (inject) to surface. FAIL-OPEN: any problem -> no file (clear stale), exit 0.
fn run_worker(args: &[String]) {
    let get = |flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    // The worker inherits the Stop hook's cwd (the project root), so --repo defaults to it.
    let repo = get("--repo").map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let (Some(agent), Some(exp), Some(launcher), Some(tp)) = (
        get("--agent"),
        get("--expertise"),
        get("--launcher"),
        get("--transcript"),
    ) else {
        std::process::exit(0);
    };

    let required = validate::required_for(Some(&Path::new(&exp).join("required.json")), &agent);
    if required.is_empty() {
        std::process::exit(0);
    }
    let manifest_dir = Path::new(&exp).join("manifests");
    let mut rules: Vec<(String, String)> = Vec::new();
    for name in &required {
        for (id, text) in validate::manifest_rule_texts(Some(&manifest_dir), name) {
            rules.push((format!("{name}#{id}"), text));
        }
    }
    if rules.is_empty() {
        std::process::exit(0);
    }

    let response = validate::current_turn_visible_text(&tp);
    if response.trim().is_empty() {
        std::process::exit(0);
    }
    let declared: HashSet<String> = match validate::parse_declarations(&tp) {
        Some((decls, _)) => decls
            .iter()
            .flat_map(|(name, es)| es.iter().map(move |(rid, _)| format!("{name}#{rid}")))
            .collect(),
        None => HashSet::new(),
    };

    let db = std::env::temp_dir().join(format!("genesis-ewarn-{}.db", std::process::id()));
    let Some(mut srv) = Server::spawn(Path::new(&launcher), &db) else {
        std::process::exit(0);
    };
    if srv.initialize().is_none() {
        let _ = std::fs::remove_file(&db);
        std::process::exit(0);
    }
    for (key, text) in &rules {
        srv.store("ewarn", &format!("{key} :: {text}"));
    }
    let dists = srv.recall("ewarn", &response, rules.len());
    drop(srv);
    let _ = std::fs::remove_file(&db);

    let warns = skip_warnings(&dists, &declared, WARN_K, WARN_MARGIN);
    let out = repo.join(".genesis").join("expertise-warnings.md");
    if warns.is_empty() {
        let _ = std::fs::remove_file(&out); // clear any stale warning
        std::process::exit(0);
    }
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&out, warnings_body(&warns));
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn flags_close_undeclared_within_topk_and_margin() {
        let dists = vec![
            ("tdd-1".to_string(), 0.90f64),
            ("tdd-2".to_string(), 0.95),
            ("mm-1".to_string(), 1.30),
            ("som-3".to_string(), 1.40),
        ];
        // nearest=0.90, margin 0.10 -> within 1.00: tdd-1 (declared, skip), tdd-2 (0.95, undeclared -> WARN);
        // mm-1/som-3 outside the margin -> no warn.
        let w = skip_warnings(&dists, &set(&["tdd-1"]), 3, 0.10);
        assert_eq!(w, vec!["tdd-2".to_string()]);
    }

    #[test]
    fn empty_when_all_close_rules_declared() {
        let dists = vec![("a".to_string(), 0.5f64), ("b".to_string(), 0.55)];
        assert!(skip_warnings(&dists, &set(&["a", "b"]), 5, 0.2).is_empty());
    }

    #[test]
    fn respects_topk_and_margin_bounds() {
        let dists = vec![
            ("a".to_string(), 0.5f64),
            ("b".to_string(), 0.6),
            ("c".to_string(), 0.65),
        ];
        // margin 0 -> only the nearest (a) qualifies, even with a large k.
        assert_eq!(
            skip_warnings(&dists, &set(&[]), 9, 0.0),
            vec!["a".to_string()]
        );
    }

    #[test]
    fn empty_inputs_no_warnings() {
        assert!(skip_warnings(&[], &set(&[]), 3, 0.1).is_empty());
        let dists = vec![("a".to_string(), 0.5f64)];
        assert!(skip_warnings(&dists, &set(&[]), 0, 0.1).is_empty());
    }

    #[test]
    fn worker_argv_prepends_worker_and_appends_transcript() {
        // The spawner re-execs itself as the detached worker: `expertise-warn --worker <base…> --transcript T`.
        let base: Vec<String> = ["--agent", "bot", "--launcher", "/l.js", "--expertise", "/e"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let a = worker_argv(&base, "/t.jsonl");
        assert_eq!(a[0], "expertise-warn");
        assert_eq!(a[1], "--worker");
        assert!(a
            .windows(2)
            .any(|w| w[0] == "--transcript" && w[1] == "/t.jsonl"));
        assert!(a.contains(&"--agent".to_string()) && a.contains(&"bot".to_string()));
        assert!(a.contains(&"/l.js".to_string()));
    }

    #[test]
    fn parse_recall_extracts_key_and_distance() {
        // The server returns a SIMILARITY `score` (higher = nearer); parse_recall flips it to a distance
        // (lower = nearer) via negation, and keys off the `<name>#<id>` prefix before ` :: `.
        let content = r#"[{"text":"test-driven-determinism#tdd-2 :: verify red first","score":1.45},{"text":"memory-management#mm-1 :: model memory as a lifecycle","score":0.45}]"#;
        let d = parse_recall(content);
        assert_eq!(
            d,
            vec![
                ("test-driven-determinism#tdd-2".to_string(), -1.45f64),
                ("memory-management#mm-1".to_string(), -0.45f64),
            ]
        );
        // the nearer (higher-score) rule has the SMALLER distance
        assert!(d[0].1 < d[1].1);
        assert!(parse_recall("not json").is_empty());
    }

    #[test]
    fn warnings_body_lists_keys_as_advisory() {
        let b = warnings_body(&["a#a-1".to_string(), "b#b-2".to_string()]);
        assert!(b.contains("- a#a-1") && b.contains("- b#b-2"));
        assert!(b.contains("advisory") && b.contains("NOT a block"));
    }
}
