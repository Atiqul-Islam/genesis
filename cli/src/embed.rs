//! `genesis-cli embed` — session-copy Phase 2b (port of embed.js / embed.py).
//!
//! Feeds each scrubbed record into the Genesis memory server under the new agent's `agent_id`, via the
//! server's `store` MCP tool over stdio — the SAME protocol the agent uses at runtime, so afterwards the
//! agent's `recall` returns the relevant slices of its copied history. An INTEGRATION step: it needs the
//! built server binary + the ONNX model, so it is kept out of the pure `store` module.

use crate::fsx;
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// Load records from a `records.jsonl` file (skipping blank / unparseable lines).
#[must_use]
pub fn load_from_jsonl(p: &Path) -> Vec<Value> {
    let Some(raw) = std::fs::read(p)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
    else {
        return Vec::new();
    };
    raw.split('\n')
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() {
                None
            } else {
                serde_json::from_str::<Value>(t).ok()
            }
        })
        .collect()
}

/// Load records from a `history.sqlite` DB (`records` table, ordered by seq).
///
/// # Errors
/// Returns a message if the DB cannot be opened or queried.
pub fn load_from_db(p: &Path) -> Result<Vec<Value>, String> {
    let con = Connection::open_with_flags(p, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("open {}: {e}", p.display()))?;
    let mut stmt = con
        .prepare("SELECT source, kind, title, text FROM records ORDER BY seq")
        .map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "source": row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                "kind": row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                "title": row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                "text": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            }))
        })
        .map_err(|e| format!("query: {e}"))?;
    Ok(rows.flatten().collect())
}

/// Give each stored memory light provenance so a recalled chunk is self-describing.
#[must_use]
pub fn record_text(r: &Value) -> String {
    let f = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("");
    let (src, title, text) = (f("source"), f("title"), f("text"));
    let head = if title.is_empty() {
        format!("[{src}] ")
    } else {
        format!("[{src}] {title}: ")
    };
    format!("{head}{text}").trim().to_string()
}

fn head_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// A minimal JSON-RPC-over-stdio client for the memory server. Killed on drop.
struct Server {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    id: i64,
}

