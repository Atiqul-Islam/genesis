//! genesis-cli — Genesis installer/orchestrator, busybox-style subcommand dispatch.
//!
//! `genesis-cli <assemble|bootstrap|promote|install|build-plugin-agents> [args...]`.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sub = args.get(1).map_or("", String::as_str);
    let rest: Vec<String> = args.iter().skip(2).cloned().collect();
    let code = match sub {
        "assemble" => genesis_cli::assemble::run(&rest),
        "promote" => genesis_cli::promote::run(&rest),
        "install" => genesis_cli::install::run(&rest),
        "bootstrap" => genesis_cli::bootstrap::run(&rest),
        "build-plugin-agents" => genesis_cli::build_plugin_agents::run(&rest),
        _ => {
            eprintln!(
                "genesis-cli: unknown subcommand {sub:?} \
                 (expected assemble|bootstrap|promote|install|build-plugin-agents)"
            );
            2
        }
    };
    std::process::exit(code);
}
