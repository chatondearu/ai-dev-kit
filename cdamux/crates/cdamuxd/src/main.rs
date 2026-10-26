//! cdamuxd — tmux poll loop + Unix socket IPC.

mod tailnet_server;
mod web_static;

use cdamux_core::{
    handle_daemon_request, load_config, poll_sessions, socket_path, write_state_cache, GroupBy,
};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::time::Duration;
use tracing::{info, warn};

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

    let sock_path = match socket_path() {
        Some(p) => p,
        None => {
            eprintln!("cdamuxd: XDG_RUNTIME_DIR not set");
            std::process::exit(1);
        }
    };

    if let Some(parent) = sock_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("cdamuxd: cannot create runtime dir: {e}");
            std::process::exit(1);
        }
    }
    if sock_path.exists() {
        let _ = std::fs::remove_file(&sock_path);
    }

    let listener = match UnixListener::bind(&sock_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cdamuxd: bind {}: {e}", sock_path.display());
            std::process::exit(1);
        }
    };
    listener
        .set_nonblocking(true)
        .expect("set_nonblocking on UnixListener");

    info!(
        socket = %sock_path.display(),
        roots = cfg.projects.roots.len(),
        "cdamuxd listening"
    );

    if cfg.daemon.listen_tailnet {
        tailnet_server::spawn_tailnet_server(
            cfg.daemon.tailnet_port,
            env!("CARGO_PKG_VERSION").to_string(),
            cfg.projects.clone(),
            cfg.sessions.clone(),
            cfg.daemon.idle_seconds_for_needs_input,
        );
    }

    let poll_secs = cfg.daemon.poll_interval_seconds.max(1);
    let projects = cfg.projects.clone();

    loop {
        let idle = cfg.daemon.idle_seconds_for_needs_input;
        let sessions = poll_sessions(&projects, GroupBy::Project, idle);
        if let Err(e) = write_state_cache(&sessions) {
            warn!("state cache write: {e}");
        }

        while let Ok((stream, _)) = listener.accept() {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() {
                let response =
                    handle_daemon_request(line.trim(), &projects, cfg.daemon.idle_seconds_for_needs_input);
                let _ = reader.get_mut().write_all(response.as_bytes());
                let _ = reader.get_mut().write_all(b"\n");
            }
        }

        std::thread::sleep(Duration::from_secs(poll_secs as u64));
    }
}
