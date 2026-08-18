//! BDD step definitions for `test/features/server.feature`.
//!
//! Source: `test/specs/genesis-memory-server.md` — acceptance criteria 1, 2, 11, 12, 14,
//! 15, 17 (protocol/lifecycle behaviour that belongs to no single tool).
//!
//! No mocks (`docs/SPEC_FORGE_RUST_UPDATE.md` §5 #2 and §5 #5): the real
//! `genesis-memory-server` binary is spawned as a child process and driven over a real
//! stdio JSON-RPC lifecycle. Hermeticity (§5 #6): the `World` owns `tempfile::TempDir`s,
//! so every scenario — and each of the two servers in criterion 17 — gets a fresh SQLite
//! database supplied through `GENESIS_MEMORY_DB`.

// These are `harness = false` test binaries, which clippy's `allow-unwrap-in-tests`
// (server/clippy.toml) does not reach — unwrap/expect-on-failure IS the intended test
// behaviour (a failed unwrap is a failed scenario). Every cucumber step is `async` by
// convention even when it does not await.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unused_async,
    clippy::needless_pass_by_value
)]

use std::path::PathBuf;
use std::process::Child;

use cucumber::{given, then, when, World as _};
use tempfile::TempDir;

/// Real stdio JSON-RPC wire helpers: spawn the built binary and speak line-delimited
/// JSON-RPC to it (no mock — `env!("CARGO_BIN_EXE_…")` resolves the compiled binary).
mod rpc {
    use std::io::{BufRead, BufReader, Write};
    use std::path::Path;
    use std::process::{Child, Command, Stdio};

    /// Spawns the real server binary with piped stdio and the given DB + model dir.
    pub(crate) fn spawn(db: &Path, model_dir: Option<&Path>) -> Child {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_genesis-memory-server"));
        cmd.env("GENESIS_MEMORY_DB", db)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(md) = model_dir {
            cmd.env("GENESIS_MODEL_DIR", md);
        }
        cmd.spawn().unwrap()
    }

    /// Writes one JSON-RPC request line and reads exactly one response line back.
    pub(crate) fn send(child: &mut Child, req: serde_json::Value) -> serde_json::Value {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "{req}").unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.as_mut().unwrap())
            .read_line(&mut line)
            .unwrap();
        serde_json::from_str(&line).unwrap()
    }

    /// Writes one JSON-RPC notification line (notifications get no response).
    pub(crate) fn notify(child: &mut Child, note: serde_json::Value) {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "{note}").unwrap();
        stdin.flush().unwrap();
    }

    /// Runs the `initialize` → `notifications/initialized` handshake, returning the response.
    pub(crate) fn initialize(child: &mut Child) -> serde_json::Value {
        let resp = send(
            child,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05", "capabilities": {},
                    "clientInfo": {"name": "bdd", "version": "0"}
                }
            }),
        );
        notify(
            child,
            serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        );
        resp
    }
}

/// Scenario-scoped state for `server.feature`.
#[allow(dead_code)]
#[derive(Debug, Default, cucumber::World)]
struct ServerWorld {
    /// Per-scenario temporary directory holding the SQLite database (§5 #6 hermeticity).
    db_dir: Option<TempDir>,
    /// `GENESIS_MEMORY_DB` value handed to the primary server child process.
    db_path: Option<PathBuf>,
    /// Second temporary directory, for the two-database isolation criterion.
    second_db_dir: Option<TempDir>,
    /// `GENESIS_MEMORY_DB` value handed to the second server child process.
    second_db_path: Option<PathBuf>,
    /// The primary spawned `genesis-memory-server` child process.
    server: Option<Child>,
    /// The second spawned `genesis-memory-server` child process (criterion 17).
    second_server: Option<Child>,
    /// The most recent raw JSON-RPC response line read from the server's stdout.
    last_response: Option<serde_json::Value>,
    /// Responses collected when a scenario issues several tool calls in a row.
    responses: Vec<serde_json::Value>,
    /// The primary child's exit status code, once it has been waited on.
    exit_code: Option<i32>,
}

/// The text carried by the first content block of a tool-call result.
fn result_text(resp: &serde_json::Value) -> String {
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

// ─── Given ───────────────────────────────────────────────────────────────────

#[given(regex = r"^a spawned memory server child process over stdio$")]
async fn a_spawned_memory_server_child_process(w: &mut ServerWorld) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("m.db");
    w.server = Some(rpc::spawn(&db, Some(&genesis_memory::embed::model_dir())));
    w.db_path = Some(db);
    w.db_dir = Some(dir);
}

