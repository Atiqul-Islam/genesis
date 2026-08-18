//! Entry point for the Genesis memory server — MCP over stdio (plus export/import subcommands).
//!
//! Thin binary: all wiring lives in [`genesis_memory::run`], which dispatches the
//! `export`/`import` migration subcommands or the stdio MCP server. Keeping `main` trivial
//! means there is nothing here to unit-test (the reference Rust components deliberately ship
//! no `tests/test_main.rs`); behaviour is covered by the library unit tests, the cucumber BDD
//! suites, and the stdio integration tests.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    genesis_memory::run().await
}
