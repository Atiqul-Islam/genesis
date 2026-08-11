//! genesis-cli — Genesis installer/orchestrator, busybox-style subcommand dispatch.
//!
//! `genesis-cli <assemble|bootstrap|promote|demote|doctor|fix|reconcile|install|build-plugin-agents|
//! capture|store|embed|build-session-agent> [args...]`.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sub = args.get(1).map_or("", String::as_str);
    let rest: Vec<String> = args.iter().skip(2).cloned().collect();
    let code = match sub {
        "assemble" => genesis_cli::assemble::run(&rest),
        "promote" => genesis_cli::promote::run(&rest),
        "demote" => genesis_cli::demote::run(&rest),
        "doctor" => genesis_cli::doctor::run(&rest),
        "fix" => genesis_cli::fix::run(&rest),
        "reconcile" => genesis_cli::reconcile::run(&rest),
        "install" => genesis_cli::install::run(&rest),
        "bootstrap" => genesis_cli::bootstrap::run(&rest),
        "build-plugin-agents" => genesis_cli::build_plugin_agents::run(&rest),
        // session-copy pipeline
        "capture" => genesis_cli::capture::run(&rest),
        "store" => genesis_cli::store::run(&rest),
        "embed" => genesis_cli::embed::run(&rest),
        "build-session-agent" => genesis_cli::build_session_agent::run(&rest),
        _ => {
            eprintln!(
                "genesis-cli: unknown subcommand {sub:?} (expected assemble|bootstrap|promote|demote|doctor|\
                 fix|reconcile|install|build-plugin-agents|capture|store|embed|build-session-agent)"
            );
            2
        }
    };
    std::process::exit(code);
}