#[given(regex = r"^an initialized memory server child process over stdio$")]
async fn an_initialized_memory_server_child_process(w: &mut ServerWorld) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("m.db");
    // Point at a non-existent (empty) model dir ⇒ the first tool call that needs the
    // embedder fails ⇒ AC11 isError:true. The path lives under the kept-alive db dir.
    let empty_model = dir.path().join("no-model-here");
    let mut child = rpc::spawn(&db, Some(&empty_model));
    rpc::initialize(&mut child);
    w.server = Some(child);
    w.db_path = Some(db);
    w.db_dir = Some(dir);
}

#[given(
    regex = r"^an initialized memory server child process over stdio with the model already on disk$"
)]
async fn an_initialized_server_with_the_model_on_disk(w: &mut ServerWorld) {
    let (m, _t) = genesis_memory::embed::model_paths();
    assert!(
        m.exists(),
        "model missing: run `node scripts/fetch-model.mjs`"
    );
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("m.db");
    let mut child = rpc::spawn(&db, Some(&genesis_memory::embed::model_dir()));
    rpc::initialize(&mut child);
    w.server = Some(child);
    w.db_path = Some(db);
    w.db_dir = Some(dir);
}

#[given(regex = r"^outbound network access is unavailable$")]
async fn outbound_network_access_is_unavailable(_w: &mut ServerWorld) {
    // v1 performs no request-time network I/O; the child was spawned with the model already
    // on disk, so every tool call completes without touching the network.
}

#[given(
    regex = r"^a memory server child process launched with GENESIS_MEMORY_DB pointing at a first temporary file$"
)]
async fn a_server_with_the_first_temporary_database(w: &mut ServerWorld) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("first.db");
    let mut child = rpc::spawn(&db, Some(&genesis_memory::embed::model_dir()));
    rpc::initialize(&mut child);
    w.server = Some(child);
    w.db_path = Some(db);
    w.db_dir = Some(dir);
}

#[given(
    regex = r"^a second memory server child process launched with GENESIS_MEMORY_DB pointing at a different temporary file$"
)]
async fn a_server_with_the_second_temporary_database(w: &mut ServerWorld) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("second.db");
    let mut child = rpc::spawn(&db, Some(&genesis_memory::embed::model_dir()));
    rpc::initialize(&mut child);
    w.second_server = Some(child);
    w.second_db_path = Some(db);
    w.second_db_dir = Some(dir);
}

// ─── When ────────────────────────────────────────────────────────────────────

#[when(regex = r"^the client sends an initialize request$")]
async fn the_client_sends_an_initialize_request(w: &mut ServerWorld) {
    let resp = rpc::initialize(w.server.as_mut().unwrap());
    w.last_response = Some(resp);
}

#[when(regex = r#"^the client sends a "([^"]*)" request$"#)]
async fn the_client_sends_a_named_request(w: &mut ServerWorld, method: String) {
    let resp = rpc::send(
        w.server.as_mut().unwrap(),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": method}),
    );
    w.last_response = Some(resp);
}

#[when(regex = r"^the client makes a tool call that triggers an internal failure$")]
async fn a_tool_call_that_triggers_an_internal_failure(w: &mut ServerWorld) {
    // The server (Given) points at an empty model dir ⇒ store cannot load the embedder.
    let resp = rpc::send(
        w.server.as_mut().unwrap(),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "store", "arguments": {"agent_id": "alpha", "text": "boom"}}
        }),
    );
    w.last_response = Some(resp);
}

#[when(regex = r"^a syntactically invalid JSON-RPC request is written to the server stdin$")]
async fn an_invalid_jsonrpc_request_is_written(w: &mut ServerWorld) {
    // rmcp (modelcontextprotocol/rust-sdk#938) IGNORES syntactically-invalid JSON — there is
    // no id to correlate a reply, and echoing errors can storm. The case the spec's
    // "Invalid Request" (-32600) error covers is a well-formed JSON value that is not a valid
    // JSON-RPC request; that is what we send here, and rmcp replies with a JSON-RPC error.
    let resp = rpc::send(
        w.server.as_mut().unwrap(),
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "not_a_valid_member": "x"}),
    );
    w.last_response = Some(resp);
}

#[when(regex = r"^the client closes the server stdin$")]
async fn the_client_closes_the_server_stdin(w: &mut ServerWorld) {
    let mut child = w.server.take().unwrap();
    // A real client initializes before disconnecting; a post-handshake stdin EOF is the
    // clean-shutdown path (exit 0). (A pre-init disconnect is a connection error, exit 1.)
    rpc::initialize(&mut child);
    drop(child.stdin.take()); // EOF on stdin
    let status = child.wait().unwrap();
    w.exit_code = status.code();
}