impl Server {
    fn spawn(server_bin: &Path, model_dir: &Path, db_path: &Path) -> Result<Self, String> {
        let mut child = Command::new(server_bin)
            .env("GENESIS_MODEL_DIR", model_dir)
            .env("GENESIS_MEMORY_DB", db_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("spawn {}: {e}", server_bin.display()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "server has no stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "server has no stdout".to_string())?;
        Ok(Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            id: 0,
        })
    }

    /// Read the next JSON-RPC RESPONSE line (skipping any notifications, which carry no `id`).
    fn read_response(&mut self) -> Result<Value, String> {
        loop {
            let mut line = String::new();
            let n = self
                .reader
                .read_line(&mut line)
                .map_err(|e| format!("read: {e}"))?;
            if n == 0 {
                return Err("server closed stdout".to_string());
            }
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            let v: Value =
                serde_json::from_str(t).map_err(|e| format!("bad json from server: {e}"))?;
            if v.get("id").is_some() {
                return Ok(v);
            }
        }
    }

    fn write_msg(&mut self, msg: &Value) -> Result<(), String> {
        writeln!(
            self.stdin,
            "{}",
            serde_json::to_string(msg).unwrap_or_default()
        )
        .map_err(|e| format!("write: {e}"))?;
        self.stdin.flush().ok();
        Ok(())
    }

    fn rpc(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.id += 1;
        // Build via a Map so `params` is moved in (not borrowed) — no needless clone.
        let mut req = serde_json::Map::new();
        req.insert("jsonrpc".to_string(), json!("2.0"));
        req.insert("id".to_string(), json!(self.id));
        req.insert("method".to_string(), json!(method));
        req.insert("params".to_string(), params);
        self.write_msg(&Value::Object(req))?;
        self.read_response()
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        let mut req = serde_json::Map::new();
        req.insert("jsonrpc".to_string(), json!("2.0"));
        req.insert("method".to_string(), json!(method));
        req.insert("params".to_string(), params);
        self.write_msg(&Value::Object(req))
    }

    fn initialize(&mut self) -> Result<(), String> {
        self.rpc(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "session-copy-embed", "version": "1"},
            }),
        )?;
        self.notify("notifications/initialized", json!({}))
    }

    /// Store `text` under `agent_id`. True on success (the tool result is not an error).
    fn store(&mut self, agent_id: &str, text: &str) -> bool {
        match self.rpc(
            "tools/call",
            json!({"name": "store", "arguments": {"agent_id": agent_id, "text": text}}),
        ) {
            Ok(r) => !r
                .get("result")
                .and_then(|res| res.get("isError"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            Err(_) => false,
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Embed `records` into the server under `agent_id`. Returns a `{stored,failed,skipped,total,...}` manifest.
///
/// # Errors
/// Returns a message if the server cannot be spawned or the handshake fails.
pub fn embed_records(
    records: &[Value],
    agent_id: &str,
    server_bin: &Path,
    model_dir: &Path,
    db_path: &Path,
    max_chars: usize,
) -> Result<Value, String> {
    let abs = if db_path.is_absolute() {
        db_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("cwd: {e}"))?
            .join(db_path)
    };
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let mut srv = Server::spawn(server_bin, model_dir, db_path)?;
    srv.initialize()?;
    let (mut stored, mut failed, mut skipped) = (0u64, 0u64, 0u64);
    for r in records {
        let mut text = record_text(r);
        if text.trim().is_empty() {
            skipped += 1;
            continue;
        }
        if text.chars().count() > max_chars {
            text = head_chars(&text, max_chars);
        }
        if srv.store(agent_id, &text) {
            stored += 1;
        } else {
            failed += 1;
        }
    }
    Ok(json!({
        "agent_id": agent_id,
        "db": db_path.to_string_lossy(),
        "stored": stored,
        "failed": failed,
        "skipped": skipped,
        "total": records.len(),
    }))
}

/// Default single-record cap (recall works on the head of a huge blob).
pub const DEFAULT_MAX_CHARS: usize = 6000;

/// Entry point for `genesis-cli embed`. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let (mut records, mut history_db, mut agent, mut server_bin, mut model_dir, mut db) =
        (None, None, None, None, None, None);
    let mut i = 0;
    while i + 1 < args.len() {
        match args[i].as_str() {
            "--records" => records = args.get(i + 1).cloned(),
            "--history-db" => history_db = args.get(i + 1).cloned(),
            "--agent-id" => agent = args.get(i + 1).cloned(),
            "--server-bin" => server_bin = args.get(i + 1).cloned(),
            "--model-dir" => model_dir = args.get(i + 1).cloned(),
            "--db" => db = args.get(i + 1).cloned(),
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    if records.is_some() == history_db.is_some() {
        fsx::fail("error: exactly one of --records / --history-db is required");
    }
    let (Some(agent), Some(server_bin), Some(model_dir), Some(db)) =
        (agent, server_bin, model_dir, db)
    else {
        fsx::fail("usage: genesis-cli embed (--records <f> | --history-db <f>) --agent-id <name> --server-bin <bin> --model-dir <dir> --db <path>");
    };
    let recs = match (records, history_db) {
        (Some(r), _) => load_from_jsonl(Path::new(&r)),
        (_, Some(h)) => match load_from_db(Path::new(&h)) {
            Ok(v) => v,
            Err(e) => fsx::fail(&e),
        },
        _ => Vec::new(),
    };
    match embed_records(
        &recs,
        &agent,
        Path::new(&server_bin),
        Path::new(&model_dir),
        Path::new(&db),
        DEFAULT_MAX_CHARS,
    ) {
        Ok(m) => {
            println!("{}", fsx::json_pretty(&m));
            0
        }
        Err(e) => fsx::fail(&e),
    }
}
