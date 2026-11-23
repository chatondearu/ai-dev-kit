//! cdamuxd — tailnet HTTP API server. `cdamux list` and the TUI read tmux directly.

mod tailnet_server;
mod web_static;

use cdamux_core::{
    load_config, resolve_tailnet_bind_ip, resolve_tailnet_token, tailnet_token_path,
    tailnet_token_mode_warning, TailnetIpError,
};
use std::time::Duration;
use tracing::{info, warn};

/// Wait between Tailscale checks when it is installed but not up.
const TAILSCALE_RETRY: Duration = Duration::from_secs(15 * 60);

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

    // H4: never serve without a token. A bad token file is host state like a missing
    // Tailscale: exit 0 with the reason so neither supervisor restarts us in a loop.
    let env_token = std::env::var("CDAMUX_TAILNET_TOKEN").ok();
    let token = match tailnet_token_path()
        .ok_or_else(|| "no config path for the tailnet token".to_string())
        .and_then(|file| {
            if env_token.as_deref().map_or(true, |t| t.trim().is_empty()) {
                if let Some(w) = tailnet_token_mode_warning(&file) {
                    warn!("{w}");
                    eprintln!("cdamuxd: warning: {w}");
                }
            }
            resolve_tailnet_token(env_token.as_deref(), &file)
                .map_err(|e| format!("tailnet token ({}): {e}", file.display()))
        }) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cdamuxd: {e}; not serving. Fix it then restart cdamuxd");
            return;
        }
    };

    info!(roots = cfg.projects.roots.len(), "cdamuxd starting");
    // Tailscale missing or down is host state, not a crash: exiting 1 would make launchd
    // (KeepAlive SuccessfulExit=false) / systemd (Restart=on-failure) restart us in a loop.
    let ip = loop {
        match resolve_tailnet_bind_ip() {
            Ok(ip) => break ip,
            Err(TailnetIpError::NotUp(e)) => {
                warn!("{e}");
                eprintln!(
                    "cdamuxd: Tailscale is not up ({e}); retrying in {} min",
                    TAILSCALE_RETRY.as_secs() / 60
                );
                std::thread::sleep(TAILSCALE_RETRY);
            }
            Err(e) => {
                // Exit 0 so neither supervisor restarts it.
                eprintln!(
                    "cdamuxd: {e}; not serving. Fix it then restart cdamuxd, or set daemon.listen_tailnet = false"
                );
                return;
            }
        }
    };
    // Serve on the main thread; a bind failure exits non-zero so the unit restarts it.
    if let Err(e) = tailnet_server::run_tailnet_server(
        ip,
        token,
        cfg.daemon.tailnet_port,
        env!("CARGO_PKG_VERSION").to_string(),
        cfg.projects.clone(),
        cfg.sessions.clone(),
        cfg.daemon.idle_seconds_for_needs_input,
        cfg.daemon.working_warn_seconds,
    ) {
        eprintln!("cdamuxd: tailnet HTTP server: {e}");
        std::process::exit(1);
    }
}
