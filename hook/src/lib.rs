//! Genesis enforcement hooks (Rust).
//!
//! A faithful, dependency-light port of the Node `hooks/*.js` set into ONE self-contained binary
//! dispatched by its first argument. Each hook reads the Claude Code event JSON on stdin and emits a
//! decision object on stdout, always exiting 0 (the decision travels in the JSON, never the exit
//! code — verified against the deployed Node hooks and the official hooks reference).
//!
//! Behavior parity with the Node hooks is the contract; the only intentional change is `validate`'s
//! traversal, which prunes heavy build dirs (semantics-preserving) so it no longer walks node_modules
//! / target on a large tree.

pub mod agent;
pub mod capture_session;
pub mod cli;
pub mod enforce_research;
pub mod expertise_db;
pub mod expertise_warn;
pub mod gate;
pub mod glob;
pub mod inject;
pub mod io;
pub mod precompact;
pub mod promote_offer;
pub mod session_pointer;
pub mod session_transfer;
pub mod validate;
