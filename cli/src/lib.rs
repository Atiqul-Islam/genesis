//! Genesis installer / orchestrator (Rust).
//!
//! The one-time build+wire layer — `assemble`, `bootstrap`, `promote`, `install`, `build-plugin-agents` —
//! plus the session-copy pipeline (`capture`, `store`, `embed`, `build-session-agent`), all ported from the
//! Node installer + session_copy scripts. Runs after the Node fetch-launcher has staged the native
//! binaries; it never runs per tool call, so it lives in its own binary rather than bloating the hot-path
//! `genesis-hook`.

pub mod assemble;
pub mod bootstrap;
pub mod build_plugin_agents;
pub mod build_session_agent;
pub mod capture;
pub mod demote;
pub mod doctor;
pub mod embed;
pub mod fix;
pub mod fsx;
pub mod install;
pub mod memfix;
pub mod promote;
pub mod reconcile;
pub mod render;
pub mod scrub;
pub mod store;