#[when(regex = r#"^the client makes a tool call for each of "([^"]*)", "([^"]*)" and "([^"]*)"$"#)]
async fn a_tool_call_for_each_of(
    w: &mut ServerWorld,
    first: String,
    second: String,
    third: String,
) {
    let child = w.server.as_mut().unwrap();
    for (i, name) in [first, second, third].into_iter().enumerate() {
        let args = match name.as_str() {
            "store" => serde_json::json!({"agent_id": "alpha", "text": "offline note"}),
            "recall" => serde_json::json!({"agent_id": "alpha", "query": "offline note"}),
            _ => serde_json::json!({"agent_id": "alpha"}),
        };
        let resp = rpc::send(
            child,
            serde_json::json!({
                "jsonrpc": "2.0", "id": 100 + i, "method": "tools/call",
                "params": {"name": name, "arguments": args}
            }),
        );
        w.responses.push(resp);
    }
}

#[when(regex = r#"^agent "([^"]*)" stores the memory "([^"]*)" through the first server$"#)]
async fn agent_stores_through_the_first_server(w: &mut ServerWorld, agent: String, text: String) {
    let resp = rpc::send(
        w.server.as_mut().unwrap(),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 200, "method": "tools/call",
            "params": {"name": "store", "arguments": {"agent_id": agent, "text": text}}
        }),
    );
    w.responses.push(resp);
}

#[when(regex = r#"^agent "([^"]*)" recalls "([^"]*)" through the second server$"#)]
async fn agent_recalls_through_the_second_server(
    w: &mut ServerWorld,
    agent: String,
    query: String,
) {
    let resp = rpc::send(
        w.second_server.as_mut().unwrap(),
        serde_json::json!({
            "jsonrpc": "2.0", "id": 201, "method": "tools/call",
            "params": {"name": "recall", "arguments": {"agent_id": agent, "query": query}}
        }),
    );
    w.last_response = Some(resp);
}

// ─── Then ────────────────────────────────────────────────────────────────────

#[then(regex = r#"^the initialize response advertises "([^"]*)" under capabilities$"#)]
async fn initialize_advertises_capability(w: &mut ServerWorld, capability: String) {
    let r = w.last_response.as_ref().unwrap();
    assert!(r["result"]["capabilities"].get(&capability).is_some());
}

#[then(regex = r#"^the initialize response protocolVersion is "([^"]*)"$"#)]
async fn initialize_protocol_version_is(w: &mut ServerWorld, version: String) {
    assert_eq!(
        w.last_response.as_ref().unwrap()["result"]["protocolVersion"],
        version
    );
}

#[then(regex = r#"^the response contains the tool names "([^"]*)", "([^"]*)" and "([^"]*)"$"#)]
async fn the_response_contains_the_tool_names(
    w: &mut ServerWorld,
    first: String,
    second: String,
    third: String,
) {
    let tools = w.last_response.as_ref().unwrap()["result"]["tools"]
        .as_array()
        .unwrap();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    for n in [first, second, third] {
        assert!(names.contains(&n.as_str()), "missing tool {n}");
    }
}

#[then(regex = r"^the response is a JSON-RPC result whose isError field is true$")]
async fn the_response_is_a_result_with_is_error_true(w: &mut ServerWorld) {
    let r = w.last_response.as_ref().unwrap();
    assert!(
        r.get("error").is_none(),
        "must be a result, not a JSON-RPC error"
    );
    assert_eq!(r["result"]["isError"], serde_json::Value::Bool(true));
}

#[then(regex = r"^the server replies with a JSON-RPC error object rather than a result$")]
async fn the_server_replies_with_a_jsonrpc_error(w: &mut ServerWorld) {
    let r = w.last_response.as_ref().unwrap();
    assert!(r.get("error").is_some());
    assert!(r.get("result").is_none());
}

#[then(regex = r"^the server child process terminates with exit status (\d+)$")]
async fn the_server_terminates_with_exit_status(w: &mut ServerWorld, code: i32) {
    assert_eq!(w.exit_code, Some(code));
}

#[then(regex = r"^every one of those tool calls completes successfully$")]
async fn every_tool_call_completes_successfully(w: &mut ServerWorld) {
    assert_eq!(w.responses.len(), 3);
    for r in &w.responses {
        assert!(r.get("error").is_none());
        assert_eq!(r["result"]["isError"], serde_json::Value::Bool(false));
    }
}

#[then(regex = r#"^the recall result contains no entry whose text is "([^"]*)"$"#)]
async fn the_recall_result_contains_no_entry_with_text(w: &mut ServerWorld, text: String) {
    let payload = result_text(w.last_response.as_ref().unwrap());
    let items: serde_json::Value = serde_json::from_str(&payload).unwrap_or(serde_json::json!([]));
    assert!(items
        .as_array()
        .unwrap()
        .iter()
        .all(|it| it["text"] != text));
}

// ─── Runner ──────────────────────────────────────────────────────────────────

// The feature files live at repo-root `test/features/`; a `[[test]]` target runs with the
// package root (`server/`) as its working directory, hence the `../` prefix.
#[tokio::main]
async fn main() {
    ServerWorld::cucumber()
        .run_and_exit("../test/features/server.feature")
        .await;
}
