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
//!
//! Every step body is `unimplemented!()` — it compiles (crate sets clippy
//! `unimplemented = "warn"`) and panics at runtime, which is the healthy RED state.

use std::path::PathBuf;
use std::process::Child;

use cucumber::{World as _, given, then, when};
use tempfile::TempDir;

/// Scenario-scoped state for `server.feature`.
// Fields are populated only once the steps are implemented via TDD; at the RED stage
// they are write-only scaffold state.
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

// ─── Given ───────────────────────────────────────────────────────────────────

#[given(regex = r"^a spawned memory server child process over stdio$")]
async fn a_spawned_memory_server_child_process(_w: &mut ServerWorld) {
    // TODO: spawn the real binary with GENESIS_MEMORY_DB pointing into a fresh TempDir,
    // with piped stdin/stdout — no initialize handshake yet.
    unimplemented!("Implement via TDD — a spawned memory server child process over stdio");
}

#[given(regex = r"^an initialized memory server child process over stdio$")]
async fn an_initialized_memory_server_child_process(_w: &mut ServerWorld) {
    // TODO: spawn the real binary and complete initialize → notifications/initialized.
    unimplemented!("Implement via TDD — an initialized memory server child process over stdio");
}

#[given(
    regex = r"^an initialized memory server child process over stdio with the model already on disk$"
)]
async fn an_initialized_server_with_the_model_on_disk(_w: &mut ServerWorld) {
    // TODO: assert the pinned-SHA-256 model file is already present, then spawn + initialize.
    unimplemented!("Implement via TDD — an initialized memory server child process over stdio with the model already on disk");
}

#[given(regex = r"^outbound network access is unavailable$")]
async fn outbound_network_access_is_unavailable(_w: &mut ServerWorld) {
    // TODO: run the remainder of the scenario with outbound network access unavailable.
    unimplemented!("Implement via TDD — outbound network access is unavailable");
}

#[given(
    regex = r"^a memory server child process launched with GENESIS_MEMORY_DB pointing at a first temporary file$"
)]
async fn a_server_with_the_first_temporary_database(_w: &mut ServerWorld) {
    // TODO: spawn server #1 with GENESIS_MEMORY_DB set to a file in `db_dir`.
    unimplemented!("Implement via TDD — a memory server child process launched with GENESIS_MEMORY_DB pointing at a first temporary file");
}

#[given(
    regex = r"^a second memory server child process launched with GENESIS_MEMORY_DB pointing at a different temporary file$"
)]
async fn a_server_with_the_second_temporary_database(_w: &mut ServerWorld) {
    // TODO: spawn server #2 with GENESIS_MEMORY_DB set to a file in `second_db_dir`.
    unimplemented!("Implement via TDD — a second memory server child process launched with GENESIS_MEMORY_DB pointing at a different temporary file");
}

// ─── When ────────────────────────────────────────────────────────────────────

#[when(regex = r"^the client sends an initialize request$")]
async fn the_client_sends_an_initialize_request(_w: &mut ServerWorld) {
    unimplemented!("Implement via TDD — the client sends an initialize request");
}

#[when(regex = r#"^the client sends a "([^"]*)" request$"#)]
async fn the_client_sends_a_named_request(_w: &mut ServerWorld, _method: String) {
    unimplemented!("Implement via TDD — the client sends the named JSON-RPC request");
}

#[when(regex = r"^the client makes a tool call that triggers an internal failure$")]
async fn a_tool_call_that_triggers_an_internal_failure(_w: &mut ServerWorld) {
    unimplemented!("Implement via TDD — the client makes a tool call that triggers an internal failure");
}

#[when(regex = r"^a syntactically invalid JSON-RPC request is written to the server stdin$")]
async fn an_invalid_jsonrpc_request_is_written(_w: &mut ServerWorld) {
    unimplemented!("Implement via TDD — a syntactically invalid JSON-RPC request is written to the server stdin");
}

#[when(regex = r"^the client closes the server stdin$")]
async fn the_client_closes_the_server_stdin(_w: &mut ServerWorld) {
    unimplemented!("Implement via TDD — the client closes the server stdin");
}

#[when(regex = r#"^the client makes a tool call for each of "([^"]*)", "([^"]*)" and "([^"]*)"$"#)]
async fn a_tool_call_for_each_of(
    _w: &mut ServerWorld,
    _first: String,
    _second: String,
    _third: String,
) {
    unimplemented!("Implement via TDD — the client makes a tool call for each of the three tools");
}

#[when(regex = r#"^agent "([^"]*)" stores the memory "([^"]*)" through the first server$"#)]
async fn agent_stores_through_the_first_server(
    _w: &mut ServerWorld,
    _agent: String,
    _text: String,
) {
    unimplemented!("Implement via TDD — agent stores the memory through the first server");
}

#[when(regex = r#"^agent "([^"]*)" recalls "([^"]*)" through the second server$"#)]
async fn agent_recalls_through_the_second_server(
    _w: &mut ServerWorld,
    _agent: String,
    _query: String,
) {
    unimplemented!("Implement via TDD — agent recalls through the second server");
}

// ─── Then ────────────────────────────────────────────────────────────────────

#[then(regex = r#"^the initialize response advertises "([^"]*)" under capabilities$"#)]
async fn initialize_advertises_capability(_w: &mut ServerWorld, _capability: String) {
    unimplemented!("Implement via TDD — the initialize response advertises the capability");
}

#[then(regex = r#"^the initialize response protocolVersion is "([^"]*)"$"#)]
async fn initialize_protocol_version_is(_w: &mut ServerWorld, _version: String) {
    unimplemented!("Implement via TDD — the initialize response protocolVersion is the pinned value");
}

#[then(regex = r#"^the response contains the tool names "([^"]*)", "([^"]*)" and "([^"]*)"$"#)]
async fn the_response_contains_the_tool_names(
    _w: &mut ServerWorld,
    _first: String,
    _second: String,
    _third: String,
) {
    unimplemented!("Implement via TDD — the response contains the three tool names");
}

#[then(regex = r"^the response is a JSON-RPC result whose isError field is true$")]
async fn the_response_is_a_result_with_is_error_true(_w: &mut ServerWorld) {
    unimplemented!("Implement via TDD — the response is a JSON-RPC result whose isError field is true");
}

#[then(regex = r"^the server replies with a JSON-RPC error object rather than a result$")]
async fn the_server_replies_with_a_jsonrpc_error(_w: &mut ServerWorld) {
    unimplemented!("Implement via TDD — the server replies with a JSON-RPC error object rather than a result");
}

#[then(regex = r"^the server child process terminates with exit status (\d+)$")]
async fn the_server_terminates_with_exit_status(_w: &mut ServerWorld, _code: i32) {
    unimplemented!("Implement via TDD — the server child process terminates with the given exit status");
}

#[then(regex = r"^every one of those tool calls completes successfully$")]
async fn every_tool_call_completes_successfully(_w: &mut ServerWorld) {
    unimplemented!("Implement via TDD — every one of those tool calls completes successfully");
}

#[then(regex = r#"^the recall result contains no entry whose text is "([^"]*)"$"#)]
async fn the_recall_result_contains_no_entry_with_text(_w: &mut ServerWorld, _text: String) {
    unimplemented!("Implement via TDD — the recall result contains no entry whose text is the given value");
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
