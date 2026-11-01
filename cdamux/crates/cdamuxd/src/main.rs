//! cdamuxd — tailnet HTTP API server. `cdamux list` and the TUI read tmux directly.

mod tailnet_server;
mod web_static;

use cdamux_core::load_config;
use tracing::info;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {e}");
            std::process::exit(1);
        }
    };

    if !cfg.daemon.listen_tailnet {
        eprintln!("cdamuxd: nothing to serve; set daemon.listen_tailnet = true");
        std::process::exit(1);
    }

    info!(roots = cfg.projects.roots.len(), "cdamuxd starting");
    tailnet_server::spawn_tailnet_server(
        cfg.daemon.tailnet_port,
        env!("CARGO_PKG_VERSION").to_string(),
        cfg.projects.clone(),
        cfg.sessions.clone(),
        cfg.daemon.idle_seconds_for_needs_input,
    );

    // The server runs on its own thread; keep the process alive.
    loop {
        std::thread::park();
    }
}
