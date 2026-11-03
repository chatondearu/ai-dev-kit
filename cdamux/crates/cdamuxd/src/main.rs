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
        // Exit 0: the systemd unit uses Restart=on-failure, so a clean exit does not loop.
        println!(
            "cdamuxd: nothing to serve (daemon.listen_tailnet = false); set it to true or disable cda.aitools.cdamuxd"
        );
        return;
    }

    info!(roots = cfg.projects.roots.len(), "cdamuxd starting");
    // Serve on the main thread; a bind/tailscale failure exits non-zero so the unit restarts it.
    if let Err(e) = tailnet_server::run_tailnet_server(
        cfg.daemon.tailnet_port,
        env!("CARGO_PKG_VERSION").to_string(),
        cfg.projects.clone(),
        cfg.sessions.clone(),
        cfg.daemon.idle_seconds_for_needs_input,
    ) {
        eprintln!("cdamuxd: tailnet HTTP server: {e}");
        std::process::exit(1);
    }
}
